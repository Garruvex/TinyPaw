//! Windows playback meter for music response. A background thread sums the
//! peak meter of every other app's active audio session on the default
//! output, so TinyPaw's own taps and line sounds never count, and no audio
//! samples are captured or kept. It runs only while music response is on
//! and needed, and stops when dropped.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::mascot::music::Envelope;

/// How often the meters are read.
const TICK: Duration = Duration::from_millis(50);
/// How often the default output is looked up again, so a device change is
/// followed without any notification plumbing.
const REFRESH: Duration = Duration::from_secs(2);

/// What the meter can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    Silent,
    Playing,
    /// No output device or the meters couldn't be read.
    Unavailable,
}

pub struct Meter {
    level: Arc<AtomicU32>,
    state: Arc<AtomicU8>,
    started: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Meter {
    pub fn start() -> Self {
        let level = Arc::new(AtomicU32::new(0));
        let state = Arc::new(AtomicU8::new(State::Starting as u8));
        let started = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (level, state, started, stop) =
                (level.clone(), state.clone(), started.clone(), stop.clone());
            std::thread::Builder::new()
                .name("tinypaw-music-meter".into())
                .spawn(move || run(&level, &state, &started, &stop))
                .ok()
        };
        if thread.is_none() {
            state.store(State::Unavailable as u8, Ordering::Relaxed);
        }
        Self {
            level,
            state,
            started,
            stop,
            thread,
        }
    }

    /// The smoothed level, 0–1 (0 while silent).
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    pub fn state(&self) -> State {
        match self.state.load(Ordering::Relaxed) {
            0 => State::Starting,
            1 => State::Silent,
            2 => State::Playing,
            _ => State::Unavailable,
        }
    }

    /// Whether music started since the last call.
    pub fn take_started(&self) -> bool {
        self.started.swap(false, Ordering::Relaxed)
    }
}

impl Drop for Meter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(level: &AtomicU32, state: &AtomicU8, started: &AtomicBool, stop: &AtomicBool) {
    use ::windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    // SAFETY: this thread owns its COM apartment for its whole life.
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let own = std::process::id();
    let mut envelope = Envelope::default();
    let mut sessions = None;
    let mut refreshed = Instant::now() - REFRESH;
    let mut last = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if sessions.is_none() || refreshed.elapsed() >= REFRESH {
            let next = com.then(session_manager).flatten();
            if next.is_none() && sessions.is_some() {
                envelope.reset();
            }
            sessions = next;
            refreshed = Instant::now();
        }
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32();
        last = now;
        match sessions.as_ref().and_then(|m| peak(m, own)) {
            Some(p) => {
                if envelope.step(p, dt) {
                    started.store(true, Ordering::Relaxed);
                }
                let s = if envelope.playing() { State::Playing } else { State::Silent };
                state.store(s as u8, Ordering::Relaxed);
            }
            None => {
                envelope.reset();
                sessions = None;
                state.store(State::Unavailable as u8, Ordering::Relaxed);
            }
        }
        level.store(envelope.level().to_bits(), Ordering::Relaxed);
        std::thread::sleep(TICK);
    }
    level.store(0f32.to_bits(), Ordering::Relaxed);
    if com {
        // SAFETY: balances the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
}

/// The default output's session manager.
fn session_manager() -> Option<::windows::Win32::Media::Audio::IAudioSessionManager2> {
    use ::windows::Win32::Media::Audio::{
        IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
    };
    use ::windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
    // SAFETY: plain COM calls on this thread's initialised apartment.
    unsafe {
        let devices: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = devices.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None).ok()
    }
}

/// The loudest active session's peak, leaving out TinyPaw itself, or
/// `None` if the sessions can't be read (the device may have gone).
fn peak(manager: &::windows::Win32::Media::Audio::IAudioSessionManager2, own: u32) -> Option<f32> {
    use ::windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
    use ::windows::Win32::Media::Audio::{AudioSessionStateActive, IAudioSessionControl2};
    use ::windows::core::Interface;
    // SAFETY: plain COM calls on this thread's initialised apartment.
    unsafe {
        let list = manager.GetSessionEnumerator().ok()?;
        let mut loudest = 0.0f32;
        for i in 0..list.GetCount().ok()? {
            let Ok(session) = list.GetSession(i) else { continue };
            if session.GetState().ok() != Some(AudioSessionStateActive) {
                continue;
            }
            let Ok(control) = session.cast::<IAudioSessionControl2>() else { continue };
            if control.GetProcessId().ok() == Some(own) {
                continue;
            }
            if let Ok(meter) = session.cast::<IAudioMeterInformation>()
                && let Ok(p) = meter.GetPeakValue()
            {
                loudest = loudest.max(p);
            }
        }
        Some(loudest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads this PC's real playback meters for a moment; it needs an audio
    /// device, so it isn't part of the regular run.
    #[test]
    #[ignore = "reads the real default audio output"]
    fn the_meter_reads_the_default_output() {
        let m = Meter::start();
        std::thread::sleep(Duration::from_millis(1500));
        let state = m.state();
        println!("meter state {state:?}, level {:.3}", m.level());
        assert_ne!(state, State::Starting, "the meter reported within 1.5 s");
        drop(m);
    }
}
