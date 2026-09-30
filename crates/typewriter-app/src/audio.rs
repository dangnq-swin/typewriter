//! Typewriter sounds, mixed on the default output device.

use std::collections::HashMap;
use std::io::Cursor;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source};
use typewriter_core::profile::Sounds;
use typewriter_core::{EraseMode, Event};

use crate::render::feed::FeedMotion;
#[cfg(test)]
use crate::render::feed::SETTLE_SECONDS;
use crate::settings;

/// Clips cut from CC0 sources by `scripts/prepare-sounds.sh`.
macro_rules! clip {
    ($name:literal) => {
        include_bytes!(concat!("../../../assets/sounds/", $name, ".wav"))
    };
}

const KEYS: [&[u8]; 6] = [
    clip!("key-1"),
    clip!("key-2"),
    clip!("key-3"),
    clip!("key-4"),
    clip!("key-5"),
    clip!("key-6"),
];
/// The bundled feed clips' lengths, if they fail to decode.
const FALLBACK_WIND_OUT_SECONDS: f64 = 1.61;
const FALLBACK_WIND_IN_SECONDS: f64 = 6.85;
const BELLS: [&[u8]; 2] = [clip!("bell-1"), clip!("bell-2")];
const ROLLS: [&[u8]; 4] = [
    clip!("roll-1"),
    clip!("roll-2"),
    clip!("roll-3"),
    clip!("roll-4"),
];

/// A hand spinning the knob clicks lighter than a line rolled on.
const WIND_BACK_GAIN: f32 = 0.5;

/// Sounds of one kind. Each may sound only so many times at once: repeats
/// faster than the clip (a held key, a flicked wheel, the blocked clunk at
/// the paper's edge) would otherwise stack up loud.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Voice {
    Key,
    Space,
    Backspace,
    Tab,
    Erase,
    Fluid,
    Return,
    /// The platen ratchet: knob, held Return, winding back.
    Roll,
    Bell,
    Blocked,
    Feed,
    Crumple,
}

impl Voice {
    /// At most this many at once. Keys overlap in fast typing, as on the machine.
    fn most(self) -> usize {
        match self {
            Self::Key => 3,
            Self::Space | Self::Backspace => 2,
            // Keys wait only for the audible part: the next feed may start
            // over the last one's quiet tail.
            Self::Feed => 2,
            _ => 1,
        }
    }
}

/// When each voice's sounds end.
#[derive(Debug, Default)]
struct Voices {
    ends: HashMap<Voice, Vec<Instant>>,
}

impl Voices {
    /// Takes a place for a `voice` sound `length` long from `now`. False if
    /// that voice is already sounding as often as it may.
    fn take(&mut self, voice: Voice, now: Instant, length: Duration) -> bool {
        let ends = self.ends.entry(voice).or_default();
        ends.retain(|&end| end > now);
        if ends.len() >= voice.most() {
            return false;
        }
        ends.push(now + length);
        true
    }
}

pub struct Audio {
    // Keep alive: dropping it stops playback.
    device: MixerDeviceSink,
    keys: Vec<SamplesBuffer>,
    bells: Vec<SamplesBuffer>,
    space: SamplesBuffer,
    backspace: SamplesBuffer,
    tab: SamplesBuffer,
    erase: SamplesBuffer,
    fluid: SamplesBuffer,
    crumple: SamplesBuffer,
    carriage_return: SamplesBuffer,
    rolls: Vec<SamplesBuffer>,
    wind_out: SamplesBuffer,
    wind_in: SamplesBuffer,
    blocked: SamplesBuffer,
    key_variety: Variety,
    bell_variety: Variety,
    roll_variety: Variety,
    voices: Voices,
    /// The machine's audible mechanisms. Silent ones stay silent.
    machine: Sounds,
    settings: settings::Sound,
}

/// Sounds switched off together in the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Keys,
    Bell,
    Platen,
    SheetFeed,
    Corrections,
    Blocked,
}

