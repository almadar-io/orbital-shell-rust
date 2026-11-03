use futures_channel::oneshot;
use orbital_core::audio_policy::{plan_audio, plan_audio_unlock, AudioAction, AudioFrame};
use orbital_core::Value;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
static COMMANDS: OnceLock<Sender<Command>> = OnceLock::new();

#[derive(Clone)]
pub struct AudioProps {
    pub source_trait: String,
    pub cue: String,
    pub cue_seq: Option<f64>,
    pub music: String,
    pub muted: bool,
    pub volume: f32,
    pub manifest: Value,
    pub base_url: String,
}

impl AudioProps {
    fn frame(&self) -> Result<AudioFrame, serde_json::Error> {
        serde_json::from_value(serde_json::json!({
            "cue": self.cue,
            "cueSeq": self.cue_seq,
            "music": self.music,
            "muted": self.muted,
            "volume": self.volume,
            "manifest": orbital_core::client::helpers::to_json(&self.manifest),
            "baseUrl": self.base_url,
        }))
    }
}

enum Command {
    Sync(u64, AudioProps),
    Gesture,
    HostAsk(
        String,
        String,
        Vec<Value>,
        oneshot::Sender<Result<(), AudioFailure>>,
    ),
    Suspend,
    Resume,
    Close(u64),
}

pub fn suspend() {
    if let Some(sender) = COMMANDS.get() {
        let _ = sender.send(Command::Suspend);
    }
}

pub fn resume() {
    if let Some(sender) = COMMANDS.get() {
        let _ = sender.send(Command::Resume);
    }
}

#[derive(Debug)]
pub struct AudioFailure {
    pub kind: &'static str,
    pub error: String,
}

impl AudioFailure {
    fn invalid(error: impl Into<String>) -> Self {
        Self {
            kind: "invalid",
            error: error.into(),
        }
    }

    fn unavailable(error: impl Into<String>) -> Self {
        Self {
            kind: "unavailable",
            error: error.into(),
        }
    }
}

impl std::fmt::Display for AudioFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.error)
    }
}

pub fn host_ask(
    source_trait: String,
    op: String,
    args: Vec<Value>,
) -> oneshot::Receiver<Result<(), AudioFailure>> {
    let (answer, receiver) = oneshot::channel();
    if let Some(sender) = COMMANDS.get() {
        if sender
            .send(Command::HostAsk(source_trait, op, args, answer))
            .is_err()
        {
            tracing::error!("native audio worker stopped before host ask");
        }
    } else {
        let _ = answer.send(Err(AudioFailure::unavailable(
            "native audio has no mounted session",
        )));
    }
    receiver
}

pub fn user_gesture() {
    if let Some(sender) = COMMANDS.get() {
        let _ = sender.send(Command::Gesture);
    }
}

pub struct AudioSessionHandle {
    id: u64,
    sender: Sender<Command>,
}

impl AudioSessionHandle {
    pub fn new() -> Self {
        let sender = COMMANDS
            .get_or_init(|| {
                let (sender, receiver) = mpsc::channel();
                std::thread::Builder::new()
                    .name("almadar-native-audio".into())
                    .spawn(move || run_audio(receiver))
                    .expect("audio worker thread");
                sender
            })
            .clone();
        Self {
            id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            sender,
        }
    }

    pub fn sync(&self, props: AudioProps) {
        if self.sender.send(Command::Sync(self.id, props)).is_err() {
            tracing::error!("native audio worker stopped");
        }
    }
}

impl Drop for AudioSessionHandle {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Close(self.id));
    }
}

struct MusicFade {
    began: Instant,
    duration: Duration,
    paused_at: Option<Instant>,
    outgoing_start: f32,
    outgoing: Sink,
}

struct Session {
    prior_frame: Option<AudioFrame>,
    music_key: String,
    music: Option<Sink>,
    music_entry_volume: f32,
    fade: Option<MusicFade>,
    effects: HashMap<String, Vec<Sink>>,
    unlocked: bool,
    props: AudioProps,
}

impl Session {
    fn new(props: AudioProps) -> Self {
        Self {
            prior_frame: None,
            music_key: String::new(),
            music: None,
            music_entry_volume: 1.0,
            fade: None,
            effects: HashMap::new(),
            unlocked: false,
            props,
        }
    }

