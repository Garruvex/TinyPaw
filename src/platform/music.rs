//! Windows playback meter for music response. A background thread reads the
//! peak meter of other apps' active audio sessions on the default output
//! (all of them, or one chosen app), so TinyPaw's own taps and line sounds
//! never count and no audio samples are captured or kept. With an input
//! device chosen instead it opens that device and keeps only its peak. It
//! runs only while music response is on and needed, and stops when dropped.
//! It also lists the apps playing and the input devices, for the picker.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::mascot::music::{Envelope, Source, fall, meter};

/// How often the meters are read.
const TICK: Duration = Duration::from_millis(50);
/// How often the default output and the device list are looked up again,
/// so a device change is followed without any notification plumbing.
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

/// An app with an active audio session.
#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub exe: String,
    /// How full its meter is (0–1, decibel scale), for the picker.
    pub shown: f32,
    /// The loudest session's process.
    pub pid: u32,
    /// Every process with a session for this app.
    pub pids: Vec<u32>,
}

#[derive(Default)]
struct Shared {
    level: AtomicU32,
    /// Meter readings (0–1, decibel scale): the chosen source, and all apps.
    heard: AtomicU32,
    all: AtomicU32,
    state: AtomicU8,
    started: AtomicBool,
    stop: AtomicBool,
    source: Mutex<Source>,
    apps: Mutex<Vec<App>>,
    inputs: Mutex<Vec<String>>,
}

pub struct Meter {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Meter {
    pub fn start(source: Source) -> Self {
        let shared = Arc::new(Shared::default());
        *lock(&shared.source) = source;
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("tinypaw-music-meter".into())
                .spawn(move || run(&shared))
                .ok()
        };
        if thread.is_none() {
            shared
                .state
                .store(State::Unavailable as u8, Ordering::Relaxed);
        }
        Self { shared, thread }
    }

    pub fn set_source(&self, source: &Source) {
        let mut s = lock(&self.shared.source);
        if *s != *source {
            *s = source.clone();
        }
    }

    /// How full the chosen source's meter is (0–1, decibel scale): what's
    /// heard right now, before any gain or "playing" decision.
    pub fn heard(&self) -> f32 {
        f32::from_bits(self.shared.heard.load(Ordering::Relaxed))
    }

    /// The meter for all apps together.
    pub fn all_apps(&self) -> f32 {
        f32::from_bits(self.shared.all.load(Ordering::Relaxed))
    }

    /// The smoothed level, 0–1 (0 while silent).
    pub fn level(&self) -> f32 {
        f32::from_bits(self.shared.level.load(Ordering::Relaxed))
    }

    pub fn state(&self) -> State {
        match self.shared.state.load(Ordering::Relaxed) {
            0 => State::Starting,
            1 => State::Silent,
            2 => State::Playing,
            _ => State::Unavailable,
        }
    }

    /// Whether music started since the last call.
    pub fn take_started(&self) -> bool {
        self.shared.started.swap(false, Ordering::Relaxed)
    }

    /// Other apps with audio sessions on the default output right now.
    pub fn apps(&self) -> Vec<App> {
        lock(&self.shared.apps).clone()
    }

    /// App `exe`'s processes with sessions, loudest first; empty if it
    /// isn't playing.
    pub fn app_pids(&self, exe: &str) -> Vec<u32> {
        lock(&self.shared.apps)
            .iter()
            .find(|a| a.exe.eq_ignore_ascii_case(exe))
            .map(|a| {
                let mut pids = vec![a.pid];
                pids.extend(a.pids.iter().filter(|p| **p != a.pid));
                pids
            })
            .unwrap_or_default()
    }

    /// Input devices (microphones, line-ins) by name.
    pub fn inputs(&self) -> Vec<String> {
        lock(&self.shared.inputs).clone()
    }
}