impl Group {
    fn of(event: Event) -> Option<Self> {
        Some(match event {
            Event::KeyStrike(_) | Event::Space | Event::Backspace | Event::Tab | Event::Freed => {
                Self::Keys
            }
            Event::Bell => Self::Bell,
            Event::CarriageReturn | Event::LineFeed => Self::Platen,
            Event::SheetFed => Self::SheetFeed,
            Event::Erase(_) => Self::Corrections,
            Event::Blocked(_) => Self::Blocked,
            Event::SlipIn | Event::SlipOut | Event::PageEnd => return None,
        })
    }

    fn is_on(self, sound: &settings::Sound) -> bool {
        match self {
            Self::Keys => sound.keys,
            Self::Bell => sound.bell,
            Self::Platen => sound.platen,
            Self::SheetFeed => sound.sheet_feed,
            Self::Corrections => sound.corrections,
            Self::Blocked => sound.blocked,
        }
    }
}

/// Squares the slider: sounds more even than linear.
fn gain(volume_percent: u8) -> f32 {
    let v = f32::from(volume_percent.min(settings::VOLUME_MAX)) / f32::from(settings::VOLUME_MAX);
    v * v
}

impl Audio {
    pub fn new(machine: Sounds, settings: settings::Sound) -> anyhow::Result<Self> {
        let mut device =
            DeviceSinkBuilder::open_default_sink().context("no audio output device")?;
        // Quiet on quit: stopping the sound then is expected.
        device.log_on_drop(false);
        Ok(Self {
            device,
            keys: decode_all(&KEYS)?,
            bells: decode_all(&BELLS)?,
            space: decode(clip!("space"))?,
            backspace: decode(clip!("backspace"))?,
            tab: decode(clip!("tab"))?,
            erase: decode(clip!("erase"))?,
            fluid: decode(clip!("fluid"))?,
            crumple: decode(clip!("crumple"))?,
            carriage_return: decode(clip!("return"))?,
            rolls: decode_all(&ROLLS)?,
            wind_out: decode(clip!("feed-out"))?,
            wind_in: decode(clip!("feed-in"))?,
            blocked: decode(clip!("blocked"))?,
            key_variety: Variety::new(seed()),
            bell_variety: Variety::new(seed().rotate_left(32)),
            roll_variety: Variety::new(seed().rotate_left(16)),
            voices: Voices::default(),
            machine,
            settings,
        })
    }

    pub fn set_machine(&mut self, machine: Sounds) {
        self.machine = machine;
    }

    pub fn set_settings(&mut self, settings: settings::Sound) {
        self.settings = settings;
    }

    fn is_on(&self, group: Group) -> bool {
        !self.settings.mute && self.settings.volume > 0 && group.is_on(&self.settings)
    }

    /// Plays `sound` as `voice`, at `loudness` of the volume, unless that
    /// voice is sounding as often as it may. Every sound goes through here.
    fn start(&mut self, voice: Voice, sound: impl Source + Send + 'static, loudness: f32) {
        let length = sound.total_duration().unwrap_or_default();
        if self.voices.take(voice, Instant::now(), length) {
            self.device
                .mixer()
                .add(sound.amplify(loudness * gain(self.settings.volume)));
        }
    }

    pub fn play(&mut self, event: Event) {
        if !Group::of(event).is_some_and(|group| self.is_on(group)) {
            return;
        }
        let sound = match event {
            Event::KeyStrike(_) => self.key_variety.pick(&self.keys).map(|s| (Voice::Key, s)),
            Event::Bell => self
                .bell_variety
                .pick(&self.bells)
                .map(|s| (Voice::Bell, s)),
            Event::Space => Some((Voice::Space, &self.space)),
            // Typebars pulled apart clack like a backspace.
            Event::Backspace | Event::Freed => Some((Voice::Backspace, &self.backspace)),
            Event::Tab => Some((Voice::Tab, &self.tab)),
            Event::Erase(EraseMode::Fluid) => Some((Voice::Fluid, &self.fluid)),
            // Digital: nothing to hear.
            Event::Erase(EraseMode::Delete) => None,
            Event::Erase(_) => Some((Voice::Erase, &self.erase)),
            // Silent: strikes through the slip sound like any strike.
            Event::SlipIn | Event::SlipOut => None,
            Event::CarriageReturn if self.machine.carriage_return => {
                Some((Voice::Return, &self.carriage_return))
            }
            Event::LineFeed if self.machine.line_feed => self
                .roll_variety
                .pick(&self.rolls)
                .map(|s| (Voice::Roll, s)),
            Event::CarriageReturn | Event::LineFeed => None,
            Event::SheetFed => {
                // One source, out then in: a delay in the mixer is gapless,
                // whatever the frame timing.
                let after = self.wind_out.total_duration().unwrap_or_default();
                let feed = self.wind_out.clone().mix(self.wind_in.clone().delay(after));
                self.start(Voice::Feed, feed, 1.0);
                None
            }
            Event::Blocked(_) => Some((Voice::Blocked, &self.blocked)),
            Event::PageEnd => None,
        };
        if let Some((voice, sound)) = sound {
            let sound = sound.clone();
            self.start(voice, sound, 1.0);
        }
    }

