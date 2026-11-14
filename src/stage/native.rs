//! Windows stage window: a layered, always-on-top, never-activating popup
//! updated with `UpdateLayeredWindow` from the pixel buffer.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use iced::Point;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::mascot::motion::Pose;
use crate::mascot::render::{Look, Placement, render_character};
use crate::mascot::rig::Rig;
use crate::mascot::surface::Raster;

/// Messages to the stage.
pub enum Intent {
    /// Replaces the character and sizes the window to `height` physical pixels
    /// of visible artwork.
    SetCharacter(Box<Rig>, f32),
    /// Moves the window's top-left corner, in physical pixels.
    MoveTo(i32, i32),
    /// Plays a gentle idle sway at the display's frame rate.
    Animate(bool),
    Quit,
}

/// Messages from the stage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Ready,
    Clicked,
    /// Carried and let go, with the window's top-left corner.
    Dropped(i32, i32),
}

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

/// How far the cursor must move before a press becomes a carry, in pixels.
const DRAG_SLOP: i32 = 4;
/// A pixel this opaque, or more, catches the mouse.
const SOLID: u8 = 32;
const FRAME: Duration = Duration::from_millis(16);

struct Window {
    hwnd: HWND,
    size: (u32, u32),
    pos: (i32, i32),
    rig: Option<Rig>,
    height: f32,
    pose: Pose,
    raster: Option<Raster>,
    dib: Option<Dib>,
    animate: bool,
    started: Instant,
    dirty: bool,
    /// Where the cursor grabbed the window, set while the left button is down.
    grab: Option<Grab>,
    events: Sender<Event>,
}

struct Grab {
    cursor: (i32, i32),
    offset: (i32, i32),
    carried: bool,
}

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

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
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
            size: (0, 0),
            pos: (0, 0),
            rig: None,
            height: 0.0,
            pose: Pose::default(),
            raster: None,
            dib: None,
            animate: false,
            started: Instant::now(),
            dirty: false,
            grab: None,
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
        let animating = with_window(|w| {
            w.frame();
            w.animate
        })
        .unwrap_or(false);
        unsafe {
            MsgWaitForMultipleObjects(
                0,
                null(),
                0,
                if animating {
                    FRAME.as_millis() as u32
                } else {
                    u32::MAX
                },
                QS_ALLINPUT,
            );
        }
    }
}

fn with_window<R>(f: impl FnOnce(&mut Window) -> R) -> Option<R> {
    WINDOW.with(|w| w.try_borrow_mut().ok()?.as_mut().map(f))
}

impl Window {
    fn apply(&mut self, intent: Intent) {
        match intent {
            Intent::SetCharacter(rig, height) => {
                self.pose = Pose {
                    angles: vec![0.0; rig.motion_parts().len()],
                    ..Pose::default()
                };
                self.height = height;
                self.rig = Some(*rig);
                self.resize();
            }
            Intent::MoveTo(x, y) => {
                self.pos = (x, y);
                self.dirty = true;
            }
            Intent::Animate(on) => {
                self.animate = on;
                self.dirty = true;
            }
            Intent::Quit => {}
        }
    }

    fn scale(&self) -> f32 {
        self.rig
            .as_ref()
            .map_or(1.0, |r| self.height / r.visible_height().max(1.0))
    }

    fn resize(&mut self) {
        let Some(rig) = &self.rig else { return };
        let scale = self.scale();
        let size = (
            (rig.size().width * scale).ceil().max(1.0) as u32,
            (rig.size().height * scale).ceil().max(1.0) as u32,
        );
        self.size = size;
        self.raster = Raster::new(size.0, size.1);
        self.dib = Dib::new(size.0, size.1);
        self.dirty = true;
    }

