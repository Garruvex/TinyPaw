//! Playback in two categories. Pointer taps mix freely; the character voice
//! is one stoppable player, so a new reaction replaces the last instead of
//! piling up. The output device opens on demand and is released when
//! neither category needs it, so an idle process holds no audio stream.
//! Starter sounds are synthesized once; an imported WAV is decoded up
//! front. If audio is unavailable, pointing and the character keep working.

use std::fs::File;
use std::io::BufReader;
use std::num::NonZero;
use std::path::{Path, PathBuf};

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

const RATE: u32 = 44_100;

// Embedded in the executable so installed builds need no external sound folder.
const BUNDLED: &[(&str, &[u8])] = &[
    ("duck-toy", include_bytes!("../assets/sounds/duck-toy-sound.wav")),
    ("mac-quack", include_bytes!("../assets/sounds/mac-quack.wav")),
    ("bonk-clip", include_bytes!("../assets/sounds/bonk_7zPAD7C.wav")),
    ("dry-fart", include_bytes!("../assets/sounds/dry-fart.wav")),
];

/// A predecoded clip.
#[derive(Debug, Clone)]
pub struct Clip {
    channels: u16,
    rate: u32,
    samples: std::sync::Arc<[f32]>,
}

impl Clip {
    fn mono(samples: Vec<f32>) -> Self {
        Self {
            channels: 1,
            rate: RATE,
            samples: samples.into(),
        }
    }

    /// Length in seconds.
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / f32::from(self.channels.max(1)) / self.rate.max(1) as f32
    }

    fn source(&self, volume: f32) -> impl Source + Send + 'static {
        SamplesBuffer::new(
            NonZero::new(self.channels).unwrap_or(NonZero::<u16>::MIN),
            NonZero::new(self.rate).unwrap_or(NonZero::<u32>::MIN),
            self.samples.to_vec(),
        )
        .amplify(volume)
    }
}

/// Where an imported tap WAV is kept so it survives moving the original.
pub fn custom_path() -> Option<PathBuf> {
    crate::preferences::data_dir().map(|d| d.join("sounds").join("custom.wav"))
}

pub fn decode_wav(path: &Path) -> Result<Clip, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let decoder = Decoder::try_from(BufReader::new(file)).map_err(|e| e.to_string())?;
    decode_clip(decoder)
}

fn decode_clip(decoder: impl Source<Item = f32>) -> Result<Clip, String> {
    let channels = decoder.channels().get();
    let rate = decoder.sample_rate().get();
    // Cap at five seconds: a tap sound should be short.
    let max = rate as usize * channels as usize * 5;
    let samples: Vec<f32> = decoder.take(max).collect();
    if samples.is_empty() {
        return Err("the file has no audio".into());
    }
    Ok(Clip {
        channels,
        rate,
        samples: samples.into(),
    })
}

/// Copies a user's WAV into app storage after checking it decodes.
pub fn import_wav(from: &Path) -> Result<Clip, String> {
    let clip = decode_wav(from)?;
    let to = custom_path().ok_or("no per-user data folder")?;
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::copy(from, &to).map_err(|e| e.to_string())?;
    Ok(clip)
}

fn envelope(t: f32, attack: f32, decay: f32) -> f32 {
    if t < attack {
        t / attack
    } else {
        (-(t - attack) / decay).exp()
    }
}

/// Small deterministic noise source so clips are identical every launch.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn normalize(mut s: Vec<f32>, peak: f32) -> Vec<f32> {
    let max = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if max > 0.0 {
        s.iter_mut().for_each(|v| *v *= peak / max);
    }
    s
}

fn synth(len_s: f32, f: impl Fn(f32, &mut u32) -> f32) -> Vec<f32> {
    let n = (len_s * RATE as f32) as usize;
    let mut seed = 0x9E37_79B9;
    (0..n)
        .map(|i| f(i as f32 / RATE as f32, &mut seed))
        .collect()
}

/// A short, hollow pointer knock against a chalkboard.
fn knock() -> Clip {
    let tau = std::f32::consts::TAU;
    let s = synth(0.16, |t, seed| {
        let body = (tau * 190.0 * t).sin() * envelope(t, 0.002, 0.045)
            + 0.55 * (tau * 470.0 * t).sin() * envelope(t, 0.001, 0.02)
            + 0.25 * (tau * 1150.0 * t).sin() * envelope(t, 0.0005, 0.008);
        let click = noise(seed) * envelope(t, 0.0003, 0.003) * 0.6;
        body + click
    });
    Clip::mono(normalize(s, 0.5))
}

/// A soft, quick pop with little tail.
fn pop() -> Clip {
    let tau = std::f32::consts::TAU;
    let s = synth(0.09, |t, _| {
        // Downward sweep 950 → 380 Hz; phase is the integral of frequency.
        let k = 570.0 / 0.05;
        let phase = 950.0 * t - 0.5 * k * t.min(0.05).powi(2) - 570.0 * (t - 0.05).max(0.0);
        (tau * phase).sin() * envelope(t, 0.002, 0.022)
    });
    Clip::mono(normalize(s, 0.42))
}