    pub fn play_crumple(&mut self) {
        if self.is_on(Group::SheetFeed) {
            self.start(Voice::Crumple, self.crumple.clone(), 1.0);
        }
    }

    /// A softer ratchet click for a sheet winding back.
    pub fn play_wind_back_click(&mut self) {
        if !self.machine.line_feed || !self.is_on(Group::Platen) {
            return;
        }
        if let Some(click) = self.roll_variety.pick(&self.rolls).cloned() {
            self.start(Voice::Roll, click, WIND_BACK_GAIN);
        }
    }

    /// Just the wind-in, for a project's first sheet.
    pub fn play_wind_in(&mut self) {
        if self.is_on(Group::SheetFeed) {
            self.start(Voice::Feed, self.wind_in.clone(), 1.0);
        }
    }
}

/// The feed's motion, timed from its clips (sound device or not): wind out
/// during the clicks, wind in with the knob turns.
pub fn sheet_feed_motion() -> FeedMotion {
    let wind_out = decode(clip!("feed-out"))
        .ok()
        .and_then(|clip| clip.total_duration())
        .map_or(FALLBACK_WIND_OUT_SECONDS, |d| d.as_secs_f64());
    let wind_in = match Decoder::new(Cursor::new(clip!("feed-in"))) {
        Ok(feed) => {
            let (channels, rate) = (feed.channels(), feed.sample_rate());
            let samples: Vec<f32> = feed.collect();
            FeedMotion::from_sound(&samples, usize::from(channels.get()), rate.get())
        }
        Err(err) => {
            eprintln!("sheet feed sound unreadable, winding evenly: {err}");
            FeedMotion::even(FALLBACK_WIND_IN_SECONDS)
        }
    };
    wind_in.after_wind_out(wind_out)
}

/// Decode up front: a key press then only copies samples.
fn decode(wav: &'static [u8]) -> anyhow::Result<SamplesBuffer> {
    let source = Decoder::new(Cursor::new(wav)).context("bundled sound is not a valid WAV")?;
    let (channels, rate) = (source.channels(), source.sample_rate());
    Ok(SamplesBuffer::new(
        channels,
        rate,
        source.collect::<Vec<_>>(),
    ))
}

fn decode_all(clips: &[&'static [u8]]) -> anyhow::Result<Vec<SamplesBuffer>> {
    clips.iter().map(|clip| decode(clip)).collect()
}

fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::from(d.subsec_nanos()))
}

/// Picks sound variants at random, never the same twice in a row.
struct Variety {
    state: u64,
    last: Option<usize>,
}

