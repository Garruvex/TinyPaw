//! Pointer page: pack choice, the new-pointer editor and previews.

use super::*;
use iced::widget::{column, row};

/// The pointer's look, plus the preview `main` keeps in view.
pub(super) fn pointer_page<'a>(
    v: &SettingsView<'a>,
) -> (Element<'a, Message>, Option<Element<'a, Message>>) {
    let s = v.session.settings();
    let laser = s.choice(point::STYLE) == "laser";

    let card = |art: Element<'a, Message>, label: String, on: bool, msg: Message| {
        button(
            column![
                container(art)
                    .height(52)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
                text(label).font(theme::MEDIUM).size(12),
            ]
            .spacing(6)
            .width(Length::Fill)
            .align_x(Alignment::Center),
        )
        .width(92)
        .height(96)
        .padding([10, 6])
        .style(theme::card(on))
        .on_press(msg)
    };
    let current = v.pack.map(|p| p.id);
    let mut pointers = row(v.packs.iter().map(|p| {
        card(
            iced::widget::image(p.image.clone()).height(48).into(),
            card_label(&p.name),
            !laser && current == Some(p.id),
            Message::Ui(UiMsg::PickPack(p.id)),
        )
        .into()
    }))
    .spacing(8);
    let laser_tint = Color::from_rgb8(0xF8, 0x71, 0x71);
    pointers = pointers.push(card(
        container(icon(Glyph::Laser, 22.0, laser_tint))
            .width(40)
            .height(40)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::fill(
                Color {
                    a: 0.14,
                    ..laser_tint
                },
                20.0,
            ))
            .into(),
        "Laser".into(),
        laser,
        Message::Set(point::STYLE, Value::Choice("laser")),
    ));
    pointers = pointers.push(card(
        container(icon(Glyph::Plus, 22.0, theme::MUTED))
            .width(40)
            .height(40)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::outlined(
                Color::TRANSPARENT,
                20.0,
                theme::LINE_STRONG,
            ))
            .into(),
        "Add pointer".into(),
        false,
        Message::Ui(UiMsg::AddPointer),
    ));
    let pointers = pointers.wrap().vertical_spacing(8);

    let pres = s.choice(point::PRESENTATION);
    let seg = |label: &'static str, id: &'static str, enabled: bool| -> Element<'a, Message> {
        let b = button(text(label).size(13))
            .padding([7, 12])
            .style(theme::segment(pres == id))
            .on_press_maybe(
                enabled.then_some(Message::Set(point::PRESENTATION, Value::Choice(id))),
            );
        if enabled { b.into() } else { later(b) }
    };
    let presentation = container(
        row![
            seg("Furry Paw", "furry", true),
            seg("Stretch Paw", "stretch", true),
            seg("Classic Stick", "stick", true),
        ]
        .spacing(4),
    )
    .padding(4)
    .style(theme::fill(theme::WELL, 10.0));
    let pres_hint = match pres {
        "stick" => "The paw on a slim rod from an off-screen grip, lower right.",
        "stretch" => {
            "A curved forearm follows the paw. Packs without forearm art use their accent colour."
        }
        _ => "A compact paw at the pointer.",
    };

    let mut stamps = row![].spacing(4);
    for (id, label) in [("off", "Off"), ("cat", "Cat"), ("dog", "Dog")] {
        stamps = stamps.push(
            button(text(label).size(13))
                .padding([7, 12])
                .style(theme::segment(s.choice(point::TAP_STAMP) == id))
                .on_press(Message::Set(point::TAP_STAMP, Value::Choice(id))),
        );
    }

    let size = s.float(point::SIZE);
    let size_row = row![
        h2("Size"),
        space().width(Length::Fill),
        text(format!("{size:.0} px")).size(13).color(theme::TEXT),
    ];
    let size_slider = slider(v.registry.range(point::SIZE), size, |x| {
        Message::Set(point::SIZE, Value::Float(x))
    })
    .step(4.0)
    .style(theme::range);

    let current_ink = s.color(system::INK_COLOR);
    let angle = s.float(point::ANGLE);
    let angle_control = section(vec![
        row![
            h2("Paw angle"),
            space().width(Length::Fill),
            text(format!("{angle:.0}°")).size(13).color(theme::TEXT),
        ]
        .into(),
        slider(v.registry.range(point::ANGLE), angle, |x| {
            Message::Set(point::ANGLE, Value::Float(x))
        })
        .step(5.0)
        .style(theme::range)
        .into(),
        hint("Positive angles reach from the lower right."),
    ]);
    let width = s.float(system::INK_WIDTH);
    let mut left = column![
        h1("Pointer"),
        section(vec![h2("Choose a pointer"), pointers.into()]),
    ]
    .spacing(22)
    .width(Length::Fill);

    if !laser {
        let mut appearance = column![
            h2("Paw appearance"),
            presentation,
            hint(pres_hint),
            size_row,
            size_slider
        ]
        .spacing(10);
        if pres == "furry" {
            appearance = appearance.push(angle_control);
        }
        if pres == "stretch" && v.pack.is_none_or(|p| p.forearm.is_none()) {
            appearance = appearance.push(forearm_colors(s, v.pack.and_then(|p| p.accent)));
        }
        left = left.push(appearance).push(section(vec![
            h2("Tap stamp"),
            row![
                stamps,
                row![hint("Fade"), fade_picker(s)]
                    .spacing(8)
                    .align_y(Alignment::Center)
            ]
            .spacing(16)
            .align_y(Alignment::Center)
            .wrap()
            .into(),
            hint_link(
                "Each tap leaves a paw print in your ink colour. Fade is shared with ink.",
                "Ink settings",
                Message::Ui(UiMsg::Nav(Page::Ink)),
            ),
        ]));
    }
    let mut actions = row![
        button("Import folder")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::ImportPack))
    ]
    .spacing(8);
    if !laser && v.pack.is_some_and(|p| p.folder.is_some()) {
        actions = actions
            .push(
                button("Re-aim pointer")
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::EditPack)),
            )
            .push(
                button("Remove pointer")
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::RemovePack)),
            );
    }
    left = left.push(section(vec![
        h2("Custom pointers"),
        actions.wrap().into(),
        hint("To create matching artwork, share a prompt and your character reference with an artist or image tool."),
        row![
            button("Copy pointer prompt").style(theme::secondary).on_press(Message::Ui(UiMsg::CopyPointerPrompt)),
            button("Copy forearm prompt").style(theme::secondary).on_press(Message::Ui(UiMsg::CopyForearmPrompt)),
        ].spacing(8).wrap().into(),
    ]));

    let ink = theme::rgba(current_ink);
    let tile = |bg: Color| {
        canvas(PreviewTile {
            forearm_color: point::forearm_color(s),
            background: bg,
            ink,
            width: width as f32,
            laser,
            stick: pres == "stick",
            stretch: pres == "stretch",
            scale: size as f32 / point::BASE_SIZE,
            angle: -(angle as f32).to_radians(),
            pack: v.pack.cloned(),
        })
        .width(Length::Fill)
        .height(150)
    };
    if let Some(draft) = v.draft {
        return (left.into(), Some(new_pointer(draft, v.editing, s)));
    }
    let preview = column![
        h2("Preview"),
        tile(Color::from_rgb8(0xF4, 0xF4, 0xF5)),
        tile(Color::from_rgb8(0x1B, 0x26, 0x21)),
        button(
            text("Preview tap sound")
                .font(theme::MEDIUM)
                .size(13)
                .width(Length::Fill)
                .center()
        )
        .width(Length::Fill)
        .padding([11, 12])
        .style(theme::secondary)
        .on_press(Message::Ui(UiMsg::PreviewSound)),
    ]
    .spacing(10)
    .width(260);

    (left.into(), Some(preview.into()))
}

