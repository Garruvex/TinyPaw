//! Live beat following (trial). An audio stream feeds
//! `mascot::beat::Tracker` on its own thread: a loopback of the default
//! output, a loopback of just one app (Windows' process loopback, which
//! leaves every other app out), or an input device. The audio callback only
//! downmixes and hands chunks over, never waiting. Runs only while Follow
//! beat is on and music response is running, and stops when dropped. Output
//! loopback hears TinyPaw too, so the app holds off onset detection while it
//! plays its own sounds.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::mascot::beat::{Sensitivity, Tracker};

/// With no audio for this long the stream is rebuilt, which follows a change
/// of default output.
const STALL: Duration = Duration::from_secs(3);

/// What the beat follower listens to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Everything on the default output.
    Output,
    /// One process and its children, by process ID.
    Process(u32),
    /// An input device, by name.
    Input(String),
}

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
    target: Target,
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
    pub fn start(sensitivity: Sensitivity, target: Target) -> Self {
        let shared = Arc::new(Shared::default());
        shared
            .sensitivity
            .store(encode(sensitivity), Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (shared, stop, target) = (shared.clone(), stop.clone(), target.clone());
            std::thread::Builder::new()
                .name("tinypaw-beat".into())
                .spawn(move || run(&shared, &stop, &target))
                .ok()
        };
        Self {
            target,
            shared,
            stop,
            thread,
        }
    }

    pub fn target(&self) -> &Target {
        &self.target
    }

    pub fn set_sensitivity(&self, s: Sensitivity) {
        self.shared.sensitivity.store(encode(s), Ordering::Relaxed);
    }

    /// Ignore onsets while TinyPaw plays a sound of this length.
    pub fn hold_off(&self, seconds: f32) {
        let until = Instant::now() + Duration::from_secs_f32(seconds.clamp(0.0, 10.0) + 0.05);
        let mut h = self
            .shared
            .hold_until
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let last = (*self
            .shared
            .last_beat
            .lock()
            .unwrap_or_else(|e| e.into_inner()))?;
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

/// A running capture; dropping it stops the audio.
#[allow(dead_code)]
enum Capture {
    Cpal(cpal::Stream),
    Process(ProcessLoopback),
}

fn run(shared: &Shared, stop: &AtomicBool, target: &Target) {
    while !stop.load(Ordering::Relaxed) {
        let (tx, rx) = sync_channel(64);
        let opened = match target {
            Target::Output => open_loopback(tx).map(|(s, rate)| (Capture::Cpal(s), rate)),
            Target::Input(name) => {
                open_input_chunks(name, tx).map(|(s, rate)| (Capture::Cpal(s), rate))
            }
            Target::Process(pid) => {
                ProcessLoopback::start(*pid, tx).map(|p| (Capture::Process(p), PROCESS_RATE))
            }
        };
        match opened {
            Some((capture, rate)) => {
                shared.available.store(true, Ordering::Relaxed);
                follow(shared, stop, &rx, rate);
                drop(capture);
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
            let left = until
                .saturating_duration_since(Instant::now())
                .as_secs_f64();
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

/// Builds a stream on `device` that downmixes to mono and passes each chunk
/// to `sink`, which must never block. Not started yet.
fn build_mono(
    device: &cpal::Device,
    supported: cpal::SupportedStreamConfig,
    mut sink: impl FnMut(&[f32]) + Send + 'static,
) -> Option<cpal::Stream> {
    use cpal::traits::DeviceTrait;
    let channels = usize::from(supported.channels()).max(1);
    let config = supported.config();
    let err = |e| eprintln!("tinypaw: audio capture: {e}");
    let mut mono = Vec::new();
    match supported.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    mono.clear();
                    mono.extend(
                        data.chunks(channels)
                            .map(|f| f.iter().sum::<f32>() / channels as f32),
                    );
                    sink(&mono);
                },
                err,
                None,
            )
            .ok(),
        cpal::SampleFormat::I16 => device
            .build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    mono.clear();
                    mono.extend(data.chunks(channels).map(|f| {
                        f.iter().map(|s| f32::from(*s) / 32768.0).sum::<f32>() / channels as f32
                    }));
                    sink(&mono);
                },
                err,
                None,
            )
            .ok(),
        _ => None,
    }
}

/// Hands chunks to the analysis thread; if it falls behind, the chunk is
/// dropped rather than blocking the audio thread.
fn to_channel(tx: SyncSender<Vec<f32>>) -> impl FnMut(&[f32]) + Send + 'static {
    move |chunk| {
        let _ = tx.try_send(chunk.to_vec());
    }
}

/// A loopback capture of the default output, started.
fn open_loopback(tx: SyncSender<Vec<f32>>) -> Option<(cpal::Stream, u32)> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let rate = supported.sample_rate();
    let stream = build_mono(&device, supported, to_channel(tx))?;
    stream.play().ok()?;
    Some((stream, rate))
}