    /// Draws and presents when something changed or the sway is running.
    fn frame(&mut self) {
        if !self.dirty && !self.animate {
            return;
        }
        let (Some(rig), Some(raster), Some(dib)) = (&self.rig, &mut self.raster, &self.dib) else {
            return;
        };
        if self.animate {
            let t = self.started.elapsed().as_secs_f32();
            for (i, angle) in self.pose.angles.iter_mut().enumerate() {
                *angle = 0.05 * (t * 2.0 + i as f32).sin();
            }
        }
        render_character(
            raster,
            rig,
            &self.pose,
            Look::Posed,
            Placement {
                origin: Point::ORIGIN,
                scale: self.height / rig.visible_height().max(1.0),
            },
        );
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
            let at = POINT {
                x: self.pos.0,
                y: self.pos.1,
            };
            let size = windows_sys::Win32::Foundation::SIZE {
                cx: dib.size.0 as i32,
                cy: dib.size.1 as i32,
            };
            let from = POINT { x: 0, y: 0 };
            UpdateLayeredWindow(
                self.hwnd,
                null_mut(),
                &at,
                &size,
                dib.dc,
                &from,
                0,
                &blend,
                ULW_ALPHA,
            );
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
        self.dirty = false;
    }

    /// Alpha of the last drawn frame at a window-local pixel.
    fn alpha_at(&self, x: i32, y: i32) -> u8 {
        let Some(raster) = &self.raster else { return 0 };
        if x < 0 || y < 0 || x >= self.size.0 as i32 || y >= self.size.1 as i32 {
            return 0;
        }
        raster.data()[((y as u32 * self.size.0 + x as u32) * 4 + 3) as usize]
    }
}

fn cursor() -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    (p.x, p.y)
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            let (x, y) = (
                (lparam & 0xFFFF) as i16 as i32,
                ((lparam >> 16) & 0xFFFF) as i16 as i32,
            );
            let solid = with_window(|w| w.alpha_at(x - w.pos.0, y - w.pos.1) >= SOLID);
            return match solid {
                Some(false) => HTTRANSPARENT as LRESULT,
                _ => HTCLIENT as LRESULT,
            };
        }
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        WM_LBUTTONDOWN => {
            with_window(|w| {
                let at = cursor();
                w.grab = Some(Grab {
                    cursor: at,
                    offset: (at.0 - w.pos.0, at.1 - w.pos.1),
                    carried: false,
                });
            });
            unsafe { SetCapture(hwnd) };
            return 0;
        }
        WM_MOUSEMOVE => {
            with_window(|w| {
                let at = cursor();
                let Some(grab) = &mut w.grab else { return };
                let moved = (at.0 - grab.cursor.0)
                    .abs()
                    .max((at.1 - grab.cursor.1).abs());
                if moved >= DRAG_SLOP {
                    grab.carried = true;
                }
                if grab.carried {
                    w.pos = (at.0 - grab.offset.0, at.1 - grab.offset.1);
                    w.dirty = true;
                    w.frame();
                }
            });
            return 0;
        }
        WM_LBUTTONUP => {
            unsafe { ReleaseCapture() };
            with_window(|w| {
                if let Some(grab) = w.grab.take() {
                    let _ = w.events.send(if grab.carried {
                        Event::Dropped(w.pos.0, w.pos.1)
                    } else {
                        Event::Clicked
                    });
                }
            });
            return 0;
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// `tinypaw --stage-probe [animate]`: shows bundled Yohta on the stage so the
/// window, hit testing, carrying and footprint can be tried by hand. It runs
/// until the process is stopped.
pub fn probe() {
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let animate = std::env::args().any(|a| a == "animate");
    let stage = Stage::spawn();
    stage.send(Intent::SetCharacter(Box::new(Rig::load()), 300.0));
    stage.send(Intent::MoveTo(300, 200));
    stage.send(Intent::Animate(animate));
    println!(
        "stage probe running (pid {}); Ctrl+C to stop",
        std::process::id()
    );
    while let Ok(event) = stage.events.recv() {
        println!("{event:?}");
    }
}