/// Colour controls apply only to the generated forearm, leaving paw art intact.
pub(super) fn forearm_colors<'a>(
    s: &'a crate::registry::SettingValues,
    accent: Option<[u8; 4]>,
) -> Element<'a, Message> {
    let automatic = s.bool(point::FOREARM_AUTO_COLOR);
    let color = if automatic {
        accent.unwrap_or([0xF2, 0xA6, 0x5A, 255])
    } else {
        s.color(point::FOREARM_COLOR)
    };
    let palette = [
        [36, 39, 60, 255],
        [52, 54, 130, 255],
        [91, 155, 255, 255],
        [255, 255, 255, 255],
        [90, 90, 98, 255],
        [242, 166, 90, 255],
        [170, 100, 64, 255],
        [244, 154, 168, 255],
        [164, 112, 220, 255],
        [104, 184, 128, 255],
    ];
    let swatches = row(palette.into_iter().map(|c| {
        button(space().width(24).height(24))
            .padding(0)
            .style(theme::swatch(theme::rgba(c), c == color))
            .on_press(Message::Ui(UiMsg::ForearmColor(c)))
            .into()
    }))
    .spacing(6)
    .wrap();
    let mut controls = column![
        h2("Forearm colour"),
        hint("Used by Stretch Paw when no forearm PNG is attached."),
        check("Use pointer colour", automatic, point::FOREARM_AUTO_COLOR),
        swatches,
    ]
    .spacing(8);
    for (channel, label) in ["Red", "Green", "Blue"].into_iter().enumerate() {
        controls = controls.push(
            row![
                container(hint(label)).width(42),
                slider(0.0..=255.0, color[channel] as f32, move |value| {
                    let mut next = color;
                    next[channel] = value.round() as u8;
                    Message::Ui(UiMsg::ForearmColor(next))
                })
                .step(1.0_f32)
                .style(theme::range),
                text(color[channel].to_string()).size(12).width(28),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    section(vec![controls.into()])
}

/// "Add your pointer": click the exact tip, name it, save.
pub(super) fn new_pointer<'a>(
    draft: &'a Draft,
    editing: bool,
    settings: &'a crate::registry::SettingValues,
) -> Element<'a, Message> {
    if editing {
        return column![
            h2("Re-aim pointer"),
            canvas(HotspotPicker {
                draft,
                forearm: false
            })
            .width(Length::Fill)
            .height(180),
            hint("Click the exact pointing tip. Both idle and pressed art keep this anchor."),
            row![
                button("Cancel")
                    .padding([8, 12])
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::CancelDraft)),
                button("Save click point")
                    .padding([8, 12])
                    .style(theme::primary)
                    .on_press(Message::Ui(UiMsg::SaveDraft))
            ]
            .spacing(8),
        ]
        .spacing(10)
        .width(260)
        .into();
    }
    let arm: Element<'a, Message> = if draft.forearm.is_some() {
        column![
            h2("Forearm attachment"),
            canvas(HotspotPicker {
                draft,
                forearm: true
            })
            .width(Length::Fill)
            .height(130),
            hint("Click the wrist joint at the top of the forearm."),
            button(text("Remove forearm").size(13))
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::RemoveForearm)),
        ]
        .spacing(8)
        .into()
    } else {
        button(text("Add matching forearm PNG").size(13))
            .padding([8, 12])
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::AddForearm))
            .into()
    };
    column![
        h2("New pointer"),
        canvas(HotspotPicker {
            draft,
            forearm: false
        })
        .width(Length::Fill)
        .height(150),
        hint(
            "Click the exact tip of the pointer. Light and dark halves show how it reads on both."
        ),
        button(if draft.pressed.is_some() {
            "Replace pressed pose"
        } else {
            "Add pressed pose PNG"
        })
        .padding([8, 12])
        .style(theme::secondary)
        .on_press(Message::Ui(UiMsg::AddPressed)),
        button(if draft.tap.is_some() {
            "Replace pack tap WAV"
        } else {
            "Add default tap WAV"
        })
        .padding([8, 12])
        .style(theme::secondary)
        .on_press(Message::Ui(UiMsg::AddPackTap)),
        h2("Paw wrist position"),
        hint("Horizontal"),
        slider(0.0..=1.0, draft.wrist.0, |x| Message::Ui(
            UiMsg::DraftWristX(x)
        ))
        .step(0.01_f32),
        hint("Vertical"),
        slider(0.0..=1.0, draft.wrist.1, |y| Message::Ui(
            UiMsg::DraftWristY(y)
        ))
        .step(0.01_f32),
        h2("Forearm width"),
        slider(0.05..=1.5, draft.joint_width, |w| Message::Ui(
            UiMsg::DraftJointWidth(w)
        ))
        .step(0.01_f32),
        arm,
        if draft.forearm.is_none() {
            forearm_colors(settings, None)
        } else {
            space().into()
        },
        h2("Test the pair"),
        canvas(PairPreview {
            pack: draft.preview(),
            forearm_color: point::forearm_color(settings),
        })
        .width(Length::Fill)
        .height(220),
        hint("Move and hold the mouse in the preview to check bending and pressing."),
        iced::widget::text_input("Name", &draft.name)
            .on_input(|n| Message::Ui(UiMsg::DraftName(n)))
            .size(14)
            .padding([9, 10])
            .style(|_, status| iced::widget::text_input::Style {
                background: theme::CONTROL.into(),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                        theme::CYAN_RING
                    } else {
                        theme::LINE_STRONG
                    },
                },
                icon: theme::MUTED,
                placeholder: theme::FAINT,
                value: theme::TEXT,
                selection: theme::CYAN_BG,
            }),
        row![
            button(text("Cancel").size(13).width(Length::Fill).center())
                .width(Length::Fill)
                .padding([10, 12])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::CancelDraft)),
            button(
                text("Save pointer")
                    .font(theme::HEADING)
                    .size(13)
                    .width(Length::Fill)
                    .center()
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(theme::primary)
            .on_press(Message::Ui(UiMsg::SaveDraft)),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .width(260)
    .into()
}

