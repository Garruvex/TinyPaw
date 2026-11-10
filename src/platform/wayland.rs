//! Niri/Wayland adapter. Overlays are wlr-layer-shell surfaces on the
//! `overlay` layer, one per selected output, created through iced_exwlshell.
//! Click-through is an empty input region; Escape works because the surface
//! under the pointer takes exclusive keyboard focus while annotating. Global
//! shortcuts come from a compositor binding to `tinypaw toggle` (see
//! docs/niri.md), so this module has no key grabs of its own.
//!
//! Output discovery and the layer-shell check use a short-lived client
//! connection, separate from the one the Iced runtime owns, so they work
//! before any surface exists and when the runtime cannot show overlays.

use std::sync::OnceLock;

use iced_exwlshell::actions::{IcedXdgWindowSettings, RegionActionCB};
use iced_exwlshell::reexport::{
    Anchor, KeyboardInteractivity, Layer, LayerSize, NewLayerShellSettings, OutputOption, PixelSize,
};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};

use super::displays::Display;

/// The layer-shell namespace, which Niri `layer-rule`s can match.
pub const NAMESPACE: &str = "tinypaw";

/// One `wl_output` as the compositor described it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutputInfo {
    /// Connector name such as `DP-1` (wl_output v4).
    pub name: Option<String>,
    /// Top-left in the compositor's global space.
    pub x: i32,
    pub y: i32,
    /// Current mode in physical pixels, after the output transform.
    pub width: i32,
    pub height: i32,
    /// Integer buffer scale; fractional scales round up here.
    pub scale: i32,
}

/// What this session offers TinyPaw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    pub wayland: bool,
    pub layer_shell: bool,
    pub outputs: Vec<OutputInfo>,
}

/// Why overlays cannot open here, or `Ok` when they can.
pub fn support(probe: &Probe) -> Result<(), &'static str> {
    if !probe.wayland {
        Err("TinyPaw on Linux needs a Wayland session such as Niri. X11 is not supported yet.")
    } else if !probe.layer_shell {
        Err(
            "This compositor does not offer layer-shell, so TinyPaw cannot draw over other \
             windows. Settings still work; annotate from Niri or another layer-shell compositor.",
        )
    } else {
        Ok(())
    }
}

/// The layer-shell check, made once per process: support never changes while
/// the compositor runs.
pub fn overlay_support() -> Result<(), &'static str> {
    static CACHED: OnceLock<Result<(), &'static str>> = OnceLock::new();
    *CACHED.get_or_init(|| support(&probe()))
}

/// Connects, lists globals and reads every output's description.
pub fn probe() -> Probe {
    let Ok(conn) = Connection::connect_to_env() else {
        return Probe::default();
    };
    let Ok((globals, mut queue)) = registry_queue_init::<State>(&conn) else {
        return Probe::default();
    };
    let qh = queue.handle();
    let mut state = State::default();
    let mut layer_shell = false;
    globals.contents().with_list(|list| {
        for global in list {
            match global.interface.as_str() {
                "zwlr_layer_shell_v1" => layer_shell = true,
                "wl_output" => {
                    let version = global.version.min(4);
                    let output: WlOutput =
                        globals
                            .registry()
                            .bind(global.name, version, &qh, state.outputs.len());
                    state.outputs.push((output, Partial::default()));
                }
                _ => {}
            }
        }
    });
    // One round trip delivers each output's geometry, mode, scale and name.
    let _ = queue.roundtrip(&mut state);
    let outputs = state
        .outputs
        .iter()
        .map(|(output, partial)| {
            let info = partial.finish();
            if output.version() >= 3 {
                output.release();
            }
            info
        })
        .collect();
    Probe {
        wayland: true,
        layer_shell,
        outputs,
    }
}

/// The session's displays, in the order and shape `displays::selected` uses.
pub fn displays() -> Vec<Display> {
    to_displays(&probe().outputs)
}

