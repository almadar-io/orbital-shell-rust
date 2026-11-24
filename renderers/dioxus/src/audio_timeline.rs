//! Scheduled audio on one sample clock: the native answer to `audio/schedule` and `audio/clock`
//! (the web host's twin is `renderers/web/src/providers/audio-scheduler.ts`). No device here: a host
//! feeds decoded sources in and pulls interleaved frames out, so every rule is testable without sound.

use serde_json::Value as Json;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Linear,
    Exponential,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Param {
    Gain,
    Rate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Start {
        key: String,
        source: String,
        at: f64,
        offset: f64,
        duration: Option<f64>,
        rate: f64,
        gain: f64,
    },
    Stop {
        key: String,
        at: f64,
    },
    Ramp {
        key: String,
        param: Param,
        value: f64,
        at: f64,
        end_at: f64,
        curve: Curve,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub kind: &'static str,
    pub error: String,
}

impl Failure {
    fn invalid(error: impl Into<String>) -> Self {
        Self { kind: "invalid", error: error.into() }
    }
}

fn number(body: &serde_json::Map<String, Json>, name: &str) -> Option<f64> {
    body.get(name).and_then(Json::as_f64).filter(|n| n.is_finite())
}

/// The commands an `audio/schedule` ask carries, refused with the web host's wording.
pub fn decode_commands(args: &[Json]) -> Result<Vec<Command>, Failure> {
    let list = match args {
        [Json::Array(list)] => list,
        _ => return Err(Failure::invalid("audio/schedule takes one list of commands")),
    };
    let mut commands = Vec::with_capacity(list.len());
    for (index, entry) in list.iter().enumerate() {
        let place = format!("audio/schedule command {index}");
        let record = entry.as_object().ok_or_else(|| Failure::invalid(format!("{place} is not a record")))?;
        if record.len() != 1 {
            return Err(Failure::invalid(format!("{place} must have exactly one key: start, stop or ramp")));
        }
        let (name, body) = record.iter().next().expect("one key");
        let body = body.as_object().ok_or_else(|| Failure::invalid(format!("{place}: {name} takes a record")))?;
        let key = match body.get("key").and_then(Json::as_str) {
            Some(key) if !key.is_empty() => key.to_string(),
            _ => return Err(Failure::invalid(format!("{place}: {name} needs a key"))),
        };
        match name.as_str() {
            "stop" => {
                let at = number(body, "at").ok_or_else(|| Failure::invalid(format!("{place}: stop needs a numeric at")))?;
                commands.push(Command::Stop { key, at });
            }
            "ramp" => {
                let param = match body.get("param").and_then(Json::as_str) {
                    Some("gain") => Param::Gain,
                    Some("rate") => Param::Rate,
                    _ => return Err(Failure::invalid(format!("{place}: ramp param is gain or rate"))),
                };
                let curve = match body.get("curve").and_then(Json::as_str) {
                    Some("linear") => Curve::Linear,
                    Some("exponential") => Curve::Exponential,
                    _ => return Err(Failure::invalid(format!("{place}: ramp curve is linear or exponential"))),
                };
                let (value, at, end_at) = match (number(body, "value"), number(body, "at"), number(body, "endAt")) {
                    (Some(value), Some(at), Some(end_at)) => (value, at, end_at),
                    _ => return Err(Failure::invalid(format!("{place}: ramp needs numeric value, at and endAt"))),
                };
                commands.push(Command::Ramp { key, param, value, at, end_at, curve });
            }
            "start" => {
                let source = match body.get("source").and_then(Json::as_str) {
                    Some(source) if !source.is_empty() => source.to_string(),
                    _ => return Err(Failure::invalid(format!("{place}: start needs a source"))),
                };
                let (at, offset, rate, gain) = match (number(body, "at"), number(body, "offset"), number(body, "rate"), number(body, "gain")) {
                    (Some(at), Some(offset), Some(rate), Some(gain)) => (at, offset, rate, gain),
                    _ => return Err(Failure::invalid(format!("{place}: start needs numeric at, offset, rate and gain"))),
                };
                let duration = match body.get("duration") {
                    None | Some(Json::Null) => None,
                    Some(_) => Some(number(body, "duration").ok_or_else(|| Failure::invalid(format!("{place}: start duration is a number")))?),
                };
                commands.push(Command::Start { key, source, at, offset, duration, rate, gain });
            }
            _ => return Err(Failure::invalid(format!("{place} must have exactly one key: start, stop or ramp"))),
        }
    }
    Ok(commands)
}

/// The distinct sources a list of commands starts, in first-use order: what the host decodes before anything plays.
pub fn sources(commands: &[Command]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for command in commands {
        if let Command::Start { source, .. } = command {
            if !seen.iter().any(|s| s == source) {
                seen.push(source.clone());
            }
        }
    }
    seen
}

/// A decoded source: interleaved samples at its own channel count and rate.
#[derive(Clone, Debug)]
pub struct Pcm {
    pub samples: Arc<Vec<f32>>,
    pub channels: u16,
    pub rate: u32,
}

impl Pcm {
    fn frames(&self) -> usize {
        if self.channels == 0 { 0 } else { self.samples.len() / self.channels as usize }
    }
}

#[derive(Clone, Debug)]
struct Segment {
    at: f64,
    end_at: f64,
    from: f64,
    to: f64,
    curve: Curve,
}

/// A parameter over the audio clock: its base value, then each ramp anchoring the value it had and moving to its target.
#[derive(Clone, Debug)]
struct Automation {
    base: f64,
    target: f64,
    segments: Vec<Segment>,
}

impl Automation {
    fn new(value: f64) -> Self {
        Self { base: value, target: value, segments: Vec::new() }
    }

    fn ramp(&mut self, value: f64, at: f64, end_at: f64, curve: Curve) {
        self.segments.push(Segment { at, end_at, from: self.target, to: value, curve });
        self.target = value;
    }

    fn at(&self, time: f64) -> f64 {
        let mut value = self.base;
        for segment in &self.segments {
            if time < segment.at {
                break;
            }
            value = if time >= segment.end_at || segment.end_at <= segment.at {
                segment.to
            } else {
                let t = (time - segment.at) / (segment.end_at - segment.at);
                match segment.curve {
                    Curve::Exponential if segment.from > 0.0 && segment.to > 0.0 => segment.from * (segment.to / segment.from).powf(t),
                    _ => segment.from + (segment.to - segment.from) * t,
                }
            };
        }
        value
    }
}

#[derive(Clone, Debug)]
struct Voice {
    pcm: Pcm,
    start: f64,
    position: f64,
    end: f64,
    stop_at: Option<f64>,
    gain: Automation,
    rate: Automation,
    ended: bool,
}

/// Voices mixed on one clock. The clock is the frames rendered so far over the output rate.
pub struct Mixer {
    rate: u32,
    channels: u16,
    frame: u64,
    voices: HashMap<String, Voice>,
    tails: Vec<Voice>,
}

impl Mixer {
    pub fn new(rate: u32, channels: u16) -> Self {
        Self { rate: rate.max(1), channels: channels.max(1), frame: 0, voices: HashMap::new(), tails: Vec::new() }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// The audio clock, in seconds.
    pub fn clock(&self) -> f64 {
        self.frame as f64 / self.rate as f64
    }

    /// A stop or a ramp for a key no command started fails the whole ask, before anything plays.
    pub fn check(&self, commands: &[Command]) -> Result<(), Failure> {
        let mut known: Vec<&str> = self.voices.keys().map(String::as_str).collect();
        for command in commands {
            match command {
                Command::Start { key, .. } => {
                    if !known.contains(&key.as_str()) {
                        known.push(key);
                    }
                }
                Command::Stop { key, .. } => {
                    if !known.contains(&key.as_str()) {
                        return Err(Failure::invalid(format!("audio/schedule: no voice named {key} was started")));
                    }
                    known.retain(|k| k != key);
                }
                Command::Ramp { key, .. } => {
                    if !known.contains(&key.as_str()) {
                        return Err(Failure::invalid(format!("audio/schedule: no voice named {key} was started")));
                    }
                }
            }
        }
        Ok(())
    }

    /// Plays exactly the declared commands. `decoded` holds every source the commands start.
    pub fn apply(&mut self, commands: &[Command], decoded: &HashMap<String, Pcm>) -> Result<(), Failure> {
        self.check(commands)?;
        for command in commands {
            if let Command::Start { source, .. } = command {
                if !decoded.contains_key(source) {
                    return Err(Failure { kind: "unavailable", error: format!("audio/schedule: {source} was not decoded") });
                }
            }
        }
        let now = self.clock();
        for command in commands {
            match command {
                Command::Start { key, source, at, offset, duration, rate, gain } => {
                    let pcm = decoded.get(source).expect("checked above").clone();
                    let begin = at.max(now);
                    if let Some(mut previous) = self.voices.remove(key) {
                        if !previous.ended {
                            previous.stop_at = Some(*at);
                            self.tails.push(previous);
                        }
                    }
                    let first = offset.max(0.0) * pcm.rate as f64;
                    let last = match duration {
                        Some(seconds) => (first + seconds.max(0.0) * pcm.rate as f64).min(pcm.frames() as f64),
                        None => pcm.frames() as f64,
                    };
                    self.voices.insert(key.clone(), Voice { pcm, start: begin, position: first, end: last, stop_at: None, gain: Automation::new(*gain), rate: Automation::new(*rate), ended: false });
                }
                Command::Stop { key, at } => {
                    if let Some(mut voice) = self.voices.remove(key) {
                        if !voice.ended {
                            voice.stop_at = Some(*at);
                            self.tails.push(voice);
                        }
                    }
                }
                Command::Ramp { key, param, value, at, end_at, curve } => {
                    if let Some(voice) = self.voices.get_mut(key) {
                        if !voice.ended {
                            match param {
                                Param::Gain => voice.gain.ramp(*value, *at, *end_at, *curve),
                                Param::Rate => voice.rate.ramp(*value, *at, *end_at, *curve),
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Adds the next frames to `out` (interleaved at the mixer's channel count) and moves the clock on.
    pub fn render(&mut self, out: &mut [f32]) {
        let channels = self.channels as usize;
        let frames = out.len() / channels;
        let rate = self.rate as f64;
        for index in 0..frames {
            let time = (self.frame + index as u64) as f64 / rate;
            for voice in self.voices.values_mut().chain(self.tails.iter_mut()) {
                mix(voice, time, rate, &mut out[index * channels..(index + 1) * channels]);
            }
        }
        self.frame += frames as u64;
        self.tails.retain(|voice| !voice.ended);
    }
}

fn mix(voice: &mut Voice, time: f64, out_rate: f64, frame: &mut [f32]) {
    if voice.ended || time < voice.start {
        return;
    }
    if voice.stop_at.is_some_and(|stop| time >= stop) || voice.position >= voice.end {
        voice.ended = true;
        return;
    }
    let source_channels = voice.pcm.channels as usize;
    let base = voice.position.floor() as usize;
    let fraction = (voice.position - base as f64) as f32;
    let gain = voice.gain.at(time) as f32;
    let total = voice.pcm.frames();
    let sample = |channel: usize| -> f32 {
        let a = voice.pcm.samples.get(base * source_channels + channel).copied().unwrap_or(0.0);
        let b = if base + 1 < total { voice.pcm.samples.get((base + 1) * source_channels + channel).copied().unwrap_or(a) } else { a };
        a + (b - a) * fraction
    };
    let outputs = frame.len();
    if source_channels == 1 {
        let value = sample(0) * gain;
        for slot in frame.iter_mut() {
            *slot += value;
        }
    } else if outputs == 1 {
        let mut sum = 0.0;
        for channel in 0..source_channels {
            sum += sample(channel);
        }
        frame[0] += sum / source_channels as f32 * gain;
    } else {
        for (channel, slot) in frame.iter_mut().enumerate() {
            *slot += sample(channel.min(source_channels - 1)) * gain;
        }
    }
    voice.position += voice.rate.at(time).max(0.0) * voice.pcm.rate as f64 / out_rate;
}

/// The bytes of a `data:` URL with a base64 body; `None` for any other text.
pub fn data_url_bytes(source: &str) -> Option<Vec<u8>> {
    let body = source.strip_prefix("data:")?;
    let (head, data) = body.split_once(',')?;
    if !head.ends_with(";base64") {
        return None;
    }
    let mut out = Vec::with_capacity(data.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for byte in data.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        buffer = (buffer << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ones(seconds: f64, rate: u32) -> Pcm {
        Pcm { samples: Arc::new(vec![1.0; (seconds * rate as f64) as usize]), channels: 1, rate }
    }

    fn ramp_up(frames: usize, rate: u32) -> Pcm {
        Pcm { samples: Arc::new((0..frames).map(|i| i as f32).collect()), channels: 1, rate }
    }

    fn decoded(name: &str, pcm: Pcm) -> HashMap<String, Pcm> {
        HashMap::from([(name.to_string(), pcm)])
    }

    fn start(key: &str, at: f64, offset: f64, duration: Option<f64>, rate: f64, gain: f64) -> Command {
        Command::Start { key: key.into(), source: "s".into(), at, offset, duration, rate, gain }
    }

    fn render(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * mixer.channels() as usize];
        mixer.render(&mut out);
        out
    }

    #[test]
    fn a_start_plays_from_its_at_offset_and_duration_with_its_gain_and_is_silent_before() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(&[start("a", 0.5, 0.2, Some(0.3), 1.0, 0.5)], &decoded("s", ramp_up(10, 10))).unwrap();
        let out = render(&mut mixer, 10);
        assert_eq!(out, vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.5, 2.0, 0.0, 0.0]);
    }

    #[test]
    fn a_start_with_no_duration_plays_to_the_end_of_the_source() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(&[start("a", 0.0, 0.7, None, 1.0, 1.0)], &decoded("s", ramp_up(10, 10))).unwrap();
        assert_eq!(render(&mut mixer, 5), vec![7.0, 8.0, 9.0, 0.0, 0.0]);
    }

    #[test]
    fn rate_two_reads_the_source_twice_as_fast_so_duration_over_rate_is_its_length_on_the_clock() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(&[start("a", 0.0, 0.0, Some(0.6), 2.0, 1.0)], &decoded("s", ramp_up(10, 10))).unwrap();
        assert_eq!(render(&mut mixer, 5), vec![0.0, 2.0, 4.0, 0.0, 0.0]);
    }

    #[test]
    fn a_source_at_another_sample_rate_is_resampled_onto_the_output_clock() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(&[start("a", 0.0, 0.0, None, 1.0, 1.0)], &decoded("s", ramp_up(4, 5))).unwrap();
        assert_eq!(render(&mut mixer, 8), vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.0]);
    }

    #[test]
    fn starting_a_playing_key_stops_the_old_voice_at_the_new_at() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(&[start("a", 0.0, 0.0, None, 1.0, 1.0)], &decoded("s", ones(1.0, 10))).unwrap();
        mixer.apply(&[start("a", 0.3, 0.0, None, 1.0, 0.25)], &decoded("s", ones(1.0, 10))).unwrap();
        assert_eq!(render(&mut mixer, 5), vec![1.0, 1.0, 1.0, 0.25, 0.25]);
    }

    #[test]
    fn stop_silences_the_voice_from_its_at_and_forgets_the_key() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(&[start("a", 0.0, 0.0, None, 1.0, 1.0)], &decoded("s", ones(1.0, 10))).unwrap();
        mixer.apply(&[Command::Stop { key: "a".into(), at: 0.2 }], &HashMap::new()).unwrap();
        assert_eq!(render(&mut mixer, 4), vec![1.0, 1.0, 0.0, 0.0]);
        let again = mixer.apply(&[Command::Stop { key: "a".into(), at: 0.5 }], &HashMap::new());
        assert_eq!(again, Err(Failure::invalid("audio/schedule: no voice named a was started")));
    }

    #[test]
    fn a_linear_gain_ramp_anchors_the_value_it_had_and_reaches_its_target_at_end_at() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(
            &[start("a", 0.0, 0.0, None, 1.0, 0.0), Command::Ramp { key: "a".into(), param: Param::Gain, value: 1.0, at: 0.2, end_at: 0.6, curve: Curve::Linear }],
            &decoded("s", ones(1.0, 10)),
        )
        .unwrap();
        let out = render(&mut mixer, 8);
        let expected = [0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0];
        for (got, want) in out.iter().zip(expected) {
            assert!((got - want).abs() < 1e-5, "{out:?}");
        }
    }

    #[test]
    fn a_second_ramp_starts_from_the_first_ramps_target() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(
            &[
                start("a", 0.0, 0.0, None, 1.0, 1.0),
                Command::Ramp { key: "a".into(), param: Param::Gain, value: 0.5, at: 0.0, end_at: 0.0, curve: Curve::Linear },
                Command::Ramp { key: "a".into(), param: Param::Gain, value: 0.0, at: 0.2, end_at: 0.4, curve: Curve::Linear },
            ],
            &decoded("s", ones(1.0, 10)),
        )
        .unwrap();
        let out = render(&mut mixer, 5);
        let expected = [0.5, 0.5, 0.5, 0.25, 0.0];
        for (got, want) in out.iter().zip(expected) {
            assert!((got - want).abs() < 1e-5, "{out:?}");
        }
    }

    #[test]
    fn a_rate_ramp_changes_how_fast_the_source_is_read() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(
            &[start("a", 0.0, 0.0, None, 1.0, 1.0), Command::Ramp { key: "a".into(), param: Param::Rate, value: 2.0, at: 0.2, end_at: 0.2, curve: Curve::Linear }],
            &decoded("s", ramp_up(20, 10)),
        )
        .unwrap();
        assert_eq!(render(&mut mixer, 5), vec![0.0, 1.0, 2.0, 4.0, 6.0]);
    }

    #[test]
    fn control_a_ramp_or_a_stop_of_an_unknown_key_fails_the_ask_and_plays_nothing() {
        let mut mixer = Mixer::new(10, 1);
        let ramp = mixer.apply(
            &[start("a", 0.0, 0.0, None, 1.0, 1.0), Command::Ramp { key: "b".into(), param: Param::Gain, value: 0.0, at: 0.0, end_at: 1.0, curve: Curve::Linear }],
            &decoded("s", ones(1.0, 10)),
        );
        assert_eq!(ramp, Err(Failure::invalid("audio/schedule: no voice named b was started")));
        assert_eq!(render(&mut mixer, 3), vec![0.0, 0.0, 0.0]);
        let stop = mixer.apply(&[Command::Stop { key: "never".into(), at: 0.0 }], &HashMap::new());
        assert_eq!(stop, Err(Failure::invalid("audio/schedule: no voice named never was started")));
    }

    #[test]
    fn control_a_source_that_was_not_decoded_fails_unavailable_and_plays_nothing() {
        let mut mixer = Mixer::new(10, 1);
        let result = mixer.apply(&[start("a", 0.0, 0.0, None, 1.0, 1.0)], &HashMap::new());
        assert_eq!(result, Err(Failure { kind: "unavailable", error: "audio/schedule: s was not decoded".into() }));
        assert_eq!(render(&mut mixer, 2), vec![0.0, 0.0]);
    }

    #[test]
    fn a_voice_that_ended_on_its_own_can_still_be_stopped_once_and_a_ramp_of_it_is_a_no_op() {
        let mut mixer = Mixer::new(10, 1);
        mixer.apply(&[start("a", 0.0, 0.0, Some(0.2), 1.0, 1.0)], &decoded("s", ones(1.0, 10))).unwrap();
        assert_eq!(render(&mut mixer, 4), vec![1.0, 1.0, 0.0, 0.0]);
        mixer.apply(&[Command::Ramp { key: "a".into(), param: Param::Gain, value: 0.0, at: 0.0, end_at: 1.0, curve: Curve::Linear }], &HashMap::new()).unwrap();
        mixer.apply(&[Command::Stop { key: "a".into(), at: 0.5 }], &HashMap::new()).unwrap();
        assert!(mixer.apply(&[Command::Stop { key: "a".into(), at: 0.6 }], &HashMap::new()).is_err());
    }

    #[test]
    fn the_clock_is_the_frames_rendered_over_the_output_rate_and_a_start_in_the_past_begins_now() {
        let mut mixer = Mixer::new(10, 2);
        assert_eq!(mixer.clock(), 0.0);
        render(&mut mixer, 5);
        assert_eq!(mixer.clock(), 0.5);
        mixer.apply(&[start("a", 0.1, 0.0, None, 1.0, 1.0)], &decoded("s", ramp_up(10, 10))).unwrap();
        assert_eq!(render(&mut mixer, 2), vec![0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn a_stereo_source_keeps_its_channels_and_a_mono_output_takes_their_mean() {
        let stereo = Pcm { samples: Arc::new(vec![1.0, 3.0, 1.0, 3.0]), channels: 2, rate: 10 };
        let mut wide = Mixer::new(10, 2);
        wide.apply(&[start("a", 0.0, 0.0, None, 1.0, 1.0)], &decoded("s", stereo.clone())).unwrap();
        assert_eq!(render(&mut wide, 2), vec![1.0, 3.0, 1.0, 3.0]);
        let mut narrow = Mixer::new(10, 1);
        narrow.apply(&[start("a", 0.0, 0.0, None, 1.0, 1.0)], &decoded("s", stereo)).unwrap();
        assert_eq!(render(&mut narrow, 2), vec![2.0, 2.0]);
    }

    #[test]
    fn the_schedule_ask_decodes_start_stop_and_ramp_and_lists_each_source_once() {
        let commands = decode_commands(&[json!([
            { "start": { "key": "in-0", "source": "data:a", "at": 1.5, "offset": 2, "duration": 30, "rate": 1.02, "gain": 0 } },
            { "ramp": { "key": "in-0", "param": "gain", "value": 1, "at": 1.5, "endAt": 9.5, "curve": "linear" } },
            { "start": { "key": "in-1", "source": "data:a", "at": 31.5, "offset": 0, "rate": 1, "gain": 1 } },
            { "stop": { "key": "in-0", "at": 0 } }
        ])])
        .unwrap();
        assert_eq!(commands.len(), 4);
        assert_eq!(commands[0], Command::Start { key: "in-0".into(), source: "data:a".into(), at: 1.5, offset: 2.0, duration: Some(30.0), rate: 1.02, gain: 0.0 });
        assert_eq!(commands[1], Command::Ramp { key: "in-0".into(), param: Param::Gain, value: 1.0, at: 1.5, end_at: 9.5, curve: Curve::Linear });
        assert_eq!(commands[3], Command::Stop { key: "in-0".into(), at: 0.0 });
        assert_eq!(sources(&commands), vec!["data:a".to_string()]);
    }

    #[test]
    fn control_a_malformed_schedule_ask_is_refused_invalid() {
        for bad in [
            json!([{ "start": { "key": "", "source": "s", "at": 0, "offset": 0, "rate": 1, "gain": 1 } }]),
            json!([{ "start": { "key": "a", "at": 0, "offset": 0, "rate": 1, "gain": 1 } }]),
            json!([{ "stop": { "key": "a" } }]),
            json!([{ "ramp": { "key": "a", "param": "pan", "value": 1, "at": 0, "endAt": 1, "curve": "linear" } }]),
            json!([{ "start": {}, "stop": {} }]),
            json!([{ "play": { "key": "a" } }]),
            json!(["start"]),
        ] {
            let result = decode_commands(std::slice::from_ref(&bad));
            assert_eq!(result.as_ref().err().map(|f| f.kind), Some("invalid"), "{bad}");
        }
        assert!(decode_commands(&[]).is_err());
        assert!(decode_commands(&[json!({})]).is_err());
    }

    #[test]
    fn a_base64_data_url_gives_its_bytes_and_any_other_text_gives_none() {
        assert_eq!(data_url_bytes("data:audio/wav;base64,SGVsbG8="), Some(b"Hello".to_vec()));
        assert_eq!(data_url_bytes("data:audio/wav;base64,SGVsbG8h"), Some(b"Hello!".to_vec()));
        assert_eq!(data_url_bytes("https://example.test/a.mp3"), None);
        assert_eq!(data_url_bytes("data:text/plain,hello"), None);
        assert_eq!(data_url_bytes("data:audio/wav;base64,SGV*"), None);
    }
}
