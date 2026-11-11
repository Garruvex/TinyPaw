use super::*;

impl App {
    /// A Lens or Spotlight anywhere shortcut went down or up, or the CLI
    /// asked for it on or off.
    pub(super) fn anywhere(&mut self, target: Target, edge: Edge, now: Millis) {
        let toggle = match target {
            Target::Lens => &mut self.lens_anywhere,
            Target::Spotlight => &mut self.spotlight_anywhere,
        };
        match edge {
            Edge::Press => toggle.press(now),
            Edge::Release => toggle.release(now),
            Edge::Show => toggle.set(true),
            Edge::Hide => toggle.set(false),
        }
        if target == Target::Spotlight {
            #[cfg(not(windows))]
            if self.spotlight_anywhere.on() {
                self.spotlight_anywhere.set(false);
                self.notice = Some("Spotlight anywhere is available on Windows".into());
            }
            self.track_spot_cursor();
        }
    }

    /// Escape: hides Lens and Spotlight anywhere.
    pub(super) fn dismiss_anywhere(&mut self, now: Millis) {
        if self.lens_anywhere.on() {
            self.anywhere(Target::Lens, Edge::Hide, now);
        }
        if self.spotlight_anywhere.on() {
            self.anywhere(Target::Spotlight, Edge::Hide, now);
        }
    }

    /// Up/Down/+/- while an anywhere shortcut is held. Spotlight wins when
    /// both are held, since it was most likely pressed second.
    pub(super) fn adjust_anywhere(&mut self, key: Adjust, out: &mut Outcomes) {
        let target = if self.spotlight_anywhere.held() {
            self.spotlight_anywhere.adjusted();
            Target::Spotlight
        } else if self.lens_anywhere.held() {
            self.lens_anywhere.adjusted();
            Target::Lens
        } else {
            return;
        };
        out.sets
            .extend(anywhere::adjust(target, key, self.session.settings()));
    }

    /// Polls the cursor for Spotlight anywhere, mapped into the logical
    /// space of the overlay under it. Annotating uses the session's pointer.
    pub(super) fn track_spot_cursor(&mut self) {
        self.spot_cursor = None;
        if !self.spotlight_anywhere.on() {
            return;
        }
        #[cfg(windows)]
        if let Some((x, y)) = platform::windows::cursor_pos() {
            self.spot_cursor = self
                .overlays
                .iter()
                .map(|o| &o.display)
                .find(|d| x >= d.x && x < d.x + d.width && y >= d.y && y < d.y + d.height)
                .map(|d| {
                    let scale = d.dpi.max(1) as f32 / 96.0;
                    (
                        d.id,
                        Pt::new((x - d.x) as f32 / scale, (y - d.y) as f32 / scale),
                    )
                });
        }
    }

    /// The Spotlight circle to draw: the Spotlight tool, else Spotlight
    /// anywhere. Neither shows while a still is being captured.
    pub(super) fn spotlight(&self) -> Option<(OutputId, Pt, f32)> {
        if let Some(spot) = self.session.spotlight() {
            return Some(spot);
        }
        if !self.spotlight_anywhere.on() || self.session.capturing() {
            return None;
        }
        let at = if self.session.is_pointing() {
            self.session.pointer()
        } else {
            self.spot_cursor
        };
        let radius = self
            .session
            .settings()
            .float(crate::features::spotlight::RADIUS) as f32;
        at.map(|(output, at)| (output, at, radius))
    }

    pub(super) fn update_lens(&mut self, now: Millis, out: &mut Outcomes) {
        // The still covers the desktop while frozen, and capture must not
        // see the lens, so neither lens shows then.
        let clear = !self.session.capturing() && self.session.frozen_frame().is_none();
        let tool = self.session.is_pointing()
            && self.session.active_tool() == crate::features::upcoming::TOOL
            && clear;
        let anywhere = self.lens_anywhere.on() && clear;
        #[cfg(windows)]
        {
            use crate::features::upcoming;
            let s = self.session.settings();
            let source = s.choice_text(upcoming::SOURCE);
            let source_window = if matches!(source, upcoming::FOREGROUND | upcoming::DESKTOP) {
                self.previous_focus
            } else {
                self.sources
                    .iter()
                    .find(|w| w.id == source)
                    .map(|w| w.handle)
            };
            let output = self.session.pointer().map(|(o, _)| o);
            let destination = self
                .overlays
                .iter()
                .find(|o| Some(o.display.id) == output && o.hwnd != 0)
                .map(|o| (o.hwnd, &o.display));
            let mut exclude: Vec<_> = self
                .overlays
                .iter()
                .map(|o| o.hwnd)
                .filter(|h| *h != 0)
                .collect();
            if let Some(m) = &self.mascot {
                exclude.push(m.hwnd());
            }
            exclude.extend(
                self.helper_hwnds
                    .iter()
                    .filter(|(id, hwnd)| {
                        **hwnd != 0
                            && (Some(**id) == self.settings_window
                                || Some(**id) == self.first_run_window)
                    })
                    .map(|(_, hwnd)| *hwnd),
            );
            let result = self.lens.update(platform::lens::Input {
                tool,
                anywhere,
                source,
                source_window,
                destination,
                exclude: &exclude,
                size: s.float(upcoming::SIZE) as f32,
                zoom: anywhere::zoom(s),
            });
            if let Some(notice) = result.notice {
                self.overlay_notice = Some((notice.clone(), now));
                if result.stop_anywhere {
                    self.notice = Some(notice);
                }
            }
            if result.stop_anywhere {
                self.lens_anywhere.set(false);
            }
            if result.return_to_point {
                out.collect(self.dispatch(point::SELECT, now).ok());
            }
        }
        #[cfg(not(windows))]
        {
            if self.lens_anywhere.on() {
                self.lens_anywhere.set(false);
                self.notice = Some("Lens is available on Windows".into());
            }
            if tool {
                self.overlay_notice = Some(("Lens is unavailable on this desktop".into(), now));
                out.collect(self.dispatch(point::SELECT, now).ok());
            }
        }
    }
}