/// Sorts by connector name and numbers the outputs. Wayland has no primary
/// output, so the one at the global origin plays that part, else the first.
pub fn to_displays(outputs: &[OutputInfo]) -> Vec<Display> {
    let mut items: Vec<Display> = outputs
        .iter()
        .enumerate()
        .map(|(i, o)| Display {
            id: crate::session::OutputId(0),
            name: super::intern(o.name.as_deref().unwrap_or(&format!("output-{i}"))),
            x: o.x,
            y: o.y,
            width: o.width,
            height: o.height,
            primary: false,
            dpi: 96 * o.scale.max(1) as u32,
        })
        .collect();
    items.sort_by_key(|d| d.name);
    let primary = items.iter().position(|d| d.x == 0 && d.y == 0).unwrap_or(0);
    for (i, d) in items.iter_mut().enumerate() {
        d.id = crate::session::OutputId(i as u32);
        d.primary = i == primary;
    }
    items
}

/// A full-output surface above fullscreen windows. It starts click-through
/// and without keyboard focus; the app grants both once the session wants
/// input, so opening never steals a click from the slide underneath.
pub fn overlay(display: &Display) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        layer: Layer::Overlay,
        anchor: Anchor::all(),
        // -1: cover panels too, and never push other surfaces aside.
        exclusive_zone: Some(-1),
        margin: None,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(display.name.to_owned()),
        events_transparent: true,
        namespace: Some(NAMESPACE.to_owned()),
        ..NewLayerShellSettings::default()
    }
}

/// Input region for passthrough (empty, so every click reaches the window
/// below) or annotating (the whole surface).
pub fn input_region(interactive: bool) -> RegionActionCB {
    RegionActionCB::new(move |region| {
        if interactive {
            let (w, h) = region.surface_size();
            // Region rectangles are in surface coordinates; the physical size
            // is never smaller, so this covers the surface at any scale.
            let _ = region.add(0, 0, w as i32, h as i32);
        }
    })
}

/// Keyboard focus for an overlay: exclusive while annotating so Escape and
/// the overlay shortcuts arrive, none in passthrough so typing reaches the
/// focused window.
pub fn keyboard(focused: bool) -> KeyboardInteractivity {
    if focused {
        KeyboardInteractivity::Exclusive
    } else {
        KeyboardInteractivity::None
    }
}

/// An ordinary xdg toplevel for settings and the first-run card.
pub fn panel(width: f32, height: f32) -> IcedXdgWindowSettings {
    IcedXdgWindowSettings {
        size: Some(PixelSize::px(width.max(1.0) as u32, height.max(1.0) as u32)),
        app_id: Some(NAMESPACE.to_owned()),
        ..IcedXdgWindowSettings::default()
    }
}

#[derive(Debug, Default, Clone)]
struct Partial {
    name: Option<String>,
    x: i32,
    y: i32,
    mode: Option<(i32, i32)>,
    swap_axes: bool,
    scale: i32,
}

impl Partial {
    fn finish(&self) -> OutputInfo {
        let (mut width, mut height) = self.mode.unwrap_or((0, 0));
        if self.swap_axes {
            std::mem::swap(&mut width, &mut height);
        }
        OutputInfo {
            name: self.name.clone(),
            x: self.x,
            y: self.y,
            width,
            height,
            scale: self.scale.max(1),
        }
    }
}

#[derive(Default)]
struct State {
    outputs: Vec<(WlOutput, Partial)>,
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Hotplug reaches the app through the runtime's shell broadcast.
    }
}

impl Dispatch<WlOutput, usize> for State {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some((_, partial)) = state.outputs.get_mut(*index) else {
            return;
        };
        apply(partial, event);
    }
}

