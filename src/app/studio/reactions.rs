use super::*;

impl App {
    /// Reaction sounds, pictures and the idle voice.
    pub(super) fn reaction_ui(
        &mut self,
        ui: UiMsg,
        now: Millis,
        out: &mut Outcomes,
    ) -> Task<Message> {
        match ui {
            UiMsg::ReactionSound(e, sound) => {
                if let Some(p) = &mut self.motion_preview {
                    p.set_reaction_sound(e, sound);
                }
            }
            UiMsg::ReactionImport(e) => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a WAV for this reaction (five seconds or shorter)")
                        .add_filter("WAV sound", &["wav"])
                        .pick_file(),
                    move |f| Message::ReactionWavPicked(e, f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ReactionCooldown(e, ms) => {
                if let Some(p) = &mut self.motion_preview {
                    p.set_cooldown(e, ms.round().max(0.0) as u32);
                }
            }
            UiMsg::ReactionSoundsOn(on) => {
                if let Some(p) = &mut self.motion_preview {
                    p.sound_settings_mut().enabled = on;
                }
            }
            UiMsg::ReactionVolume(v) => {
                if let Some(p) = &mut self.motion_preview {
                    p.sound_settings_mut().volume = v.clamp(0.0, 1.0);
                }
            }
            UiMsg::ReactionIdleOn(on) => {
                if let Some(p) = &mut self.motion_preview {
                    p.sound_settings_mut().idle_enabled = on;
                }
            }
            UiMsg::ReactionIdleInterval(lo, hi) => {
                if let Some(p) = &mut self.motion_preview {
                    p.sound_settings_mut().idle_interval = (lo.min(hi), lo.max(hi));
                }
            }
            UiMsg::ReactionVisual(e, crate::mascot::reactions::Visual::Artwork)
                if self
                    .motion_preview
                    .as_ref()
                    .is_some_and(|p| p.art(e).is_none()) =>
            {
                // Use artwork starts by choosing its PNG.
                return self.ui_message(UiMsg::ReactionPicture(e), now, out);
            }
            UiMsg::ReactionPicture(e) => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a transparent PNG of the whole character")
                        .add_filter("PNG image", &["png"])
                        .pick_file(),
                    move |f| Message::ReactionPicturePicked(e, f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ReactionFall(fall) => {
                if let Some(p) = &mut self.motion_preview {
                    p.set_fall(fall);
                }
            }
            UiMsg::ReactionHeldClear => {
                if let Some(p) = &mut self.motion_preview {
                    p.set_held(None);
                }
            }
            UiMsg::ReactionPlace(e, held, which, value) => {
                if let Some(p) = &mut self.motion_preview {
                    p.set_place(e, held, which, value);
                    if held {
                        p.test(crate::mascot::preview::Test::DragRight);
                    }
                }
            }
            UiMsg::ReactionVisual(e, v) => {
                if let Some(p) = &mut self.motion_preview {
                    p.set_visual(e, v);
                    if e == crate::mascot::reactions::Event::Click {
                        p.test(crate::mascot::preview::Test::Click);
                    }
                }
            }
            UiMsg::ReactionsReset => {
                if let Some(p) = &mut self.motion_preview {
                    p.reset_reactions();
                }
            }
            UiMsg::ReactionTest(e) => {
                // Test plays the draft at its volume, bypassing eligibility
                // and cooldown, never moving the desktop character, and
                // obeying every mute.
                if self.session.settings().bool(sound::MUTE_ALL) {
                    self.notice = Some("Mute all sounds is on.".into());
                    return Task::none();
                }
                let Some(sounds) = self.motion_preview.as_ref().map(|p| p.sounds()) else {
                    return Task::none();
                };
                if !sounds.enabled {
                    self.notice = Some("Character sounds are off for this character.".into());
                    return Task::none();
                }
                let t = sounds.get(e);
                if let Some(sound) = t.sound.clone() {
                    let play = crate::mascot::reactions::Play {
                        sound,
                        volume: (sounds.volume * t.gain).clamp(0.0, 1.0),
                        interrupt: true,
                    };
                    if self.play_reaction(&play).is_none() && self.notice.is_none() {
                        self.notice = Some("That sound is missing or unreadable.".into());
                    }
                    self.audio.release_idle(self.session.is_enabled());
                }
            }
            _ => {}
        }
        Task::none()
    }
}
