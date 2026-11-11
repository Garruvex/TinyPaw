use super::*;

impl App {
    pub(super) fn pointer(
        &mut self,
        output: OutputId,
        kind: PointerKind,
        p: Point,
        now: Millis,
        out: &mut Outcomes,
    ) {
        if let Some(w) = self.ui.wheel.as_mut() {
            match kind {
                PointerKind::Moved => w.track(p),
                PointerKind::Down => {
                    w.track(p);
                    self.close_wheel(now, out);
                }
                _ => {}
            }
            return;
        }
        if kind == PointerKind::Down && self.ui.close_menus() {
            return; // A click outside a menu only dismisses it.
        }
        let paw_angle = (kind == PointerKind::Up)
            .then(|| {
                let (_, mut tip) = self.session.tip(now)?;
                if system::reduced(self.session.settings())
                    && let crate::session::Tip::Paw { grip_x, .. } = &mut tip
                {
                    *grip_x = p.x;
                }
                let display = &self
                    .overlays
                    .iter()
                    .find(|o| o.display.id == output)?
                    .display;
                let scale = display.dpi.max(1) as f32 / 96.0;
                crate::view::paw::stamp_angle(
                    tip,
                    p,
                    Size::new(display.width as f32 / scale, display.height as f32 / scale),
                    self.current_pack(),
                )
            })
            .flatten();
        out.effects.extend(self.session.handle(
            Event::Pointer(PointerEvent {
                kind,
                output,
                pos: Pt::new(p.x, p.y),
                at: now,
                paw_angle,
            }),
            now,
        ));
    }

    pub(super) fn open_wheel(&mut self, now: Millis) {
        if !self.session.is_pointing() {
            let _ = self.dispatch(system::LATCH, now);
        }
        self.ui.close_menus();
        self.ui.wheel = Some(wheel::WheelState {
            center: self.session.pointer().map(|(_, p)| Point::new(p.x, p.y)),
            hover: None,
        });
    }

    pub(super) fn close_wheel(&mut self, now: Millis, out: &mut Outcomes) {
        let Some(state) = self.ui.wheel.take() else {
            return;
        };
        if let Some(i) = state.hover {
            self.run(wheel::ITEMS[i].command, now, out);
        }
    }

    pub(super) fn overlay_shortcut(
        &self,
        key: &keyboard::Key,
        m: keyboard::Modifiers,
    ) -> Option<&'static str> {
        let pressed = match key.as_ref() {
            keyboard::Key::Named(Named::Escape) => Key::Escape,
            keyboard::Key::Named(Named::Space) => Key::Space,
            keyboard::Key::Character(c) => Key::Char(c.chars().next()?.to_ascii_uppercase()),
            _ => return None,
        };
        self.registry
            .shortcuts(ShortcutContext::Overlay)
            .find(|(s, _)| {
                s.key == pressed
                    && s.ctrl == m.control()
                    && s.alt == m.alt()
                    && s.shift == m.shift()
            })
            .map(|(_, def)| def.id)
    }
}
