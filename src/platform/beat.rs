//! Live beat following (trial). A WASAPI loopback stream of the default
//! output feeds `mascot::beat::Tracker` on its own thread; the audio
//! callback only downmixes and hands chunks over, never waiting. Runs only
//! while Follow beat is on and music response is running, and stops when
//! dropped. Loopback hears everything, TinyPaw included, so the app holds
//! off onset detection while it plays its own sounds.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::mascot::beat::{Sensitivity, Tracker};

/// With no audio for this long the stream is rebuilt, which follows a change
/// of default output.
const STALL: Duration = Duration::from_secs(3);

#[derive(Default)]
struct Shared {
    last_beat: Mutex<Option<Instant>>,
    /// Beat period in seconds as f32 bits; 0 while no beat is followed.
    period: AtomicU32,
    hold_until: Mutex<Option<Instant>>,
    sensitivity: AtomicU8,
    available: AtomicBool,
}

pub struct Follower {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn encode(s: Sensitivity) -> u8 {
    match s {
        Sensitivity::Low => 0,
        Sensitivity::Normal => 1,
        Sensitivity::High => 2,
    }
}

fn decode(v: u8) -> Sensitivity {
    match v {
        0 => Sensitivity::Low,
        2 => Sensitivity::High,
        _ => Sensitivity::Normal,
    }
}

impl Follower {
    pub fn start(sensitivity: Sensitivity) -> Self {
        let shared = Arc::new(Shared::default());
        shared.sensitivity.store(encode(sensitivity), Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (shared, stop) = (shared.clone(), stop.clone());
            std::thread::Builder::new()
                .name("tinypaw-beat".into())
                .spawn(move || run(&shared, &stop))
                .ok()
        };
        Self {
            shared,
            stop,
            thread,
        }
    }

    pub fn set_sensitivity(&self, s: Sensitivity) {
        self.shared.sensitivity.store(encode(s), Ordering::Relaxed);
    }

    /// Ignore onsets while TinyPaw plays a sound of this length.
    pub fn hold_off(&self, seconds: f32) {
        let until = Instant::now() + Duration::from_secs_f32(seconds.clamp(0.0, 10.0) + 0.05);
        let mut h = self.shared.hold_until.lock().unwrap_or_else(|e| e.into_inner());
        *h = Some(h.map_or(until, |current| current.max(until)));
    }

    pub fn available(&self) -> bool {
        self.shared.available.load(Ordering::Relaxed)
    }

    pub fn bpm(&self) -> Option<f32> {
        let p = f32::from_bits(self.shared.period.load(Ordering::Relaxed));
        (p > 0.0).then(|| 60.0 / p)
    }

    /// Where we are between beats (0 on the beat), shifted by `offset`
    /// seconds; `None` while no beat is being followed.
    pub fn phase(&self, offset: f32) -> Option<f32> {
        let period = f32::from_bits(self.shared.period.load(Ordering::Relaxed));
        let last = (*self.shared.last_beat.lock().unwrap_or_else(|e| e.into_inner()))?;
        if period <= 0.0 {
            return None;
        }
        let since = last.elapsed().as_secs_f32();
        // Beyond two missed beats, stop pretending.
        if since > period * 2.5 {
            return None;
        }
        Some(((since - offset) / period).rem_euclid(1.0))
    }
}

impl Drop for Follower {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(shared: &Shared, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match open_loopback() {
            Some((stream, rate, rx)) => {
                shared.available.store(true, Ordering::Relaxed);
                follow(shared, stop, &rx, rate);
                drop(stream);
            }
            None => {
                shared.available.store(false, Ordering::Relaxed);
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        shared.period.store(0, Ordering::Relaxed);
    }
}

/// Analyses chunks until stopped or the audio stalls.
fn follow(shared: &Shared, stop: &AtomicBool, rx: &Receiver<Vec<f32>>, rate: u32) {
    let mut tracker = Tracker::new(rate, decode(shared.sensitivity.load(Ordering::Relaxed)));
    let mut heard = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let chunk = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => chunk,
            Err(RecvTimeoutError::Timeout) if heard.elapsed() < STALL => continue,
            Err(_) => return,
        };
        heard = Instant::now();
        tracker.set_sensitivity(decode(shared.sensitivity.load(Ordering::Relaxed)));
        if let Some(until) = *shared.hold_until.lock().unwrap_or_else(|e| e.into_inner()) {
            let left = until.saturating_duration_since(Instant::now()).as_secs_f64();
            if left > 0.0 {
                tracker.hold_off(left);
            }
        }
        tracker.push(&chunk);
        if !tracker.take_beats().is_empty() {
            *shared.last_beat.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
        }
        let period = tracker.bpm().map_or(0.0, |bpm| 60.0 / bpm);
        shared.period.store(period.to_bits(), Ordering::Relaxed);
    }
}

/// A loopback capture of the default output, delivering mono chunks.
fn open_loopback() -> Option<(cpal::Stream, u32, Receiver<Vec<f32>>)> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let rate = supported.sample_rate();
    let channels = usize::from(supported.channels()).max(1);
    let config = supported.config();
    let (tx, rx): (SyncSender<Vec<f32>>, Receiver<Vec<f32>>) = sync_channel(64);
    let err = |e| eprintln!("tinypaw: beat capture: {e}");
    // Downmix and hand over; if analysis falls behind, drop the chunk
    // rather than block the audio thread.
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mono = data
                        .chunks(channels)
                        .map(|f| f.iter().sum::<f32>() / channels as f32)
                        .collect();
                    let _ = tx.try_send(mono);
                },
                err,
                None,
            )
            .ok()?,
        cpal::SampleFormat::I16 => device
            .build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mono = data
                        .chunks(channels)
                        .map(|f| f.iter().map(|s| f32::from(*s) / 32768.0).sum::<f32>() / channels as f32)
                        .collect();
                    let _ = tx.try_send(mono);
                },
                err,
                None,
            )
            .ok()?,
        _ => return None,
    };
    stream.play().ok()?;
    Some((stream, rate, rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opens a real loopback stream on this PC's default output.
    #[test]
    #[ignore = "opens the real default audio output"]
    fn loopback_opens_on_the_default_output() {
        let f = Follower::start(Sensitivity::Normal);
        std::thread::sleep(Duration::from_millis(1500));
        println!("loopback available {}, bpm {:?}", f.available(), f.bpm());
        assert!(f.available());
    }
}
