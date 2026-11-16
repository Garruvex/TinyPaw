use super::*;

impl App {
    /// The app-wide music pose and hand position: what a character uses
    /// until it is given its own in the studio.
    pub(super) fn default_music_fit(&self) -> characters::MusicFit {
        let s = self.session.settings();
        characters::MusicFit {
            pose: match s.choice(character::MUSIC_POSE) {
                "sit" => 1,
                "stand" => 2,
                _ => 0,
            },
            hand_x: s.float(character::WAVE_X) as f32,
            hand_y: s.float(character::WAVE_Y) as f32,
            hand_angle: s.float(character::WAVE_ANGLE) as f32,
            wave_range: s.float(character::WAVE_RANGE) as f32,
        }
    }

    /// How `id` holds itself to music: its own fit, else the app-wide one.
    pub(super) fn music_fit(&self, id: &str) -> characters::MusicFit {
        let own = *self
            .music_fits
            .borrow_mut()
            .entry(id.to_owned())
            .or_insert_with(|| characters::load_overrides(id).music);
        own.unwrap_or_else(|| self.default_music_fit())
    }

    /// Gives `id` its own music pose and hand position, saved at once. One
    /// equal to the app-wide values is dropped, so the character follows them.
    pub(super) fn set_music_fit(&mut self, id: &str, fit: characters::MusicFit) {
        let own = (fit != self.default_music_fit()).then_some(fit);
        let mut overrides = characters::load_overrides(id);
        overrides.music = own;
        match characters::save_overrides(id, &overrides) {
            Ok(()) => {
                self.music_fits.borrow_mut().insert(id.to_owned(), own);
            }
            Err(e) => self.notice = Some(format!("Couldn't save the music pose: {e}")),
        }
    }

    /// The dance choices for the character `id`: its own pose and hand, and
    /// the app-wide rest.
    pub(super) fn dance_options(&self, id: &str) -> crate::mascot::motion::Dance {
        let s = self.session.settings();
        let fit = self.music_fit(id);
        crate::mascot::motion::Dance {
            pose: fit.pose,
            stand_threshold: s.float(character::EXCITEMENT) as f32 / 100.0,
            hand_offset: iced::Vector::new(fit.hand_x / 100.0, fit.hand_y / 100.0),
            hand_angle: fit.hand_angle.to_radians(),
            wave_range: fit.wave_range / 100.0,
            every: s.choice(character::DANCE_EVERY).parse().unwrap_or(16),
            included: character::DANCE_STYLES.map(|id| s.bool(id)),
            item: match s.choice(character::DANCE_ITEM) {
                "glow_stick" => 0,
                "phone" => 1,
                _ => 2,
            },
        }
    }

    #[cfg(windows)]
    /// Playback loudness scaled by Music strength, 0 when off or silent.
    pub(super) fn music_level(&self) -> f32 {
        #[cfg(windows)]
        if let Some(m) = &self.music {
            if !self.session.settings().bool(character::MUSIC) {
                return 0.0;
            }
            let strength = self.session.settings().float(character::MUSIC_STRENGTH) as f32;
            return m.level() * strength;
        }
        0.0
    }

    pub(super) fn music_energy(&self) -> Option<f32> {
        #[cfg(windows)]
        if let Some(m) = &self.music {
            return Some(if self.session.settings().bool(character::MUSIC) {
                m.level()
            } else {
                0.0
            });
        }
        Some(0.0)
    }

    pub(super) fn beat_sensitivity(&self) -> crate::mascot::beat::Sensitivity {
        use crate::mascot::beat::Sensitivity;
        match self.session.settings().choice(character::BEAT_SENSITIVITY) {
            "low" => Sensitivity::Low,
            "high" => Sensitivity::High,
            _ => Sensitivity::Normal,
        }
    }

    pub(super) fn beat_engine(&self) -> crate::mascot::beat::Engine {
        crate::mascot::beat::Engine::parse(self.session.settings().choice(character::BEAT_ENGINE))
    }