fn apply(partial: &mut Partial, event: wl_output::Event) {
    use wl_output::{Mode, Transform};
    match event {
        wl_output::Event::Geometry {
            x, y, transform, ..
        } => {
            partial.x = x;
            partial.y = y;
            partial.swap_axes = matches!(
                transform,
                WEnum::Value(
                    Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
                )
            );
        }
        wl_output::Event::Mode {
            flags,
            width,
            height,
            ..
        } => {
            let current = matches!(flags, WEnum::Value(f) if f.contains(Mode::Current));
            if current || partial.mode.is_none() {
                partial.mode = Some((width, height));
            }
        }
        wl_output::Event::Scale { factor } => partial.scale = factor,
        wl_output::Event::Name { name } => partial.name = Some(name),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(name: &str, x: i32, y: i32, w: i32, h: i32, scale: i32) -> OutputInfo {
        OutputInfo {
            name: Some(name.into()),
            x,
            y,
            width: w,
            height: h,
            scale,
        }
    }

    #[test]
    fn outputs_sort_by_name_and_the_origin_output_is_primary() {
        let list = to_displays(&[
            output("HDMI-A-1", 2560, 0, 1920, 1080, 1),
            output("DP-1", 0, 0, 3840, 2160, 2),
        ]);
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].name, list[0].primary), ("DP-1", true));
        assert_eq!((list[1].name, list[1].primary), ("HDMI-A-1", false));
        assert_eq!(list[0].dpi, 192);
        assert_eq!(list[1].id, crate::session::OutputId(1));
        assert_eq!(
            super::super::displays::selected(&list, "primary")[0].name,
            "DP-1"
        );
    }

    #[test]
    fn without_an_origin_output_the_first_is_primary_and_names_are_filled() {
        let mut nameless = output("", -1920, 0, 1920, 1080, 0);
        nameless.name = None;
        let list = to_displays(&[nameless, output("eDP-1", 0, 200, 2880, 1800, 2)]);
        assert_eq!(list[0].name, "eDP-1");
        assert!(list[0].primary);
        assert_eq!(list[1].name, "output-0");
        assert_eq!(list[1].dpi, 96, "a zero scale reads as 1");
    }

    #[test]
    fn rotated_outputs_swap_axes_and_the_current_mode_wins() {
        let mut p = Partial::default();
        apply(
            &mut p,
            wl_output::Event::Mode {
                flags: WEnum::Value(wl_output::Mode::Preferred),
                width: 1280,
                height: 720,
                refresh: 60_000,
            },
        );
        apply(
            &mut p,
            wl_output::Event::Mode {
                flags: WEnum::Value(wl_output::Mode::Current),
                width: 1920,
                height: 1080,
                refresh: 60_000,
            },
        );
        apply(
            &mut p,
            wl_output::Event::Geometry {
                x: 10,
                y: 20,
                physical_width: 0,
                physical_height: 0,
                subpixel: WEnum::Value(wl_output::Subpixel::Unknown),
                make: String::new(),
                model: String::new(),
                transform: WEnum::Value(wl_output::Transform::_90),
            },
        );
        let info = p.finish();
        assert_eq!((info.width, info.height), (1080, 1920));
        assert_eq!((info.x, info.y, info.scale), (10, 20, 1));
    }

    #[test]
    fn support_explains_what_is_missing() {
        assert!(support(&Probe::default()).unwrap_err().contains("Wayland"));
        let no_layer = Probe {
            wayland: true,
            ..Probe::default()
        };
        assert!(support(&no_layer).unwrap_err().contains("layer-shell"));
        let niri = Probe {
            wayland: true,
            layer_shell: true,
            outputs: Vec::new(),
        };
        assert_eq!(support(&niri), Ok(()));
    }

    #[test]
    fn overlays_start_click_through_on_their_output() {
        let d = &to_displays(&[output("DP-2", 0, 0, 1920, 1080, 1)])[0];
        let s = overlay(d);
        assert_eq!(s.layer, Layer::Overlay);
        assert_eq!(s.anchor, Anchor::all());
        assert_eq!(s.exclusive_zone, Some(-1));
        assert!(s.events_transparent);
        assert_eq!(s.keyboard_interactivity, KeyboardInteractivity::None);
        assert_eq!(s.output_option, OutputOption::OutputName("DP-2".into()));
        assert_eq!(s.namespace.as_deref(), Some(NAMESPACE));
        assert_eq!(keyboard(true), KeyboardInteractivity::Exclusive);
        assert_eq!(keyboard(false), KeyboardInteractivity::None);
    }
}
