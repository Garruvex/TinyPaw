//! Step 0 prototype: a Settings Characters page and a studio preview pane in
//! Slint with the software renderer. The preview picture is drawn with
//! tiny-skia into a pixel buffer (stand-in for the shared character renderer)
//! and handed to Slint every frame; the selection box is drawn by Slint.
//!
//! `cargo run --release` opens both windows. `PROTO_BENCH=1` runs the
//! measurements headlessly and prints a report.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{
    Model,
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel,
};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

slint::include_modules!();

/// Logical size of the preview stage; keep in step with `studio.slint`.
const W: f32 = 360.0;
const H: f32 = 440.0;
const LAYER_W: f32 = 90.0;
const LAYER_H: f32 = 60.0;
/// The draggable layer's exact colour, so the bench can find it in a snapshot.
const LAYER_RGB: [u8; 3] = [0x00, 0xFF, 0x80];

/// What the renderer needs; read from the Slint window every frame.
#[derive(Clone, Copy)]
struct View {
    zoom: f32,
    cx: f32,
    cy: f32,
    scale: f32,
    phase: f32,
}

fn paint(rgb: [u8; 3], aa: bool) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(rgb[0], rgb[1], rgb[2], 0xFF);
    p.anti_alias = aa;
    p
}

/// Draws the stand-in character and the layer into a pixel buffer at physical
/// resolution. Anti-aliasing is off for the layer so its edges are exact.
fn render(v: View) -> SharedPixelBuffer<Rgba8Pixel> {
    let pw = (W * v.scale).round() as u32;
    let ph = (H * v.scale).round() as u32;
    let mut pm = Pixmap::new(pw, ph).expect("pixmap");
    pm.fill(Color::from_rgba8(0x11, 0x12, 0x16, 0xFF));

    // Model space -> physical pixels: zoom about the stage centre.
    let view = Transform::from_translate(-W / 2.0, -H / 2.0)
        .post_scale(v.zoom * v.scale, v.zoom * v.scale)
        .post_translate(W / 2.0 * v.scale, H / 2.0 * v.scale);

    let bob = (v.phase * 2.0).sin() * 3.0;
    let body = paint([0x5E, 0x63, 0x6C], true);
    let head = paint([0x7A, 0x80, 0x8A], true);
    if let Some(p) = PathBuilder::from_circle(180.0, 300.0, 70.0) {
        pm.fill_path(&p, &body, FillRule::Winding, view, None);
    }
    if let Some(p) = PathBuilder::from_circle(180.0, 215.0 + bob, 56.0) {
        pm.fill_path(&p, &head, FillRule::Winding, view, None);
    }
    for (x0, x1, x2) in [(135.0, 150.0, 128.0), (225.0, 210.0, 232.0)] {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, 185.0 + bob);
        pb.line_to(x1, 170.0 + bob);
        pb.line_to(x2, 140.0 + bob);
        pb.close();
        if let Some(p) = pb.finish() {
            pm.fill_path(&p, &head, FillRule::Winding, view, None);
        }
    }
    // A swinging tail: a stroked curve whose tip moves with the phase.
    let mut pb = PathBuilder::new();
    pb.move_to(240.0, 320.0);
    pb.quad_to(285.0, 300.0, 275.0 + (v.phase * 3.0).sin() * 18.0, 250.0);
    if let Some(p) = pb.finish() {
        let stroke = Stroke {
            width: 10.0,
            ..Stroke::default()
        };
        pm.stroke_path(&p, &body, &stroke, view, None);
    }

    // The draggable layer, snapped to whole physical pixels.
    let edge = |m: f32, centre: f32| ((m - centre) * v.zoom + centre) * v.scale;
    let x0 = edge(v.cx - LAYER_W / 2.0, W / 2.0).round();
    let x1 = edge(v.cx + LAYER_W / 2.0, W / 2.0).round();
    let y0 = edge(v.cy - LAYER_H / 2.0, H / 2.0).round();
    let y1 = edge(v.cy + LAYER_H / 2.0, H / 2.0).round();
    if let Some(r) = Rect::from_ltrb(x0, y0, x1, y1) {
        pm.fill_rect(r, &paint(LAYER_RGB, false), Transform::identity(), None);
    }

    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(pw, ph);
    buf.make_mut_bytes().copy_from_slice(pm.data());
    buf
}

fn read_view(studio: &StudioWindow, phase: f32) -> View {
    View {
        zoom: studio.get_zoom(),
        cx: studio.get_layer_cx(),
        cy: studio.get_layer_cy(),
        scale: studio.window().scale_factor(),
        phase,
    }
}

#[cfg(windows)]
fn memory_mib() -> (f64, f64) {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), (&mut c as *mut PROCESS_MEMORY_COUNTERS_EX).cast(), c.cb) };
    (
        c.WorkingSetSize as f64 / 1_048_576.0,
        c.PrivateUsage as f64 / 1_048_576.0,
    )
}