/// The input device called `name`.
fn input_device(name: &str) -> Option<cpal::Device> {
    use cpal::traits::HostTrait;
    cpal::default_host()
        .input_devices()
        .ok()?
        .find(|d| d.to_string().starts_with(name))
}

/// A mono stream from the input device called `name`, not started yet.
/// Also used by the playback meter for its level.
pub(super) fn open_input(
    name: &str,
    sink: impl FnMut(&[f32]) + Send + 'static,
) -> Option<cpal::Stream> {
    use cpal::traits::DeviceTrait;
    let device = input_device(name)?;
    let supported = device.default_input_config().ok()?;
    build_mono(&device, supported, sink)
}

fn open_input_chunks(name: &str, tx: SyncSender<Vec<f32>>) -> Option<(cpal::Stream, u32)> {
    use cpal::traits::{DeviceTrait, StreamTrait};
    let rate = input_device(name)?
        .default_input_config()
        .ok()?
        .sample_rate();
    let stream = open_input(name, to_channel(tx))?;
    stream.play().ok()?;
    Some((stream, rate))
}

// ---- Process loopback ---------------------------------------------------------

/// Process loopback delivers the format it's asked for; this one.
const PROCESS_RATE: u32 = 48_000;
const PROCESS_CHANNELS: u16 = 2;

/// Captures one process tree's audio on its own thread.
struct ProcessLoopback {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ProcessLoopback {
    /// Starts capturing `pid`'s audio, or `None` if Windows can't (older
    /// than Windows 10 2004, or the process is gone).
    fn start(pid: u32, tx: SyncSender<Vec<f32>>) -> Option<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = sync_channel(1);
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("tinypaw-app-capture".into())
                .spawn(move || process_capture(pid, &tx, &stop, &ready_tx))
                .ok()?
        };
        // Wait for the capture to open (or fail) before reporting.
        if ready_rx.recv_timeout(Duration::from_secs(3)) != Ok(true) {
            stop.store(true, Ordering::Relaxed);
            let _ = thread.join();
            return None;
        }
        Some(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for ProcessLoopback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Signals when asynchronous audio activation finishes.
mod handler {
    use std::sync::mpsc::SyncSender;
    use windows::Win32::Media::Audio::{
        IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
        IActivateAudioInterfaceCompletionHandler_Impl,
    };
    use windows::Win32::System::Com::{IAgileObject, IAgileObject_Impl};
    use windows::core::{Ref, implement};

    #[implement(IActivateAudioInterfaceCompletionHandler, IAgileObject)]
    pub(super) struct Activated(pub(super) SyncSender<()>);

    impl IActivateAudioInterfaceCompletionHandler_Impl for Activated_Impl {
        fn ActivateCompleted(
            &self,
            _: Ref<IActivateAudioInterfaceAsyncOperation>,
        ) -> windows::core::Result<()> {
            let _ = self.0.try_send(());
            Ok(())
        }
    }

    impl IAgileObject_Impl for Activated_Impl {}
}

fn process_capture(
    pid: u32,
    tx: &SyncSender<Vec<f32>>,
    stop: &AtomicBool,
    ready: &SyncSender<bool>,
) {
    use ::windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    // SAFETY: this thread owns its COM apartment for its whole life.
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let client = com.then(|| activate(pid)).flatten();
    match client.and_then(|c| start_client(&c).map(|capture| (c, capture))) {
        Some((client, capture)) => {
            let _ = ready.send(true);
            capture_loop(&capture, tx, stop);
            // SAFETY: stopping a started client on its own thread.
            let _ = unsafe { client.Stop() };
        }
        None => {
            let _ = ready.send(false);
        }
    }
    if com {
        // SAFETY: balances the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
}

/// An audio client that hears only `pid` and its child processes.
fn activate(pid: u32) -> Option<::windows::Win32::Media::Audio::IAudioClient> {
    use ::windows::Win32::Media::Audio::{
        AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
        AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
        ActivateAudioInterfaceAsync, IActivateAudioInterfaceCompletionHandler, IAudioClient,
        PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
    };
    use ::windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use ::windows::Win32::System::Variant::VT_BLOB;
    use ::windows::core::{IUnknown, Interface};
    let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };
    // The blob points at `params`, which outlives the call; ManuallyDrop
    // keeps PROPVARIANT's destructor from freeing memory it doesn't own.
    let mut prop = std::mem::ManuallyDrop::new(PROPVARIANT::default());
    // SAFETY: writes the VT_BLOB variant's tag and blob fields of a zeroed
    // PROPVARIANT.
    unsafe {
        let v = &mut prop.Anonymous.Anonymous;
        v.vt = VT_BLOB;
        v.Anonymous.blob.cbSize = std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32;
        v.Anonymous.blob.pBlobData = (&raw mut params).cast();
    }
    let (done_tx, done_rx) = sync_channel(1);
    let handler: IActivateAudioInterfaceCompletionHandler = handler::Activated(done_tx).into();
    // SAFETY: the parameters and handler stay alive until activation
    // completes, which is waited for below.
    let operation = unsafe {
        ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*prop),
            &handler,
        )
    }
    .ok()?;
    done_rx.recv_timeout(Duration::from_secs(3)).ok()?;
    let mut result = ::windows::core::HRESULT(0);
    let mut unknown: Option<IUnknown> = None;
    // SAFETY: activation has completed, so its result can be read.
    unsafe { operation.GetActivateResult(&mut result, &mut unknown) }.ok()?;
    result.ok().ok()?;
    unknown?.cast().ok()
}