/// Shows the new pointer on light and dark halves with a crosshair at the
/// hotspot; a click inside the image moves the hotspot there.
pub(super) struct HotspotPicker<'a> {
    draft: &'a Draft,
    forearm: bool,
}

impl HotspotPicker<'_> {
    fn image_rect(&self, bounds: iced::Size) -> iced::Rectangle {
        let pad = 16.0;
        let (w, h) = self
            .draft
            .forearm
            .as_ref()
            .filter(|_| self.forearm)
            .map_or((self.draft.width as f32, self.draft.height as f32), |a| {
                (a.width as f32, a.height as f32)
            });
        let fit = ((bounds.width - pad * 2.0) / w).min((bounds.height - pad * 2.0) / h);
        let size = iced::Size::new(w * fit, h * fit);
        iced::Rectangle::new(
            iced::Point::new(
                (bounds.width - size.width) / 2.0,
                (bounds.height - size.height) / 2.0,
            ),
            size,
        )
    }
}

impl canvas::Program<Message> for HotspotPicker<'_> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) =
            event
            && let Some(p) = cursor.position_in(bounds)
        {
            let r = self.image_rect(bounds.size());
            if r.contains(p) {
                let fx = (p.x - r.x) / r.width;
                let fy = (p.y - r.y) / r.height;
                let message = if self.forearm {
                    UiMsg::DraftForearmAnchor(fx, fy)
                } else {
                    UiMsg::DraftHotspot(fx, fy)
                };
                return Some(canvas::Action::publish(Message::Ui(message)).and_capture());
            }
        }
        None
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path, Stroke};
        let size = bounds.size();
        let mut back = Frame::new(renderer, size);
        back.fill(
            &Path::rounded_rectangle(iced::Point::ORIGIN, size, 12.0.into()),
            Color::from_rgb8(0xF4, 0xF4, 0xF5),
        );
        back.fill_rectangle(
            iced::Point::new(size.width / 2.0, 0.0),
            iced::Size::new(size.width / 2.0, size.height),
            Color::from_rgb8(0x1B, 0x26, 0x21),
        );
        let r = self.image_rect(size);
        let (image, anchor) = self
            .draft
            .forearm
            .as_ref()
            .filter(|_| self.forearm)
            .map_or((&self.draft.image, self.draft.hotspot), |a| {
                (&a.image, a.anchor)
            });
        back.draw_image(r, canvas::Image::new(image.clone()));
        // Crosshair in its own frame: images paint above shapes in one frame.
        let mut marks = Frame::new(renderer, size);
        let hx = r.x + anchor.0 * r.width;
        let hy = r.y + anchor.1 * r.height;
        let ring = Path::circle(iced::Point::new(hx, hy), 7.0);
        marks.stroke(
            &ring,
            Stroke::default().with_color(Color::BLACK).with_width(3.0),
        );
        marks.stroke(
            &ring,
            Stroke::default().with_color(theme::CYAN).with_width(1.5),
        );
        if !self.forearm {
            let wrist = iced::Point::new(
                r.x + self.draft.wrist.0 * r.width,
                r.y + self.draft.wrist.1 * r.height,
            );
            marks.stroke(
                &Path::circle(wrist, 5.0),
                Stroke::default().with_color(theme::AMBER).with_width(2.0),
            );
        }
        vec![back.into_geometry(), marks.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        match cursor.position_in(bounds) {
            Some(p) if self.image_rect(bounds.size()).contains(p) => {
                iced::mouse::Interaction::Crosshair
            }
            _ => iced::mouse::Interaction::default(),
        }
    }
}

