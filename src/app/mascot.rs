use super::*;

/// The app's side of one desktop character: the stage thread's handle and
/// what was last sent to it, so only changes are sent.
#[cfg(windows)]
pub(super) struct MascotStage {
    /// The instance this stage shows.
    pub id: u32,
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
    signals: Option<crate::stage::Signals>,
    menu: Vec<crate::mascot::desktop::Item>,
    audio: (crate::mascot::companionship::Audio, bool),
}

impl App {
    /// The stage of an instance, if it is on the desktop.
    #[cfg(windows)]
    pub(super) fn mascot_of(&self, id: u32) -> Option<&MascotStage> {
        self.mascots.iter().find(|m| m.id == id)
    }

    /// Whether any instance of this character is on the desktop.
    #[cfg(windows)]
    pub(super) fn character_on_desktop(&self, character: &str) -> bool {
        self.mascots.iter().any(|m| m.character == character)
    }

    /// Sends every stage showing this character its art and lines again, after
    /// either changed. `edit` adjusts what is sent.
    #[cfg(windows)]
    pub(super) fn refresh_character_stages(
        &mut self,
        id: &'static str,
        edit: impl Fn(&mut crate::stage::Character),
    ) {
        let ids: Vec<u32> = self
            .mascots
            .iter()
            .filter(|m| m.character == id)
            .map(|m| m.id)
            .collect();
        for uid in ids {
            let mut character = self.character_setup(id);
            edit(&mut character);
            if let Some(m) = self.mascot_of(uid) {
                m.stage
                    .send(crate::stage::Intent::SetCharacter(Box::new(character)));
            }
        }
    }

