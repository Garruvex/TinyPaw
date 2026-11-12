use super::*;

impl App {
    pub(super) fn mascot_window(&self) -> Option<window::Id> {
        #[cfg(windows)]
        {
            self.mascot.as_ref().map(|m| m.id)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    pub(super) fn mascot_options(&self) -> crate::mascot::desktop::Options {
        let s = self.session.settings();
        crate::mascot::desktop::Options {
            eyes: s.bool(character::EYES),
            idle: s.bool(character::IDLE),
            reduced: system::reduced(s),
            lines: s.bool(character::LINES),
            quiet: s.bool(character::QUIET_LINES),
            idle_every: (
                s.float(character::IDLE_MIN) as f32 * 60.0,
                s.float(character::IDLE_MAX) as f32 * 60.0,
            ),
            music: self.music_level(),
            beat: self.beat_phase(),
        }
    }

    #[cfg_attr(not(windows), allow(unused_variables))]
    pub(super) fn mascot_message(
        &mut self,
        msg: crate::mascot::Msg,
        now: Millis,
        out: &mut Outcomes,
        tasks: &mut Vec<Task<Message>>,
    ) {
        use crate::mascot::Msg;
        match msg {
            // Menu entries go through the registry like every other surface.
            Msg::Command(id) => {
                #[cfg(windows)]
                if let Some(m) = &mut self.mascot {
                    m.close_menu();
                }
                out.collect(self.dispatch(id, now).ok());
            }
            Msg::CharacterSettings => {
                #[cfg(windows)]
                if let Some(m) = &mut self.mascot {
                    m.close_menu();
                }
                self.page = Page::Character;
                tasks.push(self.open_settings());
            }
            #[cfg(windows)]
            msg => {
                let options = self.mascot_options();
                let audio = self.companion_audio();
                let voice_busy = self.voice.busy(self.voice_clock.elapsed().as_secs_f64());
                if let Some(m) = &mut self.mascot {
                    m.set_audio(audio);
                    m.set_voice_busy(voice_busy);
                    let (task, save) = m.update(msg, &options);
                    tasks.push(task);
                    if let Some((x, y)) = save {
                        out.sets.push((character::X, Value::Float(x)));
                        out.sets.push((character::Y, Value::Float(y)));
                    }
                }
                self.play_mascot_sound();
            }
            #[cfg(not(windows))]
            _ => {}
        }
    }

    /// Opens, closes, resizes and hides the character window to match its
    /// settings. It stays while annotation is Off and steps aside while the
    /// annotation surface owns input.
    pub(super) fn reconcile_mascot(&mut self) -> Task<Message> {
        #[cfg(windows)]
        {
            let s = self.session.settings();
            let want = s.bool(character::ENABLED);
            let size = s.float(character::HEIGHT) as f32;
            let hidden = self.session.overlay_state() == OverlayState::Interactive;
            // The menu is as tall as its visible rows.
            let rows = self.mascot_menu().len();
            let selected = s.choice(character::CHARACTER);
            let anchor = (s.float(character::X), s.float(character::Y));
            let stale = self
                .mascot
                .as_ref()
                .is_some_and(|m| m.character() != selected);
            let options = self.mascot_options();
            if want && (self.mascot.is_none() || stale) {
                let rig = self.character_rig(selected);
                let lines = self.character_lines(selected);
                self.stop_character_voice();
                let c = self.characters.iter().find(|c| c.id == selected);
                self.reaction_sounds = c.map(|c| c.reaction_sounds()).unwrap_or_default();
                let compress = c.is_none_or(|c| {
                    c.reaction_visual(crate::mascot::reactions::Event::Click)
                        == crate::mascot::reactions::Visual::Compress
                });
                let pictures = c.map(character_pictures).unwrap_or_default();
                let fall = c.is_some_and(|c| {
                    let user = characters::load_overrides(c.id)
                        .reactions
                        .unwrap_or_default();
                    crate::mascot::reactions::falls(&c.pack_reactions(), &user)
                });
                match &mut self.mascot {
                    Some(m) => {
                        m.set_rig(selected, rig);
                        m.set_lines(lines, &options);
                        m.set_click_compress(compress);
                        m.set_pictures(pictures);
                        m.set_fall(fall);
                    }
                    None => {
                        let (mut m, open) = crate::mascot::desktop::Desktop::open(
                            selected, rig, size, anchor, rows, hidden,
                        );
                        m.set_lines(lines, &options);
                        m.set_click_compress(compress);
                        m.set_pictures(pictures);
                        m.set_fall(fall);
                        self.mascot = Some(m);
                        return open.map(Message::WindowOpened);
                    }
                }
            }
            match (&mut self.mascot, want) {
                (None, true) => {}
                (Some(_), false) => {
                    self.stop_character_voice();
                    let m = self.mascot.take().expect("mascot window");
                    return window::close(m.id);
                }
                (Some(m), true) => {
                    m.resize(size, rows);
                    let was_hidden = m.is_hidden();
                    let task = m.set_hidden(hidden);
                    if hidden && !was_hidden {
                        // Annotation owns input: character audio stops.
                        self.voice.stop();
                        self.audio.stop_voice();
                    }
                    let Some(m) = &mut self.mascot else {
                        return task;
                    };
                    // Back from annotation: the character can say so.
                    if was_hidden && !hidden {
                        m.say(crate::mascot::lines::Trigger::AnnotationEnd, &options);
                    }
                    self.play_mascot_sound();
                    return task;
                }
                (None, false) => {}
            }
        }
        Task::none()
    }

    /// Routes the desktop character's gesture events through the one
    /// character voice, then its line sound. A trigger plays one sound:
    /// an accepted reaction sound wins over that trigger's line sound.
    pub(super) fn play_mascot_sound(&mut self) {
        #[cfg(windows)]
        {
            use crate::mascot::reactions::{Event, Quiet};
            let now = self.voice_clock.elapsed().as_secs_f64();
            let Some(m) = &mut self.mascot else {
                return;
            };
            let mut events = m.take_events();
            let (free, hidden) = (m.idle_free(), m.is_hidden());
            let line = m.take_sound();
            let pick = || (self.voice_clock.elapsed().subsec_nanos() % 1000) as f32 / 1000.0;
            if self.voice.idle_due(&self.reaction_sounds, free, now, pick)
                && !platform::windows::foreground_fullscreen()
            {
                events.push(Event::Idle);
            }
            let quiet = Quiet {
                mute_all: self.session.settings().bool(sound::MUTE_ALL),
                annotating: hidden,
                hidden,
            };
            let mut reacted = false;
            for e in events {
                if let Ok(play) = self.voice.accept(e, &self.reaction_sounds, quiet, now)
                    && let Some(seconds) = self.play_reaction(&play)
                {
                    self.voice.started(now, seconds);
                    reacted = true;
                }
            }
            if !reacted
                && let Some(sound) = line
                && let Some(seconds) = self.play_line_sound(&sound)
                && let Some(m) = &mut self.mascot
            {
                m.hold_speech(seconds);
            }
            self.audio.release_idle(self.session.is_enabled());
        }
    }

    /// Plays an accepted reaction as the character voice; how long it runs.
    pub(super) fn play_reaction(&mut self, play: &crate::mascot::reactions::Play) -> Option<f32> {
        use crate::mascot::reactions::Sound;
        let clip = match &play.sound {
            Sound::Builtin(id) => self.audio.bundled(id)?,
            Sound::File(r) => {
                let path = self.current_character().sound_path(r)?;
                crate::audio::decode_wav(&path)
                    .inspect_err(|e| eprintln!("tinypaw: reaction sound {}: {e}", path.display()))
                    .ok()?
            }
        };
        if !self.audio.play_voice(&clip, play.volume) {
            self.notice = Some("No audio output for character sounds right now.".into());
            return None;
        }
        self.own_sound(clip.seconds());
        Some(clip.seconds())
    }

    /// Hiding, switching or removing the character stops its voice and its
    /// pending idle work.
    pub(super) fn stop_character_voice(&mut self) {
        self.voice.stop();
        self.audio.stop_voice();
    }

    /// Plays a line sound as the character voice at Character volume;
    /// returns how long it runs.
    pub(super) fn play_line_sound(
        &mut self,
        sound: &crate::mascot::lines::SoundRef,
    ) -> Option<f32> {
        if self.session.settings().bool(sound::MUTE_ALL) {
            return None;
        }
        let volume = self.session.settings().float(character::VOICE_VOLUME) as f32;
        let path = self.current_character().sound_path(sound)?;
        match crate::audio::decode_wav(&path) {
            Ok(clip) => {
                if !self.audio.play_voice(&clip, volume) {
                    return None;
                }
                self.own_sound(clip.seconds());
                Some(clip.seconds())
            }
            Err(e) => {
                eprintln!("tinypaw: line sound {}: {e}", path.display());
                None
            }
        }
    }

    /// Decodes the selected character's art. A missing or broken pack shows
    /// Yohta instead and says why, without retrying until the choice changes.
    #[cfg(windows)]
    pub(super) fn character_rig(&mut self, id: &str) -> crate::mascot::rig::Rig {
        let Some(c) = self.characters.iter().find(|c| c.id == id) else {
            return crate::mascot::rig::Rig::load();
        };
        c.rig_with_motion().unwrap_or_else(|e| {
            self.notice = Some(format!("Could not show {}: {e}. Showing Yohta.", c.name));
            crate::mascot::rig::Rig::load()
        })
    }

    /// The character's menu: annotation, its own settings and Hide. Commands
    /// go through the registry and are disabled when unavailable; eye
    /// tracking and tap mute live in Settings. A pose choice joins these once
    /// a pack has more than one pose.
    #[cfg(windows)]
    pub(super) fn mascot_menu(&self) -> Vec<crate::mascot::desktop::Item> {
        use crate::mascot::{Msg, desktop::Item};
        use crate::view::icons::Glyph;
        let command = |id: &'static str, icon, label: &'static str, checked: Option<bool>| Item {
            icon,
            label,
            checked,
            on_press: self
                .registry
                .command(id)
                .filter(|d| self.registry.availability(d.id, &self.session).is_ok())
                .map(|_| Msg::Command(id)),
        };
        vec![
            command(
                system::MASTER_TOGGLE,
                Glyph::Pen,
                "Annotation",
                Some(self.session.is_enabled()),
            ),
            Item {
                icon: Glyph::Gear,
                label: "Settings",
                checked: None,
                on_press: Some(Msg::CharacterSettings),
            },
            command(character::HIDE, Glyph::EyeOff, "Hide", None),
        ]
    }
}
