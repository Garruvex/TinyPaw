//! Windows stage window: a layered, always-on-top, never-activating popup
//! updated with `UpdateLayeredWindow` from the pixel buffer. The stage thread
//! owns the character's `Desktop` state, steps it on its own clock and draws
//! it with the shared renderer.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::{Character, Event, Intent};
use crate::mascot::Msg;
use crate::mascot::desktop::{Desktop, Effects, Options};
use crate::mascot::motion::Pose;
use crate::mascot::surface::{Raster, Surface};

/// A handle to the stage thread.
pub struct Stage {
    intents: Sender<Intent>,
    pub events: Receiver<Event>,
    window: Arc<AtomicIsize>,
    thread: Option<JoinHandle<()>>,
}

impl Stage {
    pub fn spawn() -> Self {
        let (intents, intent_rx) = channel();
        let (event_tx, events) = channel();
        let window = Arc::new(AtomicIsize::new(0));
        let shared = window.clone();
        let thread = std::thread::Builder::new()
            .name("tinypaw-stage".into())
            .spawn(move || run(intent_rx, event_tx, shared))
            .expect("start the stage thread");
        Self {
            intents,
            events,
            window,
            thread: Some(thread),
        }
    }

    pub fn send(&self, intent: Intent) {
        if self.intents.send(intent).is_ok() {
            let hwnd = self.window.load(Ordering::Acquire);
            if hwnd != 0 {
                // Wake the stage's message wait.
                unsafe { PostMessageW(hwnd as HWND, WM_APP, 0, 0) };
            }
        }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        self.send(Intent::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// How long the thread may sleep when nothing needs it.
const IDLE: Duration = Duration::from_secs(3600);

struct Dib {
    dc: *mut c_void,
    bitmap: *mut c_void,
    old: *mut c_void,
    bits: *mut u8,
    size: (u32, u32),
}

impl Dib {
    fn new(width: u32, height: u32) -> Option<Self> {
        unsafe {
            let screen = GetDC(null_mut());
            let dc = CreateCompatibleDC(screen);
            let mut info: BITMAPINFO = zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..zeroed()
            };
            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            ReleaseDC(null_mut(), screen);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, bitmap);
            Some(Self {
                dc,
                bitmap,
                old,
                bits: bits.cast(),
                size: (width, height),
            })
        }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

struct Window {
    hwnd: HWND,
    desktop: Option<Desktop>,
    options: Option<Options>,
    raster: Option<Raster>,
    dib: Option<Dib>,
    /// The pose last drawn, to skip frames where nothing moved.
    drawn: Option<Pose>,
    next_poll: Instant,
    /// Something changed outside a poll; draw at the next one.
    dirty: bool,
    events: Sender<Event>,
}

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn with_window<R>(f: impl FnOnce(&mut Window) -> R) -> Option<R> {
    WINDOW.with(|w| w.try_borrow_mut().ok()?.as_mut().map(f))
}

fn run(intents: Receiver<Intent>, events: Sender<Event>, shared: Arc<AtomicIsize>) {
    let class = wide("TinyPawStage");
    let hwnd = unsafe {
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
            hIconSm: null_mut(),
        };
        RegisterClassExW(&wc);
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            class.as_ptr(),
            WS_POPUP,
            -32000,
            -32000,
            1,
            1,
            null_mut(),
            null_mut(),
            instance,
            null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    WINDOW.with(|w| {
        *w.borrow_mut() = Some(Window {
            hwnd,
            desktop: None,
            options: None,
            raster: None,
            dib: None,
            drawn: None,
            next_poll: Instant::now(),
            dirty: false,
            events,
        });
    });
    shared.store(hwnd as isize, Ordering::Release);
    with_window(|w| {
        let _ = w.events.send(Event::Ready);
    });

    loop {
        let mut msg: MSG = unsafe { zeroed() };
        while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if msg.message == WM_QUIT {
                return;
            }
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        while let Ok(intent) = intents.try_recv() {
            if matches!(intent, Intent::Quit) {
                unsafe { DestroyWindow(hwnd) };
                return;
            }
            with_window(|w| w.apply(intent));
        }
        let wait = with_window(|w| w.tick()).unwrap_or(IDLE);
        unsafe {
            MsgWaitForMultipleObjects(
                0,
                null(),
                0,
                wait.as_millis().min(u128::from(u32::MAX - 1)) as u32,
                QS_ALLINPUT,
            );
        }
    }
}

impl Window {
    fn apply(&mut self, intent: Intent) {
        self.dirty = true;
        self.next_poll = Instant::now();
        match intent {
            Intent::Open {
                character,
                size,
                anchor,
                rows,
                hidden,
                options,
            } => {
                self.options = Some(options);
                let Character {
                    id,
                    rig,
                    lines,
                    click_compress,
                    pictures,
                    fall,
                } = *character;
                let mut desktop = Desktop::new(
                    iced::window::Id::unique(),
                    id,
                    rig,
                    size,
                    anchor,
                    rows,
                    hidden,
                );
                desktop.set_lines(lines, &options);
                desktop.set_click_compress(click_compress);
                desktop.set_pictures(pictures);
                desktop.set_fall(fall);
                let fx = desktop.attach(self.hwnd as isize, &options);
                self.desktop = Some(desktop);
                self.effects(fx);
                if !hidden {
                    unsafe { ShowWindow(self.hwnd, SW_SHOWNOACTIVATE) };
                }
            }
            Intent::SetCharacter(character) => {
                let Some(options) = self.options else { return };
                if let Some(d) = &mut self.desktop {
                    let Character {
                        id,
                        rig,
                        lines,
                        click_compress,
                        pictures,
                        fall,
                    } = *character;
                    d.set_rig(id, rig);
                    d.set_lines(lines, &options);
                    d.set_click_compress(click_compress);
                    d.set_pictures(pictures);
                    d.set_fall(fall);
                }
            }
            Intent::Options(options) => self.options = Some(options),
            Intent::Audio { audio, voice_busy } => {
                if let Some(d) = &mut self.desktop {
                    d.set_audio(audio);
                    d.set_voice_busy(voice_busy);
                }
            }
            Intent::Resize { size, rows } => {
                if let Some(d) = &mut self.desktop {
                    d.resize(size, rows);
                }
            }
            Intent::ResetPosition => {
                if let Some(d) = &mut self.desktop {
                    d.reset_position();
                }
            }
            Intent::DisplaysChanged => {
                if let Some(d) = &mut self.desktop {
                    d.displays_changed();
                }
            }
            Intent::Hidden(hidden) => {
                let hide = self.desktop.as_mut().and_then(|d| d.hide(hidden));
                self.effects(Effects {
                    hide,
                    ..Effects::default()
                });
            }
            Intent::Quit => {}
        }
    }

    /// Applies what a step asked of the window and tells the app what it saw.
    fn effects(&mut self, fx: Effects) {
        match fx.hide {
            Some(true) => unsafe {
                ShowWindow(self.hwnd, SW_HIDE);
            },
            Some(false) => unsafe {
                ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            },
            None => {}
        }
        if let Some((x, y)) = fx.save {
            let _ = self.events.send(Event::Placed(x, y));
        }
        if let Some(d) = &mut self.desktop {
            for gesture in d.take_events() {
                let _ = self.events.send(Event::Gesture(gesture));
            }
            if let Some(sound) = d.take_sound() {
                let _ = self.events.send(Event::Sound(sound));
            }
        }
    }

    /// Steps the character when due, draws if anything moved, and says how
    /// long the thread may sleep.
    fn tick(&mut self) -> Duration {
        let now = Instant::now();
        if now < self.next_poll {
            return self.next_poll - now;
        }
        let Some(options) = self.options else {
            return IDLE;
        };
        let Some(d) = &mut self.desktop else {
            return IDLE;
        };
        let fx = d.step(Msg::Poll, &options);
        let interval = d.poll_interval(&options);
        self.effects(fx);
        self.draw(interval.is_none_or(|i| i < Duration::from_millis(100)));
        let wait = interval.unwrap_or(IDLE);
        self.next_poll = Instant::now() + wait;
        wait
    }

    /// Draws and presents. `moving` is set while the character is animating;
    /// when it is still, a frame is drawn only if the pose or window changed.
    fn draw(&mut self, moving: bool) {
        let Some(d) = &self.desktop else { return };
        let Some((rect, k)) = d.window_scale() else {
            return;
        };
        let size = (
            (rect.2 - rect.0).max(1) as u32,
            (rect.3 - rect.1).max(1) as u32,
        );
        let resized = self.raster.as_ref().map(|r| (r.width(), r.height())) != Some(size);
        if resized {
            self.raster = Raster::new(size.0, size.1);
            self.dib = Dib::new(size.0, size.1);
        }
        let pose = d.pose();
        if !moving && !resized && !self.dirty && self.drawn.as_ref() == Some(pose) {
            return;
        }
        let (Some(raster), Some(dib)) = (&mut self.raster, &self.dib) else {
            return;
        };
        raster.clear();
        raster.with_save(|r| {
            r.scale(k);
            d.paint_character(r);
        });
        raster.with_save(|r| {
            r.scale(k);
            d.paint_overlay(r);
        });
        // Premultiplied RGBA into the DIB's premultiplied BGRA.
        let count = (dib.size.0 * dib.size.1) as usize;
        let out = unsafe { std::slice::from_raw_parts_mut(dib.bits, count * 4) };
        for (dst, src) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(raster.data().as_chunks::<4>().0)
        {
            *dst = [src[2], src[1], src[0], src[3]];
        }
        unsafe {
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let extent = SIZE {
                cx: dib.size.0 as i32,
                cy: dib.size.1 as i32,
            };
            let from = POINT { x: 0, y: 0 };
            // No destination: the window keeps the position the character
            // logic gave it with SetWindowPos.
            UpdateLayeredWindow(
                self.hwnd,
                null_mut(),
                null(),
                &extent,
                dib.dc,
                &from,
                0,
                &blend,
                ULW_ALPHA,
            );
        }
        self.drawn = Some(pose.clone());
        self.dirty = false;
    }
}

fn cursor_point(lparam: LPARAM) -> (i32, i32) {
    (
        (lparam & 0xFFFF) as i16 as i32,
        ((lparam >> 16) & 0xFFFF) as i16 as i32,
    )
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            let at = cursor_point(lparam);
            let solid = with_window(|w| w.desktop.as_ref().is_some_and(|d| d.over_screen(at)));
            return match solid {
                Some(false) => HTTRANSPARENT as LRESULT,
                _ => HTCLIENT as LRESULT,
            };
        }
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        WM_LBUTTONDOWN => {
            with_window(|w| {
                if let (Some(d), Some(options)) = (&mut w.desktop, w.options) {
                    d.step(Msg::Pressed, &options);
                    w.dirty = true;
                    w.next_poll = Instant::now();
                }
            });
            unsafe { SetCapture(hwnd) };
            return 0;
        }
        WM_LBUTTONUP => {
            unsafe { ReleaseCapture() };
            with_window(|w| {
                if let (Some(d), Some(options)) = (&mut w.desktop, w.options) {
                    let fx = d.step(Msg::Released, &options);
                    w.effects(fx);
                    w.dirty = true;
                    w.next_poll = Instant::now();
                }
            });
            return 0;
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_window(|w| {
                if let Some(d) = &mut w.desktop {
                    d.displays_changed();
                    w.dirty = true;
                    w.next_poll = Instant::now();
                }
            });
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// `tinypaw --stage-probe`: shows bundled Yohta on the stage, behaving as the
/// desktop character does today, so it can be tried by hand. It prints the
/// events the app would receive and runs until the process is stopped.
pub fn probe() {
    use crate::features::mascot as feature;
    use crate::mascot::desktop::Pictures;
    use crate::mascot::lines::Lines;
    use crate::mascot::rig::Rig;
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let stage = Stage::spawn();
    stage.send(Intent::Open {
        character: Box::new(Character {
            id: "yohta",
            rig: Rig::load(),
            lines: Lines::default(),
            click_compress: true,
            pictures: Pictures::default(),
            fall: false,
        }),
        size: 146.0,
        anchor: (feature::UNPLACED, feature::UNPLACED),
        rows: 5,
        hidden: false,
        options: Options {
            bop: None,
            dance: Default::default(),
            eyes: true,
            idle: true,
            reduced: false,
            lines: false,
            quiet: true,
            idle_every: (60.0, 120.0),
            music: 0.0,
            energy: None,
            beat: None,
        },
    });
    println!(
        "stage probe running (pid {}); Ctrl+C to stop",
        std::process::id()
    );
    while let Ok(event) = stage.events.recv() {
        println!("{event:?}");
    }
}