#[cfg(not(windows))]
fn memory_mib() -> (f64, f64) {
    (0.0, 0.0)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn stats(label: &str, mut v: Vec<f64>) -> String {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    let mean = v.iter().sum::<f64>() / n as f64;
    format!(
        "{label}: mean {mean:.2} ms, p95 {:.2} ms, max {:.2} ms ({n} samples)",
        v[(n * 95 / 100).min(n - 1)],
        v[n - 1]
    )
}

/// Bounding box (min x, min y, max x, max y) of pixels matching `hit`.
fn bbox(buf: &SharedPixelBuffer<Rgba8Pixel>, hit: impl Fn(&Rgba8Pixel) -> bool) -> Option<[i32; 4]> {
    let w = buf.width() as usize;
    let mut b = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for (i, px) in buf.as_slice().iter().enumerate() {
        if hit(px) {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    (b[0] != i32::MAX).then_some(b)
}

fn main() {
    let t0 = Instant::now();
    let bench = std::env::var_os("PROTO_BENCH").is_some();

    let settings = SettingsWindow::new().unwrap();
    let studio = StudioWindow::new().unwrap();

    let instances = Rc::new(VecModel::from(vec![
        Instance { name: "Yohta".into(), display: "Display 1".into(), size: 146.0, shown: true },
        Instance { name: "Yohta 2".into(), display: "Display 2".into(), size: 104.0, shown: true },
        Instance { name: "Mochi".into(), display: "Hidden".into(), size: 180.0, shown: false },
    ]));
    settings.set_instances(ModelRc::from(instances.clone()));
    {
        let m = instances.clone();
        settings.on_size_changed(move |i, v| {
            if let Some(mut row) = m.row_data(i as usize) {
                row.size = v;
                m.set_row_data(i as usize, row);
            }
        });
        let m = instances.clone();
        settings.on_shown_changed(move |i, on| {
            if let Some(mut row) = m.row_data(i as usize) {
                row.shown = on;
                m.set_row_data(i as usize, row);
            }
        });
    }
    {
        let studio = studio.as_weak();
        settings.on_customize(move || {
            if let Some(s) = studio.upgrade() {
                let _ = s.show();
            }
        });
    }

    // Redraw the picture the moment the layer or zoom changes, so it is part
    // of the same frame as the box Slint draws. Without this the picture waits
    // for the frame timer below and trails the box by up to a frame.
    if std::env::var_os("PROTO_NO_CHANGE_RENDER").is_none() {
        let studio_w = studio.as_weak();
        studio.on_layer_changed(move || {
            if let Some(s) = studio_w.upgrade() {
                let buf = render(read_view(&s, t0.elapsed().as_secs_f32()));
                s.set_frame(Image::from_rgba8_premultiplied(buf));
            }
        });
    }

    // One frame clock for the preview: animates the character and shows timing.
    // The bench drives frames itself, so it runs without this timer.
    let frame_ms = Rc::new(RefCell::new(Vec::<f64>::new()));
    let timer = Timer::default();
    if !bench || std::env::var_os("PROTO_NO_CHANGE_RENDER").is_some() {
        let studio = studio.as_weak();
        let frame_ms = frame_ms.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let Some(s) = studio.upgrade() else { return };
            if !s.window().is_visible() {
                return;
            }
            let started = Instant::now();
            let buf = render(read_view(&s, t0.elapsed().as_secs_f32()));
            s.set_frame(Image::from_rgba8_premultiplied(buf));
            let mut samples = frame_ms.borrow_mut();
            samples.push(ms(started.elapsed()));
            if samples.len() >= 60 {
                let mean = samples.iter().sum::<f64>() / samples.len() as f64;
                s.set_stats(format!("Hand-off per frame (render + set): {mean:.2} ms").into());
                samples.clear();
            }
        });
    }

    if bench {
        let settings_w = settings.as_weak();
        let studio_w = studio.as_weak();
        Timer::single_shot(Duration::from_millis(0), move || {
            let studio = studio_w.upgrade().unwrap();
            run_bench(t0, &settings_w.upgrade().unwrap(), &studio);
            // The drag test needs real event loop turns, so it chains timers
            // and quits the loop when it is done.
            println!("drag with real event loop turns (picture lag behind the box, logical px):");
            drive_drag(studio.as_weak(), vec![0.5, 1.0, 2.0]);
        });
        settings.show().unwrap();
        slint::run_event_loop_until_quit().unwrap();
        return;
    }

    settings.show().unwrap();
    slint::run_event_loop_until_quit().unwrap();
}

/// Presses on the layer and drags with the cursor, checking after every move
/// that the layer sits exactly where the cursor has carried it. Returns the
/// worst gap in logical pixels between where the layer should be and where it is.
fn drag_follow(studio: &StudioWindow, zoom: f32) -> f32 {
    use slint::platform::{PointerEventButton, WindowEvent};
    use slint::LogicalPosition;

    studio.set_zoom(zoom);
    studio.set_layer_cx(180.0);
    studio.set_layer_cy(290.0);
    let (sx, sy) = (studio.get_stage_left(), studio.get_stage_top());
    let on_screen = |m: f32, centre: f32| (m - centre) * zoom + centre;
    let start = LogicalPosition::new(
        sx + on_screen(180.0, W / 2.0),
        sy + on_screen(290.0, H / 2.0),
    );
    let w = studio.window();
    w.dispatch_event(WindowEvent::PointerMoved { position: start });
    w.dispatch_event(WindowEvent::PointerPressed {
        position: start,
        button: PointerEventButton::Left,
    });
    let mut worst = 0.0_f32;
    for k in 1..=40 {
        let (dx, dy) = (k as f32 * 2.5, k as f32 * 1.5);
        w.dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new(start.x + dx, start.y + dy),
        });
        let gap_x = (studio.get_layer_cx() - (180.0 + dx / zoom)).abs();
        let gap_y = (studio.get_layer_cy() - (290.0 + dy / zoom)).abs();
        worst = worst.max(gap_x * zoom).max(gap_y * zoom);
    }
    w.dispatch_event(WindowEvent::PointerReleased {
        position: start,
        button: PointerEventButton::Left,
    });
    worst
}