impl Variety {
    fn new(seed: u64) -> Self {
        // Never zero: xorshift sticks there.
        Self {
            state: seed | 1,
            last: None,
        }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        // Safe cast: the remainder is below `len`.
        let mut i = (self.next() % len as u64) as usize;
        if Some(i) == self.last {
            i = (i + 1) % len;
        }
        self.last = Some(i);
        Some(i)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        self.index(items.len()).and_then(|i| items.get(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variety_never_repeats_and_covers_all() {
        let mut v = Variety::new(42);
        let mut seen = [false; 6];
        let mut last = None;
        for _ in 0..200 {
            let i = v.index(6).unwrap();
            assert_ne!(Some(i), last);
            seen[i] = true;
            last = Some(i);
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn every_sounding_event_belongs_to_a_group() {
        let mut sound = settings::Sound {
            keys: false,
            ..settings::Sound::default()
        };
        assert!(!Group::of(Event::Space).unwrap().is_on(&sound));
        assert!(Group::of(Event::Bell).unwrap().is_on(&sound));
        sound.corrections = false;
        assert!(
            !Group::of(Event::Erase(EraseMode::Fluid))
                .unwrap()
                .is_on(&sound)
        );
        assert_eq!(Group::of(Event::SlipIn), None);
    }

    #[test]
    fn a_voice_sounds_only_so_often_at_once() {
        let mut voices = Voices::default();
        let start = Instant::now();
        let clip = Duration::from_millis(140);
        let at = |ms| start + Duration::from_millis(ms);
        // The blocked clunk at the paper's edge, every frame.
        assert!(voices.take(Voice::Blocked, at(0), clip));
        assert!(!voices.take(Voice::Blocked, at(16), clip));
        assert!(!voices.take(Voice::Blocked, at(139), clip));
        assert!(
            voices.take(Voice::Blocked, at(140), clip),
            "the last has ended"
        );
        // A held key: three at once, then a gap until one ends.
        let key = Duration::from_millis(280);
        for ms in [0, 33, 66] {
            assert!(voices.take(Voice::Key, at(ms), key));
        }
        assert!(!voices.take(Voice::Key, at(100), key));
        assert!(voices.take(Voice::Key, at(280), key));
        // Voices don't take each other's places.
        assert!(voices.take(Voice::Roll, at(100), Duration::from_millis(100)));
        // Feeding again once the keys are back, the last feed still ringing.
        let feed = Duration::from_millis(8460);
        assert!(voices.take(Voice::Feed, at(0), feed));
        assert!(voices.take(Voice::Feed, at(4000), feed));
    }

    #[test]
    fn volume_is_quieter_than_its_slider() {
        assert_eq!(gain(100), 1.0);
        assert_eq!(gain(0), 0.0);
        assert!((gain(50) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn variety_handles_one_and_none() {
        let mut v = Variety::new(7);
        assert_eq!(v.index(1), Some(0));
        assert_eq!(v.index(1), Some(0));
        assert_eq!(v.index(0), None);
    }

    #[test]
    fn sheet_feed_ends_once_the_sheet_has_settled() {
        let m = sheet_feed_motion();
        let seconds = m.duration();
        // Not the full 8.46 s of sound: its end is quiet.
        assert!(
            seconds > 6.0 && seconds < 8.46 - SETTLE_SECONDS,
            "{seconds}"
        );
        assert_eq!(m.progress(seconds - SETTLE_SECONDS), 1.0);
    }

    #[test]
    fn sheet_winds_in_with_the_knob_turns() {
        let m = sheet_feed_motion();
        // The finished sheet winds out steadily during the clicks, before
        // the new one moves.
        assert!(
            m.roll_out(0.5)
                .is_some_and(|r| (r - 0.5 / 1.61).abs() < 0.01)
        );
        assert_eq!(m.roll_out(1.65), None);
        assert_eq!(m.progress(1.6), 0.0);
        // Resting in the pause before the ratchet run.
        assert!((m.progress(5.11) - m.progress(5.26)).abs() < 0.01);
        assert!(m.progress(5.16) > 0.3 && m.progress(5.16) < 0.9);
        assert_eq!(m.progress(7.9), 1.0);
        assert_eq!(m.pointer_opacity(m.duration() - SETTLE_SECONDS), 0.0);
    }

    #[test]
    fn bundled_clips_decode() {
        assert_eq!(decode_all(&KEYS).unwrap().len(), 6);
        assert_eq!(decode_all(&BELLS).unwrap().len(), 2);
        assert_eq!(decode_all(&ROLLS).unwrap().len(), 4);
        let others: [&[u8]; 10] = [
            clip!("fluid"),
            clip!("crumple"),
            clip!("space"),
            clip!("backspace"),
            clip!("tab"),
            clip!("erase"),
            clip!("return"),
            clip!("feed-out"),
            clip!("feed-in"),
            clip!("blocked"),
        ];
        assert_eq!(decode_all(&others).unwrap().len(), 10);
    }
}