impl Drop for Meter {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(shared: &Shared) {
    use ::windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    // SAFETY: this thread owns its COM apartment for its whole life.
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let own = std::process::id();
    let mut names = Names::default();
    let mut envelope = Envelope::default();
    let mut sessions = None;
    let mut input: Option<(String, InputPeak)> = None;
    // Falling meter readings per app (by lower-case name), all apps and
    // the chosen source.
    let mut shown: HashMap<String, f32> = HashMap::new();
    let (mut shown_all, mut shown_heard) = (0.0f32, 0.0f32);
    let mut refreshed = Instant::now() - REFRESH;
    let mut last = Instant::now();
    shared.state.store(State::Starting as u8, Ordering::Relaxed);
    while !shared.stop.load(Ordering::Relaxed) {
        let source = lock(&shared.source).clone();
        if sessions.is_none() || refreshed.elapsed() >= REFRESH {
            let next = com.then(session_manager).flatten();
            if next.is_none() && sessions.is_some() {
                envelope.reset();
            }
            sessions = next;
            *lock(&shared.inputs) = input_names();
            refreshed = Instant::now();
        }
        // The chosen input stays open only while it's chosen.
        match &source {
            Source::Input(name) if input.as_ref().is_none_or(|(n, _)| n != name) => {
                envelope.reset();
                input = InputPeak::open(name).map(|p| (name.clone(), p));
            }
            Source::Input(_) => {}
            _ => input = None,
        }
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32();
        last = now;
        let mut apps = sessions.as_ref().and_then(|m| peaks(m, own, &mut names));
        if let Some(list) = &mut apps {
            let mut next = HashMap::new();
            for (a, p) in list.iter_mut() {
                let key = a.exe.to_lowercase();
                let s = fall(shown.get(&key).copied().unwrap_or(0.0), *p, dt);
                a.shown = meter(s);
                next.insert(key, s);
            }
            shown = next;
        }
        *lock(&shared.apps) = apps
            .as_ref()
            .map(|list| list.iter().map(|(a, _)| a.clone()).collect())
            .unwrap_or_default();
        let all = apps
            .as_ref()
            .map_or(0.0, |list| list.iter().fold(0.0f32, |m, (_, p)| m.max(*p)));
        shown_all = fall(shown_all, all, dt);
        shared
            .all
            .store(meter(shown_all).to_bits(), Ordering::Relaxed);
        let peak = match &source {
            Source::All => apps.map(|list| list.iter().fold(0.0f32, |m, (_, p)| m.max(*p))),
            Source::App(_) => apps.map(|list| {
                list.iter()
                    .filter(|(a, _)| source.is_app(&a.exe))
                    .fold(0.0f32, |m, (_, p)| m.max(*p))
            }),
            Source::Input(_) => input.as_mut().and_then(|(_, p)| p.take()),
        };
        shown_heard = fall(shown_heard, peak.unwrap_or(0.0), dt);
        shared
            .heard
            .store(meter(shown_heard).to_bits(), Ordering::Relaxed);
        match peak {
            Some(p) => {
                if envelope.step(p, dt) {
                    shared.started.store(true, Ordering::Relaxed);
                }
                let s = if envelope.playing() {
                    State::Playing
                } else {
                    State::Silent
                };
                shared.state.store(s as u8, Ordering::Relaxed);
            }
            None => {
                envelope.reset();
                if matches!(source, Source::Input(_)) {
                    input = None;
                } else {
                    sessions = None;
                }
                shared
                    .state
                    .store(State::Unavailable as u8, Ordering::Relaxed);
            }
        }
        shared
            .level
            .store(envelope.level().to_bits(), Ordering::Relaxed);
        std::thread::sleep(TICK);
    }
    shared.level.store(0f32.to_bits(), Ordering::Relaxed);
    shared.heard.store(0f32.to_bits(), Ordering::Relaxed);
    drop(input);
    drop(sessions);
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
        device
            .Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)
            .ok()
    }
}

/// Each other app's active session with its peak, leaving out TinyPaw
/// itself, or `None` if the sessions can't be read (the device may have
/// gone). Sessions of one app are merged, keeping the loudest.
fn peaks(
    manager: &::windows::Win32::Media::Audio::IAudioSessionManager2,
    own: u32,
    names: &mut Names,
) -> Option<Vec<(App, f32)>> {
    use ::windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
    use ::windows::Win32::Media::Audio::{AudioSessionStateActive, IAudioSessionControl2};
    use ::windows::core::Interface;
    let mut out: Vec<(App, f32)> = Vec::new();
    // SAFETY: plain COM calls on this thread's initialised apartment.
    unsafe {
        let list = manager.GetSessionEnumerator().ok()?;
        for i in 0..list.GetCount().ok()? {
            let Ok(session) = list.GetSession(i) else {
                continue;
            };
            if session.GetState().ok() != Some(AudioSessionStateActive) {
                continue;
            }
            let Ok(control) = session.cast::<IAudioSessionControl2>() else {
                continue;
            };
            let Ok(pid) = control.GetProcessId() else {
                continue;
            };
            // 0 is the system sounds session.
            if pid == own || pid == 0 {
                continue;
            }
            let peak = session
                .cast::<IAudioMeterInformation>()
                .and_then(|m| m.GetPeakValue())
                .unwrap_or(0.0);
            let Some(exe) = names.get(pid) else { continue };
            // Another TinyPaw (a second copy, a build) isn't music either.
            if exe.eq_ignore_ascii_case("tinypaw.exe") {
                continue;
            }
            match out
                .iter_mut()
                .find(|(a, _)| a.exe.eq_ignore_ascii_case(&exe))
            {
                Some((a, p)) => {
                    if !a.pids.contains(&pid) {
                        a.pids.push(pid);
                    }
                    // The loudest session's process is the one to capture.
                    if peak > *p {
                        a.pid = pid;
                        *p = peak;
                    }
                }
                None => out.push((
                    App {
                        exe,
                        shown: 0.0,
                        pid,
                        pids: vec![pid],
                    },
                    peak,
                )),
            }
        }
    }
    out.sort_by_key(|(a, _)| a.exe.to_lowercase());
    Some(out)
}

