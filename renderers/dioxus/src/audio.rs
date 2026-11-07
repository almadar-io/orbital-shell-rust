use futures_channel::oneshot;
use orbital_core::Value;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
static COMMANDS: OnceLock<Sender<Command>> = OnceLock::new();

/// A sound std-audio-cue resolved; the host plays it as given.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPlay {
    pub key: String,
    pub source: String,
    pub volume: f32,
    #[serde(rename = "loop")]
    pub looping: bool,
    pub pool_size: usize,
    pub fade_ms: u64,
}

impl AudioPlay {
    pub fn from_value(value: Option<&Value>) -> Result<Option<Self>, serde_json::Error> {
        match value {
            None | Some(Value::Null) => Ok(None),
            Some(value) => {
                serde_json::from_value(orbital_core::client::helpers::to_json(value)).map(Some)
            }
        }
    }

    pub fn list_from_value(value: Option<&Value>) -> Result<Vec<Self>, serde_json::Error> {
        match value {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(value) => serde_json::from_value(orbital_core::client::helpers::to_json(value)),
        }
    }
}

#[derive(Clone)]
pub struct AudioProps {
    pub source_trait: String,
    pub cue_seq: Option<f64>,
    pub cue_play: Option<AudioPlay>,
    pub music_play: Option<AudioPlay>,
    pub autostart: Vec<AudioPlay>,
    pub muted: bool,
    pub volume: f32,
    pub manifest: Value,
    pub base_url: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PortPaths {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortEntry {
    path: PortPaths,
    volume: Option<f32>,
    #[serde(rename = "loop")]
    looping: Option<bool>,
    pool_size: Option<usize>,
    crossfade: Option<bool>,
    crossfade_duration_ms: Option<u64>,
}

impl AudioProps {
    /// The key-addressed audio/* host port still resolves its key against the manifest here (G-CROSS-499).
    fn port_play(&self, key: &str, music: bool) -> Result<AudioPlay, AudioFailure> {
        let manifest: BTreeMap<String, PortEntry> =
            serde_json::from_value(orbital_core::client::helpers::to_json(&self.manifest))
                .map_err(|error| {
                    AudioFailure::invalid(format!("audio manifest is invalid: {error}"))
                })?;
        let entry = manifest
            .get(key)
            .ok_or_else(|| AudioFailure::invalid(format!("audio manifest has no entry `{key}`")))?;
        let path = match &entry.path {
            PortPaths::One(path) => path.clone(),
            PortPaths::Many(paths) if music => paths.first().cloned().unwrap_or_default(),
            PortPaths::Many(paths) if !paths.is_empty() => {
                paths[fastrand::usize(..paths.len())].clone()
            }
            PortPaths::Many(_) => String::new(),
        };
        if path.is_empty() {
            return Err(AudioFailure::invalid(format!(
                "audio entry `{key}` has no source"
            )));
        }
        Ok(AudioPlay {
            key: key.to_string(),
            source: format!("{}{path}", self.base_url),
            volume: entry.volume.unwrap_or(1.0),
            looping: music || entry.looping.unwrap_or(false),
            pool_size: entry.pool_size.unwrap_or(1).max(1),
            fade_ms: if !music || entry.crossfade == Some(false) {
                0
            } else {
                entry.crossfade_duration_ms.unwrap_or(1500)
            },
        })
    }
}

fn music_identity(props: &AudioProps) -> Option<(&str, &str)> {
    props
        .music_play
        .as_ref()
        .map(|plan| (plan.key.as_str(), plan.source.as_str()))
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

struct Voices {
    volume: f32,
    sinks: Vec<Sink>,
}

struct Session {
    synced: bool,
    music: Option<Sink>,
    music_entry_volume: f32,
    fade: Option<MusicFade>,
    effects: HashMap<String, Voices>,
    unlocked: bool,
    props: AudioProps,
}

/// What a frame asks the host to play, against the frame it last played.
#[derive(Debug, PartialEq)]
enum Played {
    Cue(AudioPlay),
    Music(AudioPlay),
}

fn changes(prior: Option<&AudioProps>, next: &AudioProps) -> Vec<Played> {
    let mut played = Vec::new();
    if prior.map(music_identity) != Some(music_identity(next)) {
        if let Some(plan) = &next.music_play {
            played.push(Played::Music(plan.clone()));
        }
    }
    if let (Some(before), Some(now), Some(plan)) =
        (prior.and_then(|p| p.cue_seq), next.cue_seq, &next.cue_play)
    {
        if before != now && !plan.key.is_empty() {
            played.push(Played::Cue(plan.clone()));
        }
    }
    played
}

impl Session {
    fn new(props: AudioProps) -> Self {
        Self {
            synced: false,
            music: None,
            music_entry_volume: 1.0,
            fade: None,
            effects: HashMap::new(),
            unlocked: false,
            props,
        }
    }

    fn sync(&mut self, stream: &OutputStream, next: AudioProps) {
        let played = changes(self.synced.then_some(&self.props), &next);
        self.synced = true;
        self.props = next;
        for item in played {
            let result = match &item {
                Played::Cue(plan) => self.play_effect(stream, plan),
                Played::Music(plan) => self.change_music(stream, plan),
            };
            match result {
                Ok(()) => {
                    #[cfg(feature = "research-audio")]
                    tracing::info!(?item, "native audio played");
                }
                Err(error) => tracing::error!(%error, "audio playback failed"),
            }
        }
        self.apply_levels();
    }

    fn change_music(
        &mut self,
        stream: &OutputStream,
        plan: &AudioPlay,
    ) -> Result<(), AudioFailure> {
        let incoming = if plan.key.is_empty() {
            None
        } else {
            let bytes = load_audio("", &plan.source).map_err(AudioFailure::unavailable)?;
            let decoder = Decoder::try_from(Cursor::new(bytes)).map_err(|error| {
                AudioFailure::invalid(format!(
                    "audio decoder rejected music `{}`: {error}",
                    plan.key
                ))
            })?;
            let sink = Sink::connect_new(stream.mixer());
            sink.append(decoder.repeat_infinite());
            sink.set_volume(0.0);
            Some(sink)
        };
        if let Some(old) = self.fade.take() {
            old.outgoing.stop();
        }
        let outgoing = self.music.take();
        let outgoing_start = outgoing.as_ref().map(Sink::volume).unwrap_or(0.0);
        self.music_entry_volume = plan.volume;
        self.music = incoming;
        if let Some(outgoing) = outgoing {
            self.fade = Some(MusicFade {
                began: Instant::now(),
                duration: Duration::from_millis(plan.fade_ms),
                paused_at: None,
                outgoing_start,
                outgoing,
            });
        } else if let Some(incoming) = &self.music {
            incoming.set_volume(self.effective_volume(plan.volume));
        }
        Ok(())
    }

    fn play_effect(&mut self, stream: &OutputStream, plan: &AudioPlay) -> Result<(), AudioFailure> {
        if self.props.muted {
            return Ok(());
        }
        let bytes = load_audio("", &plan.source).map_err(AudioFailure::unavailable)?;
        let decoder = Decoder::try_from(Cursor::new(bytes)).map_err(|error| {
            AudioFailure::invalid(format!(
                "audio decoder rejected cue `{}`: {error}",
                plan.key
            ))
        })?;
        let volume = self.effective_volume(plan.volume);
        let voices = self.effects.entry(plan.key.clone()).or_insert(Voices {
            volume: plan.volume,
            sinks: Vec::new(),
        });
        voices.volume = plan.volume;
        voices.sinks.retain(|sink| !sink.empty());
        if voices.sinks.len() >= plan.pool_size.max(1) {
            if plan.looping {
                return Ok(());
            }
            voices.sinks.remove(0).stop();
        }
        let sink = Sink::connect_new(stream.mixer());
        sink.set_volume(volume);
        if plan.looping {
            sink.append(decoder.repeat_infinite());
        } else {
            sink.append(decoder);
        }
        voices.sinks.push(sink);
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
        for plan in self.props.autostart.clone() {
            if let Err(error) = self.play_effect(stream, &plan) {
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
                    let plan = self.props.port_play(key, false)?;
                    self.play_effect(stream, &plan)
                } else {
                    Err(AudioFailure::invalid(
                        "audio/play-sound expects one sound key",
                    ))
                }
            }
            "audio/stop-sound" => {
                if let [Value::String(key)] = args {
                    if let Some(voices) = self.effects.remove(key.as_str()) {
                        for sink in voices.sinks {
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
                    let plan = self.props.port_play(key, true)?;
                    self.change_music(stream, &plan)
                } else if args.is_empty() || matches!(args, [Value::Null]) {
                    self.change_music(stream, &port_stop())
                } else {
                    Err(AudioFailure::invalid(
                        "audio/change-music expects a music key or none",
                    ))
                }
            }
            "audio/stop-music" if args.is_empty() => self.change_music(stream, &port_stop()),
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
        for voices in self.effects.values() {
            let volume = self.effective_volume(voices.volume);
            for sink in &voices.sinks {
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
        for voices in self.effects.values() {
            for sink in &voices.sinks {
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
        for voices in self.effects.values() {
            for sink in &voices.sinks {
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

/// The port's stop carries no fade length of its own; it keeps the one it always used (G-CROSS-499).
fn port_stop() -> AudioPlay {
    AudioPlay {
        key: String::new(),
        source: String::new(),
        volume: 0.0,
        looping: true,
        pool_size: 1,
        fade_ms: 1000,
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

    fn plan(key: &str, source: &str) -> AudioPlay {
        AudioPlay {
            key: key.into(),
            source: source.into(),
            volume: 0.4,
            looping: false,
            pool_size: 3,
            fade_ms: 0,
        }
    }

    fn props(seq: f64, music: AudioPlay) -> AudioProps {
        AudioProps {
            source_trait: "AudioCue".into(),
            cue_seq: Some(seq),
            cue_play: Some(plan("jump", "/assets/jump.wav")),
            music_play: Some(music),
            autostart: Vec::new(),
            muted: false,
            volume: 0.7,
            manifest: Value::Null,
            base_url: String::new(),
        }
    }

    #[test]
    fn rendered_plans_decode_from_the_std_audio_cue_props() {
        let value: Value = serde_json::from_str(
            r#"{"key":"jump","source":"/assets/jump.wav","volume":0.4,"loop":false,"poolSize":3,"fadeMs":0}"#,
        )
        .unwrap();
        assert_eq!(
            AudioPlay::from_value(Some(&value)).unwrap(),
            Some(plan("jump", "/assets/jump.wav"))
        );
        assert_eq!(AudioPlay::from_value(None).unwrap(), None);
        let missing: Value = serde_json::from_str(r#"{"key":"jump"}"#).unwrap();
        assert!(AudioPlay::from_value(Some(&missing)).is_err());
    }

    #[test]
    fn a_cue_plays_only_when_cue_seq_moves_after_mount_and_music_only_when_its_plan_changes() {
        let stop = AudioPlay {
            fade_ms: 1000,
            ..plan("", "")
        };
        let mounted = props(0.0, stop.clone());
        assert_eq!(changes(None, &mounted), vec![Played::Music(stop.clone())]);
        assert!(changes(Some(&mounted), &mounted).is_empty());
        assert_eq!(
            changes(Some(&mounted), &props(1.0, stop.clone())),
            vec![Played::Cue(plan("jump", "/assets/jump.wav"))]
        );
        let theme = plan("theme", "/assets/theme.ogg");
        assert_eq!(
            changes(Some(&mounted), &props(0.0, theme.clone())),
            vec![Played::Music(theme)]
        );
    }

    #[test]
    fn the_audio_port_resolves_its_key_against_the_manifest() {
        let mut port = props(0.0, plan("", ""));
        port.base_url = "/assets/".into();
        port.manifest = serde_json::from_str(
            r#"{"jump":{"path":["jump-1.wav"],"volume":0.4,"poolSize":3},"theme":{"path":"theme.ogg","crossfade":false}}"#,
        )
        .unwrap();
        let jump = port.port_play("jump", false).unwrap();
        assert_eq!(
            (jump.source.as_str(), jump.pool_size),
            ("/assets/jump-1.wav", 3)
        );
        assert_eq!(port.port_play("theme", true).unwrap().fade_ms, 0);
        assert!(port.port_play("missing", false).is_err());
    }
}
