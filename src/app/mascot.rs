use super::*;

/// The app's side of the desktop character: the stage thread's handle and
/// what was last sent to it, so only changes are sent.
#[cfg(windows)]
pub(super) struct MascotStage {
    pub stage: crate::stage::Stage,
    pub character: &'static str,
    size: f32,
    rows: usize,
    /// Annotation owns input, so the character is hidden.
    pub hidden: bool,
    /// The character has sleep artwork, which keeps audio listening.
    pub sleep: bool,
    /// Visible and not busy: an idle sound may play.
    pub free: bool,
    /// Moving fast enough that timers need to be fine.
    pub animating: bool,
    options: crate::mascot::desktop::Options,
    menu: Vec<crate::mascot::desktop::Item>,
    audio: (crate::mascot::companionship::Audio, bool),
}

impl App {
    pub(super) fn mascot_options(&self) -> crate::mascot::desktop::Options {
        let s = self.session.settings();
        crate::mascot::desktop::Options {
            bop: self
                .studio
                .motion_preview
                .as_ref()
                .filter(|p| p.id == s.choice(character::CHARACTER))
                .map(|p| p.rig().bop()),
            eyes: s.bool(character::EYES),
            idle: s.bool(character::IDLE),
            reduced: system::reduced(s),
            lines: s.bool(character::LINES),
            quiet: s.bool(character::QUIET_LINES),
            idle_every: (
                s.float(character::IDLE_MIN) as f32 * 60.0,
                s.float(character::IDLE_MAX) as f32 * 60.0,
            ),
            // The stage samples these from `Signals` on its own clock.
            music: 0.0,
            energy: None,
            beat: None,
            dance: self.dance_options(),
        }
    }