/// Initialises loopback capture in our fixed float format and starts it.
fn start_client(
    client: &::windows::Win32::Media::Audio::IAudioClient,
) -> Option<::windows::Win32::Media::Audio::IAudioCaptureClient> {
    use ::windows::Win32::Media::Audio::{
        AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, WAVEFORMATEX,
    };
    const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
    let block = PROCESS_CHANNELS * 4;
    let format = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_IEEE_FLOAT,
        nChannels: PROCESS_CHANNELS,
        nSamplesPerSec: PROCESS_RATE,
        nAvgBytesPerSec: PROCESS_RATE * u32::from(block),
        nBlockAlign: block,
        wBitsPerSample: 32,
        cbSize: 0,
    };
    // SAFETY: plain COM calls; `format` lives across Initialize.
    unsafe {
        // A 200 ms buffer, read every 10 ms.
        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                2_000_000,
                0,
                &format,
                None,
            )
            .ok()?;
        let capture: IAudioCaptureClient = client.GetService().ok()?;
        client.Start().ok()?;
        Some(capture)
    }
}

/// Reads packets until stopped, handing mono chunks over. Silence still
/// arrives as packets, so a quiet app doesn't look like a stall.
fn capture_loop(
    capture: &::windows::Win32::Media::Audio::IAudioCaptureClient,
    tx: &SyncSender<Vec<f32>>,
    stop: &AtomicBool,
) {
    use ::windows::Win32::Media::Audio::AUDCLNT_BUFFERFLAGS_SILENT;
    let channels = usize::from(PROCESS_CHANNELS);
    let mut last_packet = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(10));
        let mut mono = Vec::new();
        // SAFETY: each buffer is read within its GetBuffer/ReleaseBuffer
        // pair, for the frame count the call reported, in the float format
        // the client was initialised with.
        unsafe {
            loop {
                let Ok(size) = capture.GetNextPacketSize() else {
                    return;
                };
                if size == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                if capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                    .is_err()
                {
                    return;
                }
                let n = frames as usize;
                if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || data.is_null() {
                    mono.extend(std::iter::repeat_n(0.0, n));
                } else {
                    let samples = std::slice::from_raw_parts(data.cast::<f32>(), n * channels);
                    mono.extend(
                        samples
                            .chunks(channels)
                            .map(|f| f.iter().sum::<f32>() / channels as f32),
                    );
                }
                let _ = capture.ReleaseBuffer(frames);
            }
        }
        if mono.is_empty() {
            // Process loopback may send nothing while the app is silent:
            // feed silence so the tracker's clock keeps running.
            let quiet = last_packet.elapsed();
            if quiet >= Duration::from_millis(50) {
                let n = (quiet.as_secs_f64() * f64::from(PROCESS_RATE)) as usize;
                let _ = tx.try_send(vec![0.0; n]);
                last_packet = Instant::now();
            }
            continue;
        }
        last_packet = Instant::now();
        let _ = tx.try_send(mono);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opens a real loopback stream on this PC's default output.
    #[test]
    #[ignore = "opens the real default audio output"]
    fn loopback_opens_on_the_default_output() {
        let f = Follower::start(Sensitivity::Normal, Target::Output);
        std::thread::sleep(Duration::from_millis(1500));
        println!("loopback available {}, bpm {:?}", f.available(), f.bpm());
        assert!(f.available());
    }

    /// Opens Windows' process loopback on this test process itself: it
    /// plays nothing, but the capture must open.
    #[test]
    #[ignore = "opens real audio capture"]
    fn process_loopback_opens() {
        let (tx, rx) = sync_channel(64);
        let p = ProcessLoopback::start(std::process::id(), tx);
        assert!(p.is_some(), "process loopback opened");
        let chunk = rx.recv_timeout(Duration::from_secs(2));
        println!("first chunk {:?} samples", chunk.map(|c| c.len()));
    }
}
