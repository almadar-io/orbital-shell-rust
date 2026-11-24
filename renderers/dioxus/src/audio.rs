use futures_channel::oneshot;
use orbital_core::Value;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::audio_timeline as timeline;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
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
}

/// An `audio/*` host ask, decoded: what std-audio-cue resolved, the key of a playing sound, or the music fade-out length.
#[derive(Clone, Debug, PartialEq)]
pub enum PortAsk {
    PlaySound(AudioPlay),
    StopSound(Option<String>),
    ChangeMusic(AudioPlay),
    StopMusic(u64),
    Schedule(Vec<timeline::Command>),
    Clock,
}

pub fn decode_port(op: &str, args: &[Value]) -> Result<PortAsk, AudioFailure> {
    let play = |op: &str| {
        match args {
        [value] => AudioPlay::from_value(Some(value))
            .ok()
            .flatten()
            .ok_or_else(|| AudioFailure::invalid(format!("{op} takes one resolved AudioPlay {{ key, source, volume, loop, poolSize, fadeMs }}"))),
        _ => Err(AudioFailure::invalid(format!("{op} takes one resolved AudioPlay {{ key, source, volume, loop, poolSize, fadeMs }}"))),
    }
    };
    match op {
        "audio/play-sound" => play(op).map(PortAsk::PlaySound),
        "audio/change-music" => play(op).map(PortAsk::ChangeMusic),
        "audio/stop-sound" => match args {
            [Value::String(key)] if !key.is_empty() => {
                Ok(PortAsk::StopSound(Some(key.to_string())))
            }
            [] => Ok(PortAsk::StopSound(None)),
            _ => Err(AudioFailure::invalid(
                "audio/stop-sound takes a sound key or none",
            )),
        },
        "audio/stop-music" => match args {
            [value] => orbital_core::client::helpers::to_json(value)
                .as_f64()
                .filter(|ms| ms.is_finite() && *ms >= 0.0)
                .map(|ms| PortAsk::StopMusic(ms.round() as u64))
                .ok_or_else(|| {
                    AudioFailure::invalid("audio/stop-music takes one fade length in milliseconds")
                }),
            _ => Err(AudioFailure::invalid(
                "audio/stop-music takes one fade length in milliseconds",
            )),
        },
        "audio/schedule" => {
            let json: Vec<serde_json::Value> = args.iter().map(orbital_core::client::helpers::to_json).collect();
            timeline::decode_commands(&json)
                .map(PortAsk::Schedule)
                .map_err(AudioFailure::from)
        }
        "audio/clock" => match args {
            [] => Ok(PortAsk::Clock),
            _ => Err(AudioFailure::invalid("audio/clock takes no arguments")),
        },
        _ => Err(AudioFailure::unavailable(format!(
            "unsupported native audio host ask `{op}`"
        ))),
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
        oneshot::Sender<Result<Value, AudioFailure>>,
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

impl From<timeline::Failure> for AudioFailure {
    fn from(failure: timeline::Failure) -> Self {
        Self {
            kind: failure.kind,
            error: failure.error,
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
) -> oneshot::Receiver<Result<Value, AudioFailure>> {
    let (answer, receiver) = oneshot::channel();
    // The timeline asks need no mounted cue session: a page that only schedules clips starts the worker itself.
    let sender = if op == "audio/schedule" || op == "audio/clock" {
        Some(commands())
    } else {
        COMMANDS.get().cloned()
    };
    if let Some(sender) = sender {
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

fn commands() -> Sender<Command> {
    COMMANDS
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel();
            std::thread::Builder::new()
                .name("almadar-native-audio".into())
                .spawn(move || run_audio(receiver))
                .expect("audio worker thread");
            sender
        })
        .clone()
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
        let sender = commands();
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
            let bytes = load_audio(&plan.source).map_err(AudioFailure::unavailable)?;
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
        let bytes = load_audio(&plan.source).map_err(AudioFailure::unavailable)?;
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
        match decode_port(op, args)? {
            PortAsk::PlaySound(plan) => self.play_effect(stream, &plan),
            PortAsk::StopSound(Some(key)) => {
                if let Some(voices) = self.effects.remove(key.as_str()) {
                    for sink in voices.sinks {
                        sink.stop();
                    }
                }
                Ok(())
            }
            PortAsk::StopSound(None) => {
                self.effects.clear();
                Ok(())
            }
            PortAsk::ChangeMusic(plan) => self.change_music(stream, &plan),
            PortAsk::StopMusic(fade_ms) => self.change_music(stream, &music_stop(fade_ms)),
            PortAsk::Schedule(_) | PortAsk::Clock => Err(AudioFailure::unavailable(format!(
                "`{op}` is answered by the audio timeline, not a cue session"
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

fn music_stop(fade_ms: u64) -> AudioPlay {
    AudioPlay {
        key: String::new(),
        source: String::new(),
        volume: 0.0,
        looping: true,
        pool_size: 1,
        fade_ms,
    }
}

fn load_audio(source: &str) -> Result<Vec<u8>, String> {
    let source = source.to_string();
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

/// The timeline's voices as one source on the output mixer: it pulls a block of frames at a time under one lock.
struct TimelineSource {
    mixer: Arc<Mutex<timeline::Mixer>>,
    channels: u16,
    rate: u32,
    block: Vec<f32>,
    cursor: usize,
}

impl Iterator for TimelineSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.cursor >= self.block.len() {
            self.block.clear();
            self.block.resize(1024 * self.channels as usize, 0.0);
            if let Ok(mut mixer) = self.mixer.lock() {
                mixer.render(&mut self.block);
            }
            self.cursor = 0;
        }
        let sample = self.block[self.cursor];
        self.cursor += 1;
        Some(sample)
    }
}

impl Source for TimelineSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// `audio/schedule` and `audio/clock` (the web host's twin is `providers/audio-scheduler.ts`): the declared commands on the
/// output's own sample clock, each started source decoded once and kept only while an ask names it.
fn timeline_ask(
    stream: &OutputStream,
    mixer: &mut Option<Arc<Mutex<timeline::Mixer>>>,
    decoded: &mut HashMap<String, timeline::Pcm>,
    op: &str,
    args: &[Value],
) -> Result<Value, AudioFailure> {
    let shared = mixer
        .get_or_insert_with(|| {
            let config = stream.config();
            let shared = Arc::new(Mutex::new(timeline::Mixer::new(
                config.sample_rate(),
                config.channel_count(),
            )));
            stream.mixer().add(TimelineSource {
                mixer: shared.clone(),
                channels: config.channel_count(),
                rate: config.sample_rate(),
                block: Vec::new(),
                cursor: 0,
            });
            shared
        })
        .clone();
    let poisoned = || AudioFailure::unavailable("the native audio timeline stopped");
    match decode_port(op, args)? {
        PortAsk::Clock => {
            let audio = shared.lock().map_err(|_| poisoned())?.clock();
            let wall = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs_f64() * 1000.0)
                .map_err(|error| AudioFailure::unavailable(format!("audio/clock: {error}")))?;
            Ok(orbital_core::client::helpers::from_json(serde_json::json!({ "audio": audio, "wall": wall })))
        }
        PortAsk::Schedule(commands) => {
            shared.lock().map_err(|_| poisoned())?.check(&commands)?;
            let named = timeline::sources(&commands);
            for source in &named {
                if decoded.contains_key(source) {
                    continue;
                }
                let bytes = match timeline::data_url_bytes(source) {
                    Some(bytes) => bytes,
                    None => load_audio(source).map_err(|error| AudioFailure::unavailable(format!("audio/schedule: {error}")))?,
                };
                let decoder = Decoder::try_from(Cursor::new(bytes))
                    .map_err(|error| AudioFailure::unavailable(format!("audio/schedule: a source did not decode: {error}")))?;
                let channels = decoder.channels();
                let rate = decoder.sample_rate();
                let samples: Vec<f32> = decoder.collect();
                decoded.insert(source.clone(), timeline::Pcm { samples: Arc::new(samples), channels, rate });
            }
            shared.lock().map_err(|_| poisoned())?.apply(&commands, decoded)?;
            decoded.retain(|source, _| named.contains(source));
            Ok(Value::Null)
        }
        _ => Err(AudioFailure::unavailable(format!("`{op}` is not an audio timeline ask"))),
    }
}

fn run_audio(receiver: Receiver<Command>) {
    let mut stream: Option<OutputStream> = None;
    let mut sessions: HashMap<u64, Session> = HashMap::new();
    let mut suspended = false;
    let mut timeline_mixer: Option<Arc<Mutex<timeline::Mixer>>> = None;
    let mut timeline_sources: HashMap<String, timeline::Pcm> = HashMap::new();
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
            Ok(Command::HostAsk(_, op, args, answer)) if op == "audio/schedule" || op == "audio/clock" => {
                if stream.is_none() {
                    match OutputStreamBuilder::open_default_stream() {
                        Ok(opened) => stream = Some(opened),
                        Err(error) => tracing::error!(%error, "native audio output unavailable"),
                    }
                }
                let result = match &stream {
                    Some(stream) => timeline_ask(stream, &mut timeline_mixer, &mut timeline_sources, &op, &args),
                    None => Err(AudioFailure::unavailable("native audio output unavailable")),
                };
                let _ = answer.send(result);
            }
            Ok(Command::HostAsk(source_trait, op, args, answer)) => {
                let result = if let Some(stream) = &stream {
                    let mut matching: Vec<_> = sessions
                        .values_mut()
                        .filter(|session| session.props.source_trait == source_trait)
                        .collect();
                    if let [session] = matching.as_mut_slice() {
                        session.host_ask(stream, &op, &args).map(|()| Value::Null)
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
    fn the_audio_port_takes_the_resolved_play_the_key_or_the_fade_length() {
        let jump: Value = serde_json::from_str(
            r#"{"key":"jump","source":"/assets/jump.wav","volume":0.4,"loop":false,"poolSize":3,"fadeMs":0}"#,
        )
        .unwrap();
        assert_eq!(
            decode_port("audio/play-sound", &[jump.clone()]).unwrap(),
            PortAsk::PlaySound(plan("jump", "/assets/jump.wav"))
        );
        assert_eq!(
            decode_port("audio/change-music", &[jump]).unwrap(),
            PortAsk::ChangeMusic(plan("jump", "/assets/jump.wav"))
        );
        assert_eq!(
            decode_port("audio/stop-sound", &[Value::String("jump".into())]).unwrap(),
            PortAsk::StopSound(Some("jump".into()))
        );
        assert_eq!(
            decode_port("audio/stop-sound", &[]).unwrap(),
            PortAsk::StopSound(None)
        );
        let fade: Value = serde_json::from_str("750").unwrap();
        assert_eq!(
            decode_port("audio/stop-music", &[fade]).unwrap(),
            PortAsk::StopMusic(750)
        );
    }

    #[test]
    fn the_timeline_asks_decode_to_their_commands_and_the_clock_takes_no_argument() {
        let list = orbital_core::client::helpers::from_json(serde_json::json!([{ "stop": { "key": "a", "at": 2 } }]));
        assert_eq!(
            decode_port("audio/schedule", &[list]).unwrap(),
            PortAsk::Schedule(vec![timeline::Command::Stop { key: "a".into(), at: 2.0 }])
        );
        assert_eq!(decode_port("audio/clock", &[]).unwrap(), PortAsk::Clock);
        assert_eq!(decode_port("audio/clock", &[Value::Null]).unwrap_err().kind, "invalid");
        assert_eq!(decode_port("audio/schedule", &[]).unwrap_err().kind, "invalid");
    }

    #[test]
    fn control_a_bare_key_a_partial_play_or_a_stop_without_its_fade_is_refused() {
        assert!(decode_port("audio/play-sound", &[Value::String("jump".into())]).is_err());
        let partial: Value = serde_json::from_str(r#"{"key":"jump","source":"/a.wav"}"#).unwrap();
        assert!(decode_port("audio/change-music", &[partial]).is_err());
        assert!(decode_port("audio/stop-music", &[]).is_err());
        let negative: Value = serde_json::from_str("-5").unwrap();
        assert!(decode_port("audio/stop-music", &[negative]).is_err());
        assert!(decode_port("audio/unknown", &[]).is_err());
    }
}
