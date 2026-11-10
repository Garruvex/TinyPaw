//! Native Lens lifecycle. App supplies intent and live host windows;
//! this controller owns native resources, fallback and source transitions.
use super::{
    displays::Display,
    windows::{DesktopLens, Lens},
};
use crate::features::upcoming;

#[derive(Default)]
pub struct Controller {
    window: Option<Lens>,
    desktop: Option<DesktopLens>,
    desktop_failed: bool,
    source: String,
}

pub struct Input<'a> {
    pub tool: bool,
    pub anywhere: bool,
    pub source: &'a str,
    pub source_window: Option<isize>,
    pub destination: Option<(isize, &'a Display)>,
    pub exclude: &'a [isize],
    pub size: f32,
    pub zoom: f32,
}

#[derive(Default)]
pub struct Update {
    pub notice: Option<String>,
    pub stop_anywhere: bool,
    pub return_to_point: bool,
}

impl Controller {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, input: Input<'_>) -> Update {
        let mut result = Update::default();
        if self.source != input.source {
            self.reset();
            self.source = input.source.to_owned();
        }
        if !input.tool {
            self.window = None;
        }
        if !input.tool && !input.anywhere {
            self.reset();
            return result;
        }
        if (input.anywhere || input.source == upcoming::DESKTOP) && !self.desktop_failed {
            let configured = match self.desktop.as_mut() {
                Some(lens) => lens.configure(input.size, input.zoom, input.exclude),
                None => DesktopLens::new(input.size, input.zoom).and_then(|mut lens| {
                    lens.configure(input.size, input.zoom, input.exclude)?;
                    self.desktop = Some(lens);
                    Ok(())
                }),
            };
            match configured {
                Ok(()) => {
                    self.window = None;
                    return result;
                }
                Err(why) => {
                    self.desktop = None;
                    self.desktop_failed = true;
                    if input.anywhere && !input.tool {
                        result.stop_anywhere = true;
                        result.notice = Some(format!("Lens unavailable: {why}"));
                        return result;
                    }
                    result.notice = Some(format!(
                        "Desktop lens unavailable: {why}. Using the window lens."
                    ));
                }
            }
        }
        self.desktop = None;
        if !input.tool {
            return result;
        }
        let Some((hwnd, display)) = input.destination else {
            self.window = None;
            return result;
        };
        if self
            .window
            .as_ref()
            .is_some_and(|l| l.destination() != hwnd)
        {
            self.window = None;
        }
        let updated = (|| {
            if self.window.is_none() {
                let source = input
                    .source_window
                    .ok_or("Lens source is no longer available; choose a window in Settings")?;
                self.window = Some(Lens::new(hwnd, source)?);
            }
            self.window
                .as_ref()
                .unwrap()
                .update(display, input.size, input.zoom)
        })();
        if let Err(why) = updated {
            self.window = None;
            result.notice = Some(why);
            result.return_to_point = true;
        }
        result
    }
}