/// Drags in steps spread over real event loop turns. After each turn the
/// painted layer is compared with the box Slint draws: a gap means the picture
/// is a frame behind. Runs each zoom in turn, then quits the loop.
fn drive_drag(handle: slint::Weak<StudioWindow>, mut zooms: Vec<f32>) {
    use slint::platform::{PointerEventButton, WindowEvent};
    use slint::LogicalPosition;

    if zooms.is_empty() {
        let _ = slint::quit_event_loop();
        return;
    }
    let Some(studio) = handle.upgrade() else { return };
    let zoom = zooms.remove(0);
    studio.set_zoom(zoom);
    studio.set_layer_cx(180.0);
    studio.set_layer_cy(290.0);
    let on_screen = |m: f32, centre: f32| (m - centre) * zoom + centre;
    let start = LogicalPosition::new(
        studio.get_stage_left() + on_screen(180.0, W / 2.0),
        studio.get_stage_top() + on_screen(290.0, H / 2.0),
    );
    let w = studio.window();
    w.dispatch_event(WindowEvent::PointerMoved { position: start });
    w.dispatch_event(WindowEvent::PointerPressed {
        position: start,
        button: PointerEventButton::Left,
    });
    drag_step(handle, zoom, start, 0, 0, zooms);
}

fn drag_step(
    handle: slint::Weak<StudioWindow>,
    zoom: f32,
    start: slint::LogicalPosition,
    k: i32,
    lag: i32,
    rest: Vec<f32>,
) {
    use slint::platform::{PointerEventButton, WindowEvent};

    Timer::single_shot(Duration::from_millis(4), move || {
        let Some(studio) = handle.upgrade() else { return };
        let mut lag = lag;
        if k > 0 {
            // A loop turn has passed since the last move: this is what the
            // screen would show.
            let snap = studio.window().take_snapshot().expect("snapshot");
            let layer = bbox(&snap, |p| [p.r, p.g, p.b] == LAYER_RGB);
            let boxed = bbox(&snap, |p| p.r > 200 && p.b > 200 && p.g < 60);
            if let (Some(l), Some(b)) = (layer, boxed) {
                lag = lag.max((b[0] - (l[0] - 1)).abs()).max((b[1] - (l[1] - 1)).abs());
            }
        }
        if k == 30 {
            studio.window().dispatch_event(WindowEvent::PointerReleased {
                position: start,
                button: PointerEventButton::Left,
            });
            println!("  zoom {zoom:.1}: {lag} px");
            drive_drag(handle, rest);
            return;
        }
        let (dx, dy) = ((k + 1) as f32 * 3.0, (k + 1) as f32 * 2.0);
        studio.window().dispatch_event(WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(start.x + dx, start.y + dy),
        });
        drag_step(handle, zoom, start, k + 1, lag, rest);
    });
}