/// Program file names by process ID, looked up once per process.
#[derive(Default)]
struct Names(HashMap<u32, Option<String>>);

impl Names {
    fn get(&mut self, pid: u32) -> Option<String> {
        if self.0.len() > 256 {
            self.0.clear();
        }
        self.0.entry(pid).or_insert_with(|| exe_name(pid)).clone()
    }
}

/// A process's program file name, such as `Spotify.exe`.
fn exe_name(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    // SAFETY: the handle is checked, used for one query and closed; the
    // buffer length is passed in and updated by the call.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(process);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/'])
            .next()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    }
}

/// Names of the input devices, for the picker.
fn input_names() -> Vec<String> {
    use cpal::traits::HostTrait;
    let Ok(devices) = cpal::default_host().input_devices() else {
        return Vec::new();
    };
    let mut names: Vec<String> = devices.map(|d| d.to_string()).collect();
    names.sort();
    names.dedup();
    names
}

/// An open input device, keeping only the loudest sample since last read.
struct InputPeak {
    _stream: cpal::Stream,
    peak: Arc<AtomicU32>,
    packets: Arc<AtomicU32>,
    last_packets: u32,
    last_packet: Instant,
}

impl InputPeak {
    fn open(name: &str) -> Option<Self> {
        use cpal::traits::StreamTrait;
        let peak = Arc::new(AtomicU32::new(0));
        let p = peak.clone();
        let packets = Arc::new(AtomicU32::new(0));
        let callback_packets = packets.clone();
        let stream = super::beat::open_input(name, move |chunk: &[f32]| {
            callback_packets.fetch_add(1, Ordering::Relaxed);
            let loudest = chunk.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            // Non-negative floats order the same as their bits.
            p.fetch_max(loudest.to_bits(), Ordering::Relaxed);
        })?;
        stream.play().ok()?;
        Some(Self {
            _stream: stream,
            peak,
            packets,
            last_packets: 0,
            last_packet: Instant::now(),
        })
    }

    fn take(&mut self) -> Option<f32> {
        let packets = self.packets.load(Ordering::Relaxed);
        if packets != self.last_packets {
            self.last_packets = packets;
            self.last_packet = Instant::now();
        }
        if self.last_packet.elapsed() >= Duration::from_secs(2) {
            return None;
        }
        Some(f32::from_bits(self.peak.swap(0, Ordering::Relaxed)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads a real input selected through TINYPAW_AUDIO_INPUT"]
    fn the_meter_reads_a_selected_input() {
        let name = std::env::var("TINYPAW_AUDIO_INPUT").expect("set TINYPAW_AUDIO_INPUT");
        let meter = Meter::start(Source::Input(name));
        for _ in 0..12 {
            std::thread::sleep(Duration::from_millis(250));
            println!(
                "state {:?}, heard {:.4}, level {:.4}, all {:.4}",
                meter.state(),
                meter.heard(),
                meter.level(),
                meter.all_apps()
            );
        }
        println!("inputs {:?}", meter.inputs());
    }

    /// Reads this PC's real playback meters for a moment; it needs an audio
    /// device, so it isn't part of the regular run.
    #[test]
    #[ignore = "reads the real default audio output"]
    fn the_meter_reads_the_default_output() {
        let m = Meter::start(Source::All);
        std::thread::sleep(Duration::from_millis(2500));
        let state = m.state();
        println!(
            "meter state {state:?}, level {:.3}, apps {:?}, inputs {:?}",
            m.level(),
            m.apps(),
            m.inputs()
        );
        assert_ne!(state, State::Starting, "the meter reported within 2.5 s");
        drop(m);
    }
}