    fn sync(&mut self, stream: &OutputStream, next: AudioProps) {
        let frame = match next.frame() {
            Ok(frame) => frame,
            Err(error) => {
                tracing::error!(%error, "invalid native audio frame");
                return;
            }
        };
        let actions = plan_audio(self.prior_frame.as_ref(), &frame);
        self.prior_frame = Some(frame);
        self.props = next;
        for action in actions {
            match action {
                AudioAction::PlayCue { key, .. } => match self.play_effect(stream, &key) {
                    Ok(()) => {
                        #[cfg(feature = "research-audio")]
                        tracing::info!(cue = %key, "native audio cue scheduled");
                    }
                    Err(error) => tracing::error!(%error, "audio cue failed"),
                },
                AudioAction::ChangeMusic { .. } | AudioAction::StopMusic => {
                    match self.change_music(stream) {
                        Ok(()) => {
                            #[cfg(feature = "research-audio")]
                            tracing::info!(music = %self.props.music, "native audio music changed");
                        }
                        Err(error) => tracing::error!(%error, "audio music change failed"),
                    }
                }
                AudioAction::SetMuted { .. } | AudioAction::SetVolume { .. } => self.apply_levels(),
            }
        }
        self.apply_levels();
    }

    fn change_music(&mut self, stream: &OutputStream) -> Result<(), AudioFailure> {
        let requested = self.props.music.clone();
        if requested.is_empty() {
            if let Some(old) = self.fade.take() {
                old.outgoing.stop();
            }
            let outgoing = self.music.take();
            let outgoing_start = outgoing.as_ref().map(Sink::volume).unwrap_or(0.0);
            self.music_key.clear();
            if let Some(outgoing) = outgoing {
                self.fade = Some(MusicFade {
                    began: Instant::now(),
                    duration: Duration::from_millis(1000),
                    paused_at: None,
                    outgoing_start,
                    outgoing,
                });
            }
            return Ok(());
        }
        let Some(frame) = &self.prior_frame else {
            return Err(AudioFailure::invalid("audio frame is missing"));
        };
        let Some(entry) = frame.manifest.get(&requested) else {
            return Err(AudioFailure::invalid(format!(
                "audio manifest has no music entry `{requested}`"
            )));
        };
        let Some(path) = &entry.path else {
            return Err(AudioFailure::invalid("audio music has no source"));
        };
        let sources = path.prefixed(&frame.base_url);
        let Some(path) = sources.first() else {
            return Err(AudioFailure::invalid("audio music has no source"));
        };
        let bytes = load_audio("", path).map_err(AudioFailure::unavailable)?;
        let decoder = Decoder::try_from(Cursor::new(bytes)).map_err(|error| {
            AudioFailure::invalid(format!(
                "audio decoder rejected music `{requested}`: {error}"
            ))
        })?;
        if let Some(old) = self.fade.take() {
            old.outgoing.stop();
        }
        let outgoing = self.music.take();
        let outgoing_start = outgoing.as_ref().map(Sink::volume).unwrap_or(0.0);
        let incoming = Sink::connect_new(stream.mixer());
        incoming.append(decoder.repeat_infinite());
        incoming.set_volume(0.0);
        self.music_key = requested;
        self.music_entry_volume = entry.volume;
        self.music = Some(incoming);
        if let Some(outgoing) = outgoing {
            self.fade = Some(MusicFade {
                began: Instant::now(),
                duration: Duration::from_millis(entry.crossfade_duration_ms),
                paused_at: None,
                outgoing_start,
                outgoing,
            });
        } else if let Some(incoming) = &self.music {
            incoming.set_volume(self.effective_volume(entry.volume));
        }
        Ok(())
    }