fn run_bench(t0: Instant, settings: &SettingsWindow, studio: &StudioWindow) {
    // SLINT_SCALE_FACTOR is not honoured by this backend, so ask for a scale
    // factor through the window event instead.
    // Windows are set to their real preferred sizes so render times are not
    // understated by the headless minimum size.
    let scale = std::env::var("PROTO_SCALE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);
    for (w, (lw, lh)) in [
        (settings.window(), (1000.0_f32, 680.0_f32)),
        (studio.window(), (1100.0, 720.0)),
    ] {
        if scale != 1.0 {
            w.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                scale_factor: scale,
            });
        }
        w.set_size(slint::PhysicalSize::new(
            (lw * scale).round() as u32,
            (lh * scale).round() as u32,
        ));
    }
    println!("Slint {} (software renderer, winit)", "1.18.1");
    println!("scale factor: {}", settings.window().scale_factor());
    println!("process start -> event loop running: {:.1} ms", ms(t0.elapsed()));

    let s = Instant::now();
    settings.window().take_snapshot().expect("settings snapshot");
    println!("Settings first full render: {:.1} ms", ms(s.elapsed()));
    let (ws, pv) = memory_mib();
    println!("memory, Settings open: {ws:.1} MiB working set, {pv:.1} MiB private");

    let s = Instant::now();
    studio.show().unwrap();
    println!("studio show(): {:.1} ms", ms(s.elapsed()));
    let s = Instant::now();
    studio.set_frame(Image::from_rgba8_premultiplied(render(read_view(studio, 0.0))));
    studio.window().take_snapshot().expect("studio snapshot");
    println!("studio first frame (render + hand-off + full window render): {:.1} ms", ms(s.elapsed()));
    let (ws, pv) = memory_mib();
    println!("memory, both open: {ws:.1} MiB working set, {pv:.1} MiB private");

    // A drag at 300 steps: renderer hand-off, and the whole-window software
    // render as the worst case for presenting.
    let (mut handoff, mut window) = (Vec::new(), Vec::new());
    for i in 0..300 {
        let t = i as f32 / 300.0 * std::f32::consts::TAU;
        studio.set_layer_cx(180.0 + 70.0 * t.sin());
        studio.set_layer_cy(290.0 + 60.0 * t.cos());
        let s = Instant::now();
        studio.set_frame(Image::from_rgba8_premultiplied(render(read_view(studio, t))));
        handoff.push(ms(s.elapsed()));
        let s = Instant::now();
        studio.window().take_snapshot().expect("snapshot");
        window.push(ms(s.elapsed()));
    }
    println!("{}", stats("drag, renderer + hand-off", handoff));
    println!("{}", stats("drag, whole studio window render", window));
    let (ws, pv) = memory_mib();
    println!("memory, after drag: {ws:.1} MiB working set, {pv:.1} MiB private");

    // Does the layer follow the cursor? Worst gap on screen, in logical px.
    println!("layer follows cursor (worst gap while dragging):");
    for zoom in [0.5_f32, 1.0, 2.0] {
        println!("  zoom {zoom:.1}: {:.2} px", drag_follow(studio, zoom));
    }

    // Alignment: the exact green layer against the magenta box Slint draws.
    println!("alignment of the Slint box to the Rust layer (box should be 1 px outside the layer):");
    let mut worst = 0;
    for zoom in [0.5_f32, 1.0, 1.37, 2.0, 3.0] {
        studio.set_zoom(zoom);
        studio.set_layer_cx(173.3);
        studio.set_layer_cy(281.7);
        studio.set_frame(Image::from_rgba8_premultiplied(render(read_view(studio, 0.0))));
        let snap = studio.window().take_snapshot().expect("snapshot");
        let layer = bbox(&snap, |p| [p.r, p.g, p.b] == LAYER_RGB);
        let boxed = bbox(&snap, |p| p.r > 200 && p.b > 200 && p.g < 60);
        match (layer, boxed) {
            (Some(l), Some(b)) => {
                let d = [b[0] - (l[0] - 1), b[1] - (l[1] - 1), b[2] - (l[2] + 1), b[3] - (l[3] + 1)];
                let err = d.iter().map(|v| v.abs()).max().unwrap();
                worst = worst.max(err);
                println!("  zoom {zoom:.2}: layer {l:?}, box {b:?}, edge error {d:?} px");
            }
            _ => println!("  zoom {zoom:.2}: layer or box not found in the snapshot"),
        }
    }
    println!("worst edge error: {worst} px");

    // Optional pictures of both windows, to check the look by eye.
    if let Some(dir) = std::env::var_os("PROTO_SNAPSHOT_DIR") {
        studio.set_zoom(1.0);
        studio.set_layer_cx(180.0);
        studio.set_layer_cy(290.0);
        studio.set_frame(Image::from_rgba8_premultiplied(render(read_view(studio, 0.0))));
        for (name, snap) in [
            ("settings", settings.window().take_snapshot().unwrap()),
            ("studio", studio.window().take_snapshot().unwrap()),
        ] {
            let mut pm = Pixmap::new(snap.width(), snap.height()).unwrap();
            pm.data_mut().copy_from_slice(snap.as_bytes());
            let path = std::path::Path::new(&dir).join(format!("{name}.png"));
            pm.save_png(&path).unwrap();
            println!("saved {}", path.display());
        }
    }
}
