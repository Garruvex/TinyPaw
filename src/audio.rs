//! Tap playback. The output device opens when TinyPaw switches On and closes
//! on Off, so the idle process holds no audio stream. Starter sounds are
//! synthesized once; an imported WAV is decoded up front. If audio is
//! unavailable, pointing keeps working.

use std::fs::File;
use std::io::BufReader;
use std::num::NonZero;
use std::path::{Path, PathBuf};

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source};

const RATE: u32 = 44_100;

/// A predecoded clip.
#[derive(Clone)]
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
    (0..n).map(|i| f(i as f32 / RATE as f32, &mut seed)).collect()
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
    knock: Clip,
    pop: Clip,
    bonk: Clip,
    custom: Option<Clip>,
}

impl Audio {
    pub fn new() -> Self {
        let custom = custom_path()
            .filter(|p| p.exists())
            .and_then(|p| decode_wav(&p).ok());
        Self {
            sink: None,
            knock: knock(),
            pop: pop(),
            bonk: bonk(),
            custom,
        }
    }

    pub fn has_custom(&self) -> bool {
        self.custom.is_some()
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

    pub fn close(&mut self) {
        self.sink = None;
    }

    fn clip(&self, id: &str) -> &Clip {
        match id {
            "pop" => &self.pop,
            "bonk" => &self.bonk,
            "custom" => self.custom.as_ref().unwrap_or(&self.knock),
            // "pack": the placeholder paw ships no tap of its own.
            _ => &self.knock,
        }
    }

    pub fn play(&mut self, id: &str, volume: f32) {
        self.open();
        if let Some(sink) = &self.sink {
            sink.mixer().add(self.clip(id).source(volume.clamp(0.0, 1.0)));
        }
    }
}