    /// The behaviour settings of one instance: its own eyes and idle, and
    /// the app-wide lines, reduced motion and music.
    pub(super) fn mascot_options(
        &self,
        instance: &crate::instances::Instance,
    ) -> crate::mascot::desktop::Options {
        let s = self.session.settings();
        crate::mascot::desktop::Options {
            bop: self
                .studio
                .motion_preview
                .as_ref()
                .filter(|p| p.id == instance.character)
                .map(|p| p.rig().bop()),
            eyes: instance.eyes,
            idle: instance.idle,
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

    /// A menu row chosen on a stage.
    #[cfg_attr(not(windows), allow(unused_variables))]
    pub(super) fn mascot_message(
        &mut self,
        instance: u32,
        msg: crate::mascot::Msg,
        now: Millis,
        out: &mut Outcomes,
        tasks: &mut Vec<Task<Message>>,
    ) {
        use crate::mascot::Msg;
        match msg {
            // Hide on a character's menu hides that character, not all of them.
            Msg::Command(character::HIDE) => {
                // The last one showing turns the master off instead, as Hide
                // always did, so the tray's Show characters brings it back.
                let others = self
                    .instances
                    .list()
                    .iter()
                    .any(|i| i.shown && i.id != instance);
                if !others {
                    out.sets.push((character::ENABLED, Value::Bool(false)));
                } else if let Some(i) = self.instances.get_mut(instance) {
                    i.shown = false;
                    self.save_instances();
                    tasks.push(self.reconcile_mascot());
                }
            }
            // Other menu entries go through the registry like every other surface.
            Msg::Command(id) => out.collect(self.dispatch(id, now).ok()),
            Msg::CharacterSettings => tasks.push(self.open_settings()),
            _ => {}
        }
    }

    /// What a stage reported: gestures and line sounds to voice, a spot to
    /// save, a menu row, or a change in how busy the character is.
    #[cfg(windows)]
    pub(super) fn character_event(
        &mut self,
        instance: u32,
        event: crate::stage::Event,
        now: Millis,
        out: &mut Outcomes,
        tasks: &mut Vec<Task<Message>>,
    ) {
        use crate::stage::Event;
        match event {
            Event::Ready => {}
            Event::Voice { gestures, line } => self.play_mascot_sound(instance, gestures, line),
            Event::Placed(x, y) => {
                self.instances.place(instance, x, y);
                self.save_instances();
            }
            Event::Menu(msg) => self.mascot_message(instance, msg, now, out, tasks),
            Event::Status { free, animating } => {
                if let Some(m) = self.mascots.iter_mut().find(|m| m.id == instance) {
                    m.free = free;
                    m.animating = animating;
                }
            }
        }
    }

    /// Saves the instance list; a failure is shown, not fatal.
    pub(super) fn save_instances(&mut self) {
        if let Err(e) = self.instances.save() {
            self.notice = Some(format!("Could not save the characters: {e}"));
        }
    }

    /// The live music inputs the stages sample for themselves.
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

    /// Tells every stage what is playing and whether a voice line is busy.
    #[cfg(windows)]
    pub(super) fn sync_mascot_audio(&mut self) {
        let audio = (
            self.companion_audio(),
            self.voice.busy(self.voice_clock.elapsed().as_secs_f64()),
        );
        for m in &mut self.mascots {
            if m.audio != audio {
                m.audio = audio;
                m.stage.send(crate::stage::Intent::Audio {
                    audio: audio.0,
                    voice_busy: audio.1,
                });
            }
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
            name: c.map_or_else(|| id.to_owned(), |c| c.name.clone()),
            rig,
            lines,
            click_compress,
            pictures,
            fall,
        }
    }

    /// Starts, stops, resizes and hides the desktop characters to match the
    /// instance list and the master switch. They stay while annotation is Off
    /// and step aside while the annotation surface owns input. Only what
    /// changed is sent.
    pub(super) fn reconcile_mascot(&mut self) -> Task<Message> {
        #[cfg(windows)]
        {
            use crate::stage::Intent;
            let master = self.session.settings().bool(character::ENABLED);
            let hidden = self.session.overlay_state() == OverlayState::Interactive;
            // Who should be on the desktop, in list order.
            let wanted: Vec<crate::instances::Instance> = if master {
                self.instances
                    .list()
                    .iter()
                    .filter(|i| i.shown)
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            // Instances that were hidden or removed: close their stages.
            let before = self.mascots.len();
            self.mascots.retain(|m| wanted.iter().any(|i| i.id == m.id));
            if self.mascots.len() != before && self.mascots.is_empty() {
                self.stop_character_voice();
            }
            // The menu is as tall as its visible rows.
            let menu = self.mascot_menu();
            let rows = menu.len();
            let signals = self.mascot_signals();
            for instance in &wanted {
                let size = instance.height as f32;
                let options = self.mascot_options(instance);
                let selected = self.character_id(&instance.character);
                let current = self.mascots.iter().position(|m| m.id == instance.id);
                let stale = current.is_some_and(|n| self.mascots[n].character != selected);
                if current.is_none() || stale {
                    let character = self.character_setup(selected);
                    self.load_reaction_sounds(selected);
                    let sleep = character.pictures.settings.zzz;
                    match current {
                        Some(n) => {
                            let m = &mut self.mascots[n];
                            m.stage.send(Intent::SetCharacter(Box::new(character)));
                            m.character = selected;
                            m.sleep = sleep;
                        }
                        None => {
                            let uid = instance.id;
                            let stage =
                                crate::stage::Stage::spawn(uid, self.crowd.clone(), move |event| {
                                    platform::send(External::Character(uid, event))
                                });
                            stage.send(Intent::Open {
                                character: Box::new(character),
                                size,
                                anchor: (instance.x, instance.y),
                                rows,
                                hidden,
                                options,
                            });
                            stage.send(Intent::Menu(menu.clone()));
                            self.mascots.push(MascotStage {
                                id: uid,
                                stage,
                                character: selected,
                                size,
                                rows,
                                hidden,
                                sleep,
                                free: false,
                                animating: false,
                                options,
                                signals: None,
                                menu: menu.clone(),
                                audio: (crate::mascot::companionship::Audio::Unknown, false),
                            });
                        }
                    }
                }
                let Some(m) = self.mascots.iter_mut().find(|m| m.id == instance.id) else {
                    continue;
                };
                if m.signals.as_ref() != Some(&signals) {
                    m.signals = Some(signals.clone());
                    m.stage.send(Intent::Signals(signals.clone()));
                }
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
                    m.stage.send(Intent::Menu(menu.clone()));
                }
                if hidden != m.hidden {
                    m.hidden = hidden;
                    m.stage.send(Intent::Hidden(hidden));
                    if !hidden {
                        // Back from annotation: each character can say so; the
                        // stages take turns.
                        m.stage
                            .send(Intent::Say(crate::mascot::lines::Trigger::AnnotationEnd));
                    }
                }
            }
            if hidden && !self.mascots.is_empty() {
                // Annotation owns input: character audio stops.
                self.voice.stop();
                self.audio.stop_voice();
            }
            self.sync_mascot_audio();
        }
        Task::none()
    }

    /// The installed character with this ID, or Yohta when it is gone.
    pub(super) fn character_id(&self, id: &str) -> &'static str {
        self.characters
            .iter()
            .find(|c| c.id == id)
            .map_or(characters::BUILTIN, |c| c.id)
    }

    /// Remembers a character's reaction sounds for the one shared voice.
    pub(super) fn load_reaction_sounds(&mut self, id: &'static str) {
        let sounds = self
            .characters
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.reaction_sounds())
            .unwrap_or_default();
        self.reaction_sounds.insert(id, sounds);
    }