    fn play_effect(&mut self, stream: &OutputStream, key: &str) -> Result<(), AudioFailure> {
        let Some(frame) = &self.prior_frame else {
            return Err(AudioFailure::invalid("audio frame is missing"));
        };
        let Some(entry) = frame.manifest.get(key) else {
            return Err(AudioFailure::invalid(format!(
                "audio manifest has no cue entry `{key}`"
            )));
        };
        let Some(path) = &entry.path else {
            return Err(AudioFailure::invalid("audio cue has no source"));
        };
        let sources = path.prefixed(&frame.base_url);
        if sources.is_empty() {
            return Err(AudioFailure::invalid("audio cue has no source"));
        }
        let index = fastrand::usize(..sources.len());
        let bytes = load_audio("", &sources[index]).map_err(AudioFailure::unavailable)?;
        let decoder = Decoder::try_from(Cursor::new(bytes)).map_err(|error| {
            AudioFailure::invalid(format!("audio decoder rejected cue `{key}`: {error}"))
        })?;
        let pool = self.effects.entry(key.to_string()).or_default();
        pool.retain(|sink| !sink.empty());
        if pool.len() >= entry.pool_size.max(1) {
            if entry.looping {
                return Ok(());
            }
            pool.remove(0).stop();
        }
        let sink = Sink::connect_new(stream.mixer());
        sink.set_volume(if self.props.muted {
            0.0
        } else {
            (entry.volume * self.props.volume).clamp(0.0, 1.0)
        });
        if entry.looping {
            sink.append(decoder.repeat_infinite());
        } else {
            sink.append(decoder);
        }
        pool.push(sink);
        Ok(())
    }

    fn unlock(&mut self, stream: &OutputStream) {
        if self.unlocked {
            return;
        }
        self.unlocked = true;
        if self.props.muted {
            return;
        }
        let actions = self
            .prior_frame
            .as_ref()
            .map(plan_audio_unlock)
            .unwrap_or_default();
        for action in actions {
            let AudioAction::PlayCue { key, .. } = action else {
                continue;
            };
            if let Err(error) = self.play_effect(stream, &key) {
                tracing::error!(message = %error.error, "audio autostart failed");
            }
        }
    }

    fn host_ask(
        &mut self,
        stream: &OutputStream,
        op: &str,
        args: &[Value],
    ) -> Result<(), AudioFailure> {
        match op {
            "audio/play-sound" => {
                if let [Value::String(key)] = args {
                    self.play_effect(stream, key)
                } else {
                    Err(AudioFailure::invalid(
                        "audio/play-sound expects one sound key",
                    ))
                }
            }
            "audio/stop-sound" => {
                if let [Value::String(key)] = args {
                    if let Some(pool) = self.effects.remove(key.as_str()) {
                        for sink in pool {
                            sink.stop();
                        }
                    }
                    Ok(())
                } else if args.is_empty() || matches!(args, [Value::Null]) {
                    self.effects.clear();
                    Ok(())
                } else {
                    Err(AudioFailure::invalid(
                        "audio/stop-sound expects a sound key or none",
                    ))
                }
            }
            "audio/change-music" => {
                if let [Value::String(key)] = args {
                    self.props.music = key.to_string();
                    self.change_music(stream)
                } else if args.is_empty() || matches!(args, [Value::Null]) {
                    self.props.music.clear();
                    self.change_music(stream)
                } else {
                    Err(AudioFailure::invalid(
                        "audio/change-music expects a music key or none",
                    ))
                }
            }
            "audio/stop-music" if args.is_empty() => {
                self.props.music.clear();
                self.change_music(stream)
            }
            _ => Err(AudioFailure::unavailable(format!(
                "unsupported native audio host ask `{op}`"
            ))),
        }
    }

    fn effective_volume(&self, entry_volume: f32) -> f32 {
        if self.props.muted {
            0.0
        } else {
            (entry_volume * self.props.volume).clamp(0.0, 1.0)
        }
    }

    fn apply_levels(&self) {
        if let Some(music) = &self.music {
            if self.fade.is_none() {
                music.set_volume(self.effective_volume(self.music_entry_volume));
            }
        }
        for (key, pool) in &self.effects {
            let volume = self
                .prior_frame
                .as_ref()
                .and_then(|frame| frame.manifest.get(key))
                .map(|entry| self.effective_volume(entry.volume))
                .unwrap_or(0.0);
            for sink in pool {
                sink.set_volume(volume);
            }
        }
    }

    fn pause(&mut self) {
        if let Some(music) = &self.music {
            music.pause();
        }
        if let Some(fade) = &mut self.fade {
            fade.outgoing.pause();
            if fade.paused_at.is_none() {
                fade.paused_at = Some(Instant::now());
            }
        }
        for pool in self.effects.values() {
            for sink in pool {
                sink.pause();
            }
        }
    }

    fn resume(&mut self) {
        if let Some(music) = &self.music {
            music.play();
        }
        if let Some(fade) = &mut self.fade {
            fade.outgoing.play();
            if let Some(paused_at) = fade.paused_at.take() {
                fade.began = fade.began + paused_at.elapsed();
            }
        }
        for pool in self.effects.values() {
            for sink in pool {
                sink.play();
            }
        }
    }