/// A low, springy comic tap.
fn bonk() -> Clip {
    let tau = std::f32::consts::TAU;
    let s = synth(0.22, |t, _| {
        let wobble = 1.0 + 0.06 * (tau * 22.0 * t).sin();
        let phase = (150.0 * t - 90.0 * t * t) * wobble;
        ((tau * phase).sin() + 0.3 * (tau * 2.0 * phase).sin()) * envelope(t, 0.003, 0.07)
    });
    Clip::mono(normalize(s, 0.55))
}

pub struct Audio {
    sink: Option<MixerDeviceSink>,
    /// The character voice: at most one clip at a time.
    voice: Option<Player>,
    knock: Clip,
    pop: Clip,
    bonk: Clip,
    bundled: Vec<(&'static str, Clip)>,
    custom: Option<Clip>,
    pack: Option<Clip>,
}

impl Audio {
    pub fn new() -> Self {
        let custom = custom_path()
            .filter(|p| p.exists())
            .and_then(|p| decode_wav(&p).ok());
        Self {
            sink: None,
            voice: None,
            knock: knock(),
            pop: pop(),
            bonk: bonk(),
            bundled: BUNDLED
                .iter()
                .filter_map(|&(id, bytes)| {
                    match Decoder::try_from(std::io::Cursor::new(bytes))
                        .map_err(|e| e.to_string())
                        .and_then(decode_clip)
                    {
                        Ok(clip) => Some((id, clip)),
                        Err(e) => {
                            eprintln!("tinypaw: could not decode bundled sound {id}: {e}");
                            None
                        }
                    }
                })
                .collect(),
            custom,
            pack: None,
        }
    }

    pub fn has_custom(&self) -> bool {
        self.custom.is_some()
    }

    pub fn set_pack(&mut self, clip: Option<Clip>) {
        self.pack = clip;
    }

    pub fn set_custom(&mut self, clip: Clip) {
        self.custom = Some(clip);
    }

    /// Opens the default output device. Failure leaves TinyPaw silent.
    pub fn open(&mut self) {
        if self.sink.is_none() {
            match DeviceSinkBuilder::open_default_sink() {
                Ok(mut sink) => {
                    sink.log_on_drop(false);
                    self.sink = Some(sink);
                }
                Err(e) => eprintln!("tinypaw: audio unavailable: {e}"),
            }
        }
    }

    /// Tap playback is done (TinyPaw switched Off). A character voice that
    /// is still playing keeps the device until it ends.
    pub fn close(&mut self) {
        if !self.voice_playing() {
            self.voice = None;
            self.sink = None;
        }
    }

    /// Releases the device once the voice has finished, unless taps still
    /// need it (`taps`: TinyPaw is On).
    pub fn release_idle(&mut self, taps: bool) {
        if self.voice.as_ref().is_some_and(Player::empty) {
            self.voice = None;
        }
        if !taps && self.voice.is_none() {
            self.sink = None;
        }
    }

    /// A bundled clip by its exact ID; the pointer's `pack` and `custom`
    /// aliases and the synthesized taps are not character sounds.
    pub fn bundled(&self, id: &str) -> Option<Clip> {
        self.bundled
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, clip)| clip.clone())
    }

    /// Plays `clip` as the character voice, stopping any voice already
    /// playing first. Returns whether an output device was available.
    pub fn play_voice(&mut self, clip: &Clip, volume: f32) -> bool {
        self.stop_voice();
        self.open();
        let Some(sink) = &self.sink else {
            return false;
        };
        let player = Player::connect_new(sink.mixer());
        player.append(clip.source(volume.clamp(0.0, 1.0)));
        self.voice = Some(player);
        true
    }

    pub fn stop_voice(&mut self) {
        if let Some(v) = self.voice.take() {
            v.stop();
        }
    }

    pub fn voice_playing(&self) -> bool {
        self.voice.as_ref().is_some_and(|v| !v.empty())
    }

    fn clip(&self, id: &str) -> &Clip {
        match id {
            "pack" => self.pack.as_ref().unwrap_or(&self.knock),
            "pop" => &self.pop,
            "bonk" => &self.bonk,
            "custom" => self.custom.as_ref().unwrap_or(&self.knock),
            _ => self
                .bundled
                .iter()
                .find(|(key, _)| *key == id)
                .map(|(_, clip)| clip)
                .unwrap_or(&self.knock),
        }
    }

    pub fn play(&mut self, id: &str, volume: f32) {
        self.open();
        if let Some(sink) = &self.sink {
            sink.mixer()
                .add(self.clip(id).source(volume.clamp(0.0, 1.0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_wavs_decode_and_are_selectable() {
        for &(id, bytes) in BUNDLED {
            let clip = decode_clip(Decoder::try_from(std::io::Cursor::new(bytes)).unwrap())
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(clip.samples.iter().all(|s| s.is_finite()), "{id}");
            assert!(clip.samples.iter().any(|s| s.abs() > 0.001), "{id} is silent");
            assert!(crate::features::sound::CHOICES.contains(&id));
            assert_ne!(crate::features::sound::label(id), "Unknown");
        }
    }
}