    /// A menu row chosen on the stage.
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
            Msg::Command(id) => out.collect(self.dispatch(id, now).ok()),
            Msg::CharacterSettings => {
                self.page = Page::Character;
                tasks.push(self.open_settings());
            }
            _ => {}
        }
    }

    /// What the stage reported: gestures and line sounds to voice, a spot to
    /// save, a menu row, or a change in how busy the character is.
    #[cfg(windows)]
    pub(super) fn character_event(
        &mut self,
        event: crate::stage::Event,
        now: Millis,
        out: &mut Outcomes,
        tasks: &mut Vec<Task<Message>>,
    ) {
        use crate::stage::Event;
        match event {
            Event::Ready => {}
            Event::Voice { gestures, line } => self.play_mascot_sound(gestures, line),
            Event::Placed(x, y) => {
                out.sets.push((character::X, Value::Float(x)));
                out.sets.push((character::Y, Value::Float(y)));
            }
            Event::Menu(msg) => self.mascot_message(msg, now, out, tasks),
            Event::Status { free, animating } => {
                if let Some(m) = &mut self.mascot {
                    m.free = free;
                    m.animating = animating;
                }
            }
        }
    }

    /// The live music inputs the stage samples for itself.
    #[cfg(windows)]
    pub(super) fn mascot_signals(&self) -> crate::stage::Signals {
        let s = self.session.settings();
        crate::stage::Signals {
            meter: self.music.as_ref().map(Arc::downgrade).unwrap_or_default(),
            follower: self.beat.as_ref().map(Arc::downgrade).unwrap_or_default(),
            clock: self.beat_clock.clone(),
            music_on: s.bool(character::MUSIC),
            strength: s.float(character::MUSIC_STRENGTH) as f32,
            beat_offset: match s.choice(character::BEAT_TIMING) {
                "earlier" => 0.06,
                "later" => -0.06,
                _ => 0.0,
            },
        }
    }

    /// Tells the stage what is playing and whether a voice line is busy.
    #[cfg(windows)]
    pub(super) fn sync_mascot_audio(&mut self) {
        let audio = (
            self.companion_audio(),
            self.voice.busy(self.voice_clock.elapsed().as_secs_f64()),
        );
        if let Some(m) = &mut self.mascot
            && m.audio != audio
        {
            m.audio = audio;
            m.stage.send(crate::stage::Intent::Audio {
                audio: audio.0,
                voice_busy: audio.1,
            });
        }
    }

    /// Everything the stage needs to show a character, from the app's packs.
    #[cfg(windows)]
    pub(super) fn character_setup(&mut self, id: &'static str) -> crate::stage::Character {
        let rig = self.character_rig(id);
        let lines = self.character_lines(id);
        let c = self.characters.iter().find(|c| c.id == id);
        let click_compress = c.is_none_or(|c| {
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
        crate::stage::Character {
            id,
            rig,
            lines,
            click_compress,
            pictures,
            fall,
        }
    }

    /// Starts, stops, resizes and hides the character on the stage to match
    /// its settings. It stays while annotation is Off and steps aside while the
    /// annotation surface owns input. Only what changed is sent.
    pub(super) fn reconcile_mascot(&mut self) -> Task<Message> {
        #[cfg(windows)]
        {
            use crate::stage::Intent;
            let s = self.session.settings();
            let want = s.bool(character::ENABLED);
            let size = s.float(character::HEIGHT) as f32;
            let hidden = self.session.overlay_state() == OverlayState::Interactive;
            let selected = s.choice(character::CHARACTER);
            let anchor = (s.float(character::X), s.float(character::Y));
            if !want {
                if self.mascot.take().is_some() {
                    self.stop_character_voice();
                }
                return Task::none();
            }
            // The menu is as tall as its visible rows.
            let menu = self.mascot_menu();
            let rows = menu.len();
            let options = self.mascot_options();
            let stale = self
                .mascot
                .as_ref()
                .is_some_and(|m| m.character != selected);
            if self.mascot.is_none() || stale {
                self.stop_character_voice();
                let character = self.character_setup(selected);
                self.reaction_sounds = self
                    .characters
                    .iter()
                    .find(|c| c.id == selected)
                    .map(|c| c.reaction_sounds())
                    .unwrap_or_default();
                let sleep = character.pictures.settings.zzz;
                match &mut self.mascot {
                    Some(m) => {
                        m.stage.send(Intent::SetCharacter(Box::new(character)));
                        m.character = selected;
                        m.sleep = sleep;
                    }
                    None => {
                        let stage = crate::stage::Stage::spawn(|event| {
                            platform::send(External::Character(event))
                        });
                        stage.send(Intent::Open {
                            character: Box::new(character),
                            size,
                            anchor,
                            rows,
                            hidden,
                            options,
                        });
                        self.mascot = Some(MascotStage {
                            stage,
                            character: selected,
                            size,
                            rows,
                            hidden,
                            sleep,
                            free: false,
                            animating: false,
                            options,
                            menu: menu.clone(),
                            audio: (crate::mascot::companionship::Audio::Unknown, false),
                        });
                        let items = menu.clone();
                        if let Some(m) = &self.mascot {
                            m.stage.send(Intent::Menu(items));
                        }
                    }
                }
            }
            let signals = self.mascot_signals();
            let Some(m) = &mut self.mascot else {
                return Task::none();
            };
            m.stage.send(Intent::Signals(signals));
            if (size, rows) != (m.size, m.rows) {
                m.size = size;
                m.rows = rows;
                m.stage.send(Intent::Resize { size, rows });
            }
            if options != m.options {
                m.options = options;
                m.stage.send(Intent::Options(options));
            }
            if menu != m.menu {
                m.menu = menu.clone();
                m.stage.send(Intent::Menu(menu));
            }
            let was_hidden = m.hidden;
            if hidden != was_hidden {
                m.hidden = hidden;
                m.stage.send(Intent::Hidden(hidden));
                if hidden {
                    // Annotation owns input: character audio stops.
                    self.voice.stop();
                    self.audio.stop_voice();
                } else {
                    // Back from annotation: the character can say so.
                    m.stage
                        .send(Intent::Say(crate::mascot::lines::Trigger::AnnotationEnd));
                }
            }
            self.sync_mascot_audio();
        }
        Task::none()
    }

    /// Routes the desktop character's gesture events through the one
    /// character voice, then its line sound. A trigger plays one sound:
    /// an accepted reaction sound wins over that trigger's line sound.
    pub(super) fn play_mascot_sound(
        &mut self,
        gestures: Vec<crate::mascot::reactions::Event>,
        line: Option<crate::mascot::lines::SoundRef>,
    ) {
        #[cfg(windows)]
        {
            use crate::mascot::reactions::{Event, Quiet};
            let now = self.voice_clock.elapsed().as_secs_f64();
            let Some(m) = &self.mascot else {
                return;
            };
            let mut events = gestures;
            let (free, hidden) = (m.free, m.hidden);
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
                && let Some(m) = &self.mascot
            {
                m.stage.send(crate::stage::Intent::HoldSpeech(seconds));
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