    fn tick(&mut self) {
        let Some(fade) = &self.fade else { return };
        if fade.paused_at.is_some() {
            return;
        }
        let progress = if fade.duration.is_zero() {
            1.0
        } else {
            (fade.began.elapsed().as_secs_f32() / fade.duration.as_secs_f32()).min(1.0)
        };
        fade.outgoing.set_volume(if self.props.muted {
            0.0
        } else {
            fade.outgoing_start * (1.0 - progress)
        });
        if let Some(incoming) = &self.music {
            incoming.set_volume(if self.props.muted {
                0.0
            } else {
                self.music_entry_volume * self.props.volume * progress
            });
        }
        if progress >= 1.0 {
            if let Some(fade) = self.fade.take() {
                fade.outgoing.stop();
            }
        }
    }
}

fn load_audio(base_url: &str, path: &str) -> Result<Vec<u8>, String> {
    let source = format!("{base_url}{path}");
    if source.starts_with("http://") || source.starts_with("https://") {
        let response = ureq::get(&source)
            .call()
            .map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        return Ok(bytes);
    }
    if let Some(path) = source.strip_prefix("file://") {
        return std::fs::read(path).map_err(|error| error.to_string());
    }
    let response =
        dioxus_asset_resolver::native::serve_asset(&source).map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "audio asset {source} returned {}",
            response.status()
        ));
    }
    Ok(response.into_body())
}

fn run_audio(receiver: Receiver<Command>) {
    let mut stream: Option<OutputStream> = None;
    let mut sessions: HashMap<u64, Session> = HashMap::new();
    let mut suspended = false;
    loop {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(Command::Sync(id, props)) => {
                if stream.is_none() {
                    match OutputStreamBuilder::open_default_stream() {
                        Ok(opened) => stream = Some(opened),
                        Err(error) => tracing::error!(%error, "native audio output unavailable"),
                    }
                }
                let session = sessions
                    .entry(id)
                    .or_insert_with(|| Session::new(props.clone()));
                if let Some(stream) = &stream {
                    session.sync(stream, props);
                    if suspended {
                        session.pause();
                    }
                } else {
                    session.props = props;
                }
            }
            Ok(Command::Close(id)) => {
                sessions.remove(&id);
            }
            Ok(Command::Gesture) => {
                if let Some(stream) = &stream {
                    for session in sessions.values_mut() {
                        session.unlock(stream);
                    }
                }
            }
            Ok(Command::HostAsk(source_trait, op, args, answer)) => {
                let result = if let Some(stream) = &stream {
                    let mut matching: Vec<_> = sessions
                        .values_mut()
                        .filter(|session| session.props.source_trait == source_trait)
                        .collect();
                    if let [session] = matching.as_mut_slice() {
                        session.host_ask(stream, &op, &args)
                    } else {
                        Err(AudioFailure::unavailable(format!(
                            "audio host ask `{op}` needs one matching session for `{source_trait}`; found {}",
                            matching.len()
                        )))
                    }
                } else {
                    Err(AudioFailure::unavailable("native audio output unavailable"))
                };
                let _ = answer.send(result);
            }
            Ok(Command::Suspend) => {
                suspended = true;
                for session in sessions.values_mut() {
                    session.pause();
                }
            }
            Ok(Command::Resume) => {
                suspended = false;
                for session in sessions.values_mut() {
                    session.resume();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        for session in sessions.values_mut() {
            session.tick();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dioxus_props_use_the_shared_audio_frame_contract() {
        let props = AudioProps {
            source_trait: "AudioCue".into(),
            cue: "jump".into(),
            cue_seq: Some(1.0),
            music: String::new(),
            muted: false,
            volume: 0.7,
            manifest: serde_json::from_str(
                r#"{"jump":{"path":["jump-1.wav","jump-2.wav"],"volume":0.4,"poolSize":3}}"#,
            )
            .unwrap(),
            base_url: "/assets/".into(),
        };
        let frame = props.frame().unwrap();
        assert_eq!(frame.cue_seq, Some(1.0));
        assert_eq!(frame.manifest["jump"].pool_size, 3);
        assert_eq!(
            frame.manifest["jump"]
                .path
                .as_ref()
                .unwrap()
                .prefixed(&frame.base_url),
            ["/assets/jump-1.wav", "/assets/jump-2.wav"]
        );
    }
}