    /// The followed beat's phase, shifted by Beat timing, while music plays.
    pub(super) fn beat_phase(&self) -> Option<f32> {
        #[cfg(windows)]
        {
            self.mascot_signals().sample().2
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// The Follow beat trial's status line.
    pub(super) fn beat_status(&self) -> String {
        #[cfg(windows)]
        if self.beat.is_none()
            && self.music.is_some()
            && self.session.settings().bool(character::BEAT)
            && let source @ crate::mascot::music::Source::App(_) = self.music_source()
        {
            return format!("Starts when {source} plays");
        }
        #[cfg(windows)]
        if let Some(b) = &self.beat {
            return match (b.available(), b.bpm()) {
                (false, _) => "Can't listen to the output".into(),
                // The eased tempo the character is moving at.
                (true, Some(_)) => format!(
                    "Following · {:.0} BPM",
                    self.beat_clock
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .0
                        .bpm()
                ),
                (true, None) => "Looking for a beat".into(),
            };
        }
        if self.session.settings().bool(character::BEAT) {
            "Starts with music response".into()
        } else {
            "Off".into()
        }
    }

    /// TinyPaw is playing a sound: Follow beat's loopback hears it too, so
    /// onsets are held off meanwhile.
    pub(super) fn own_sound(&self, seconds: f32) {
        // A single app's capture never hears TinyPaw.
        #[cfg(windows)]
        if let Some(b) = &self.beat
            && !matches!(b.target(), platform::beat::Target::Process(_))
        {
            b.hold_off(seconds);
        }
        let _ = seconds;
    }

    /// What the Music section shows: off, starting, silent, playing or
    /// unavailable, with the raw level for the meter.
    pub(super) fn music_status(&self) -> (String, f32) {
        #[cfg(windows)]
        if let Some(m) = &self.music {
            use crate::mascot::music::Source;
            use platform::music::State;
            let source = self.music_source();
            return match (m.state(), &source) {
                (State::Starting, _) => ("Listening…".into(), 0.0),
                (State::Silent, Source::App(exe)) if m.app_pids(exe).is_empty() => {
                    (format!("{source} isn't playing"), 0.0)
                }
                (State::Silent, _) => ("Too quiet to move to".into(), m.heard()),
                (State::Playing, _) => ("Moving to it".into(), m.heard()),
                (State::Unavailable, Source::Input(name)) => (format!("Can't open {name}"), 0.0),
                (State::Unavailable, _) => {
                    ("Unavailable: no playback device to listen to".into(), 0.0)
                }
            };
        }
        if self.session.settings().bool(character::MUSIC) {
            (
                "Starts while the character or this page is showing".into(),
                0.0,
            )
        } else {
            ("Off".into(), 0.0)
        }
    }

    /// What music response listens to.
    pub(super) fn music_source(&self) -> crate::mascot::music::Source {
        crate::mascot::music::Source::parse(self.session.settings().choice(character::MUSIC_SOURCE))
    }

    /// The app sources to pick from: all apps, then each app playing now,
    /// with live meters; the saved app stays listed while it's quiet.
    pub(super) fn music_rows(&self) -> Vec<(crate::mascot::music::Source, f32, bool)> {
        use crate::mascot::music::Source;
        let mut rows = vec![(Source::All, 0.0, true)];
        #[cfg(windows)]
        if let Some(m) = &self.music {
            rows[0].1 = m.all_apps();
            rows.extend(
                m.apps()
                    .into_iter()
                    .map(|a| (Source::App(a.exe), a.shown, true)),
            );
        }
        let current = self.music_source();
        if matches!(current, Source::App(_)) && !rows.iter().any(|(s, _, _)| *s == current) {
            rows.push((current, 0.0, false));
        }
        rows
    }

    /// The input devices, keeping the saved one while it's unplugged.
    pub(super) fn music_inputs(&self) -> Vec<crate::mascot::music::Source> {
        use crate::mascot::music::Source;
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut list: Vec<Source> = Vec::new();
        #[cfg(windows)]
        if let Some(m) = &self.music {
            list.extend(m.inputs().iter().map(|n| Source::input(n)));
        }
        let current = self.music_source();
        if matches!(current, Source::Input(_)) && !list.contains(&current) {
            list.push(current);
        }
        list
    }

    /// Runs the playback meter only while music response is on and the
    /// desktop character is showing, or the Character page is open to show
    /// the meter; otherwise it's stopped and costs nothing. Also passes on
    /// "music started" to the character.
    pub(super) fn companion_audio(&self) -> crate::mascot::companionship::Audio {
        use crate::mascot::companionship::Audio;
        match self.music.as_ref().map(|m| m.state()) {
            Some(platform::music::State::Silent) => Audio::Quiet,
            Some(platform::music::State::Playing) => Audio::Playing,
            _ => Audio::Unknown,
        }
    }

    pub(super) fn sync_music(&mut self) {
        #[cfg(windows)]
        {
            let on = self.session.settings().bool(character::MUSIC);
            let showing = self.mascots.iter().any(|m| !m.hidden);
            let sleep_audio = self.mascots.iter().any(|m| m.sleep && !m.hidden);
            // The Settings window's Music page shows the meter, too.
            let watched = self.studio.motion_preview.is_some() || self.settings_child.is_some();
            let wanted = (on && (showing || watched)) || sleep_audio;
            use crate::mascot::music::Source;
            use platform::beat::Target;
            let source = if sleep_audio && !on {
                Source::All
            } else {
                self.music_source()
            };
            match (wanted, &self.music) {
                (true, None) => {
                    self.music = Some(Arc::new(platform::music::Meter::start(source.clone())))
                }
                (true, Some(m)) => m.set_source(&source),
                (false, Some(_)) => self.music = None,
                (false, None) => {}
            }
            // Follow beat runs only alongside music response, on the same
            // source: one app's own audio while it's playing.
            let target = match &source {
                Source::All => Some(Target::Output),
                Source::Input(name) => Some(Target::Input(name.clone())),
                Source::App(exe) => {
                    let pids = self
                        .music
                        .as_ref()
                        .map(|m| m.app_pids(exe))
                        .unwrap_or_default();
                    match self.beat.as_ref().map(|b| b.target()) {
                        // Keep a capture still on one of the app's processes.
                        Some(Target::Process(pid)) if pids.contains(pid) => {
                            Some(Target::Process(*pid))
                        }
                        _ => pids.first().map(|pid| Target::Process(*pid)),
                    }
                }
            };
            let follow = on && wanted && self.session.settings().bool(character::BEAT);
            let sensitivity = self.beat_sensitivity();
            let engine = self.beat_engine();
            match (follow.then_some(target).flatten(), &self.beat) {
                // A new engine starts a fresh tracker.
                (Some(t), Some(b)) if *b.target() == t && b.engine() == engine => {
                    b.set_sensitivity(sensitivity)
                }
                (Some(t), _) => {
                    self.beat = Some(Arc::new(platform::beat::Follower::start(
                        sensitivity,
                        engine,
                        t,
                    )))
                }
                (None, Some(_)) => self.beat = None,
                (None, None) => {}
            }
            if on && self.music.as_ref().is_some_and(|m| m.take_started()) {
                // They take turns, so one of them says it.
                for m in &self.mascots {
                    m.stage.send(crate::stage::Intent::Say(
                        crate::mascot::lines::Trigger::MusicStart,
                    ));
                }
            }
        }
    }

    /// Idle motion on, and reduced motion, for the Motion tab preview.
    pub(super) fn motion_options(&self) -> (bool, bool) {
        let s = self.session.settings();
        (s.bool(character::IDLE), system::reduced(s))
    }

    /// Builds the Motion tab's preview while Settings shows the Character
    /// page (not while editing art or adding a character), and drops it
    /// otherwise so its art isn't held in memory.
    pub(super) fn sync_motion_preview(&mut self) {
        // Kept while the artwork editor is open: one draft spans both.
        let wanted = self.settings_window.is_some()
            && self.page == Page::Character
            && self.studio.character_draft.is_none();
        if !wanted {
            self.studio.motion_preview = None;
            self.studio.motion_ticked = None;
            return;
        }
        let c = self.current_character().clone();
        if self
            .studio
            .motion_preview
            .as_ref()
            .is_some_and(|p| p.id == c.id)
            || self.studio.motion_failed == Some(c.id)
        {
            return;
        }
        match c.rig() {
            Ok(base) => {
                let defaults = base.motion().to_vec();
                let mut rig = base;
                if let Some(specs) = characters::load_motion(c.id) {
                    rig.set_motion(&specs);
                }
                rig.set_bop(c.bop());
                self.studio.motion_preview = Some(crate::mascot::preview::MotionPreview::new(
                    c.id,
                    rig,
                    defaults,
                    c.lines(),
                    c.pack_lines(),
                    c.pack_bop(),
                    characters::load_overrides(c.id)
                        .reactions
                        .unwrap_or_default(),
                    c.pack_reactions(),
                ));
                if let Some(p) = &mut self.studio.motion_preview {
                    for art in p.picture_art() {
                        if let Some(img) = c.reaction_image(&art) {
                            p.add_art_picture(art, &img);
                        }
                    }
                }
                self.studio.motion_failed = None;
                self.studio.motion_ticked = None;
            }
            Err(e) => {
                self.studio.motion_preview = None;
                self.studio.motion_failed = Some(c.id);
                self.notice = Some(format!("Could not preview {}: {e}", c.name));
            }
        }
    }
}