    /// Routes a desktop character's gesture events through the one
    /// character voice, then its line sound. A trigger plays one sound:
    /// an accepted reaction sound wins over that trigger's line sound. All
    /// characters share the voice, so only one sound plays at a time.
    pub(super) fn play_mascot_sound(
        &mut self,
        instance: u32,
        gestures: Vec<crate::mascot::reactions::Event>,
        line: Option<crate::mascot::lines::SoundRef>,
    ) {
        #[cfg(windows)]
        {
            use crate::mascot::reactions::{Event, Quiet};
            let now = self.voice_clock.elapsed().as_secs_f64();
            let Some(m) = self.mascot_of(instance) else {
                return;
            };
            let mut events = gestures;
            let (free, hidden, character) = (m.free, m.hidden, m.character);
            let sounds = self
                .reaction_sounds
                .get(character)
                .cloned()
                .unwrap_or_default();
            let pick = || (self.voice_clock.elapsed().subsec_nanos() % 1000) as f32 / 1000.0;
            if self.voice.idle_due(&sounds, free, now, pick)
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
                if let Ok(play) = self.voice.accept(e, &sounds, quiet, now)
                    && let Some(seconds) = self.play_reaction(&play, character)
                {
                    self.voice.started(now, seconds);
                    reacted = true;
                }
            }
            if !reacted
                && let Some(sound) = line
                && let Some(seconds) = self.play_line_sound(&sound, character)
                && let Some(m) = self.mascot_of(instance)
            {
                m.stage.send(crate::stage::Intent::HoldSpeech(seconds));
            }
            self.audio.release_idle(self.session.is_enabled());
        }
    }

    /// Plays an accepted reaction as the character voice; how long it runs.
    pub(super) fn play_reaction(
        &mut self,
        play: &crate::mascot::reactions::Play,
        character: &str,
    ) -> Option<f32> {
        use crate::mascot::reactions::Sound;
        let clip = match &play.sound {
            Sound::Builtin(id) => self.audio.bundled(id)?,
            Sound::File(r) => {
                let path = self.character_named(character).sound_path(r)?;
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
        character: &str,
    ) -> Option<f32> {
        if self.session.settings().bool(sound::MUTE_ALL) {
            return None;
        }
        let volume = self.session.settings().float(character::VOICE_VOLUME) as f32;
        let path = self.character_named(character).sound_path(sound)?;
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