pub(super) struct PairPreview {
    pack: Pack,
    forearm_color: Option<[u8; 4]>,
}

pub(super) struct PairPreviewState {
    session: Session,
    started: std::time::Instant,
    // Only a press begun in this canvas owns the matching release.
    pressed: bool,
}

impl Default for PairPreviewState {
    fn default() -> Self {
        let registry = Registry::build(crate::features::BUILTINS).expect("valid built-ins");
        let mut settings = registry.defaults();
        registry
            .set(&mut settings, point::PRESENTATION, Value::Choice("stretch"))
            .unwrap();
        let mut session = Session::new(point::TOOL, point::new_state, settings);
        session.handle(crate::session::Event::MasterToggle, 0);
        let ticket = session.pending_ticket().unwrap();
        session.handle(crate::session::Event::InputAcquired(ticket), 0);
        Self {
            session,
            started: std::time::Instant::now(),
            pressed: false,
        }
    }
}

impl canvas::Program<Message> for PairPreview {
    type State = PairPreviewState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        use crate::session::{Event, OutputId, PointerEvent, PointerKind, Pt};
        let now = state.started.elapsed().as_millis() as u64;
        let position = cursor.position_in(bounds);
        let kind =
            match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })
                    if position.is_some() =>
                {
                    Some(PointerKind::Moved)
                }
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })
                    if state.session.pointer().is_some() =>
                {
                    Some(PointerKind::Left)
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) if position.is_some() => Some(PointerKind::Down),
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) if state.pressed => Some(PointerKind::Up),
                iced::Event::Mouse(iced::mouse::Event::CursorLeft) => Some(PointerKind::Left),
                _ => None,
            };
        if let Some(kind) = kind {
            match kind {
                PointerKind::Down => state.pressed = true,
                PointerKind::Up | PointerKind::Left => state.pressed = false,
                _ => {}
            }
            let p = position.unwrap_or(iced::Point::new(bounds.width * 0.4, 30.0));
            state.session.handle(
                Event::Pointer(PointerEvent {
                    kind,
                    output: OutputId(0),
                    pos: Pt::new(p.x, p.y),
                    at: now,
                    paw_angle: None,
                }),
                now,
            );
            if matches!(kind, PointerKind::Up | PointerKind::Left) {
                state.session.handle(Event::Clear, now);
            }
            return Some(canvas::Action::request_redraw().and_capture());
        }
        state
            .session
            .animating(now)
            .then(canvas::Action::request_redraw)
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _: &Theme,
        bounds: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), theme::WELL);
        let now = state.started.elapsed().as_millis() as u64;
        let tip = state
            .session
            .tip(now)
            .map(|(_, tip)| tip)
            .unwrap_or(crate::session::Tip::Paw {
                at: crate::session::Pt::new(bounds.width * 0.4, 30.0),
                press: 0.0,
                scale: 1.0,
                angle: 0.0,
                stick: false,
                stretch: true,
                grip_x: bounds.width * 0.4,
            });
        super::super::paw::draw_tip(
            &mut frame,
            tip,
            bounds.size(),
            Some(&self.pack),
            self.forearm_color,
        );
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod pointer_preview_tests {
    use super::*;
    use iced::widget::canvas::Program;

    #[test]
    fn preview_leaves_button_clicks_alone_but_finishes_its_own_press() {
        let preview = PairPreview {
            pack: crate::packs::load_all().0.remove(0),
            forearm_color: None,
        };
        let mut state = PairPreviewState::default();
        let bounds = iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(260.0, 220.0));
        let outside = iced::mouse::Cursor::Available(iced::Point::new(150.0, 300.0));
        let inside = iced::mouse::Cursor::Available(iced::Point::new(100.0, 100.0));
        let down = iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
        let up = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        ));
        let status = |action: Option<canvas::Action<Message>>| {
            action.map_or(iced::event::Status::Ignored, |a| a.into_inner().2)
        };

        // Save and Cancel are outside the preview and must get both events.
        assert_eq!(
            status(preview.update(&mut state, &down, bounds, outside)),
            iced::event::Status::Ignored
        );
        assert_eq!(
            status(preview.update(&mut state, &up, bounds, outside)),
            iced::event::Status::Ignored
        );
        assert_eq!(
            status(preview.update(&mut state, &down, bounds, inside)),
            iced::event::Status::Captured
        );
        assert_eq!(
            status(preview.update(&mut state, &up, bounds, outside)),
            iced::event::Status::Captured
        );
        assert_eq!(
            status(preview.update(&mut state, &up, bounds, outside)),
            iced::event::Status::Ignored
        );
    }
}
