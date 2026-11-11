use super::*;

impl App {
    /// The character's lines: the user's if they changed them, else the pack's.
    pub(super) fn character_lines(&self, id: &str) -> crate::mascot::lines::Lines {
        self.characters
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.lines())
            .unwrap_or_default()
    }

    /// Once: Size used to scale the whole canvas, so its number meant
    /// something different per character. It now means visible height.
    /// The active character keeps its exact on-screen size.
    pub(super) fn migrate_character_height(&mut self, first_run: bool) {
        let s = self.session.settings();
        if s.bool(character::HEIGHT_MIGRATED) {
            return;
        }
        let mut sets = vec![(character::HEIGHT_MIGRATED, Value::Bool(true))];
        if !first_run {
            let old = s.float(character::SIZE);
            let ratio = self
                .current_character()
                .rig()
                .map(|r| f64::from(r.visible_height() / r.size().height))
                .unwrap_or(1.0);
            sets.push((
                character::HEIGHT,
                Value::Float((old * ratio).clamp(character::MIN_HEIGHT, character::MAX_HEIGHT)),
            ));
        }
        let mut values = self.session.settings().clone();
        for (id, value) in sets {
            let _ = self.registry.set(&mut values, id, value);
        }
        self.session.handle(Event::SettingsChanged(values), 0);
        self.preference_writer.schedule(self.session.settings());
    }

    /// Whether the character draft (artwork or behavior) has changes Save
    /// would write.
    pub(super) fn character_unsaved(&self) -> bool {
        self.motion_preview.as_ref().is_some_and(|p| p.unsaved())
            || self
                .art
                .as_ref()
                .is_some_and(|e| e.new_install || e.can_undo())
    }

    /// Writes the behavior draft to the character's overrides in one file,
    /// then shows it on the desktop. Nothing to write is a success.
    pub(super) fn save_behavior(&mut self) -> Result<(), String> {
        let Some(p) = &mut self.motion_preview else {
            return Ok(());
        };
        if !p.unsaved() {
            return Ok(());
        }
        let id = p.id;
        let o = p.overrides(characters::load_overrides(id));
        characters::save_overrides(id, &o)?;
        p.mark_all_saved();
        characters::prune_line_sounds(id, &p.lines, Some(&p.reactions));
        let saved_sounds = p.sounds();
        let compress = p.visual(crate::mascot::reactions::Event::Click)
            == crate::mascot::reactions::Visual::Compress;
        let fall = p.falls();
        self.notice = None;
        #[cfg(windows)]
        if self.mascot.as_ref().is_some_and(|m| m.character() == id) {
            self.reaction_sounds = saved_sounds;
            let lines = p.lines.clone();
            let rig = self.character_rig(id);
            let options = self.mascot_options();
            let pictures = self
                .characters
                .iter()
                .find(|c| c.id == id)
                .map(character_pictures)
                .unwrap_or_default();
            if let Some(m) = &mut self.mascot {
                m.set_rig(id, rig);
                m.set_lines(lines, &options);
                m.set_click_compress(compress);
                m.set_pictures(pictures);
                m.set_fall(fall);
            }
        }
        Ok(())
    }

    /// Cancel / Discard: drops the artwork edit and every unsaved behavior
    /// change. The installed character is untouched.
    pub(super) fn discard_character(&mut self) {
        self.cancel_character_job();
        self.art = None;
        self.art_saving = None;
        if let Some(p) = &mut self.motion_preview {
            p.discard();
            // Sounds the discarded draft imported are no longer needed.
            characters::prune_line_sounds(p.id, p.saved_lines(), Some(p.saved_reactions()));
        }
    }

    pub(super) fn pick_character(&mut self, id: &'static str, out: &mut Outcomes) {
        self.cancel_character_job();
        self.prepared_character = None;
        self.character_conflict = None;
        self.character_draft = None;
        self.art = None;
        out.sets.push((character::CHARACTER, Value::Choice(id)));
    }

    /// Goes where the leave prompt was holding.
    pub(super) fn finish_leave(
        &mut self,
        to: view::settings::Leave,
        out: &mut Outcomes,
    ) -> Task<Message> {
        use view::settings::Leave;
        match to {
            Leave::Page(page) => self.go_page(page),
            Leave::Character(id) => self.pick_character(id, out),
            Leave::Close => return self.close_settings(),
        }
        Task::none()
    }

    /// The selected character, or bundled Yohta if it's gone.
    pub(super) fn current_character(&self) -> &Character {
        let id = self.session.settings().choice(character::CHARACTER);
        self.characters
            .iter()
            .find(|c| c.id == id)
            .unwrap_or(&self.characters[0])
    }

    /// Adds or replaces a character in the list, keeping Yohta first and the
    /// rest by name.
    pub(super) fn add_character(&mut self, c: Character) {
        self.characters.retain(|x| x.id != c.id);
        self.characters.push(c);
        self.characters[1..].sort_by_key(|c| c.name.to_lowercase());
    }

    pub(super) fn cancel_character_job(&mut self) {
        self.character_request += 1;
        if let Some(job) = self.character_job.take() {
            let _ = job.compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst);
        }
    }

    pub(super) fn begin_character_job(&mut self) -> u64 {
        self.cancel_character_job();
        self.character_job = Some(Arc::new(AtomicU8::new(0)));
        self.character_jobs.insert(
            self.character_request,
            self.character_job.as_ref().unwrap().clone(),
        );
        self.notice = Some("Preparing character…".into());
        self.character_request
    }

    pub(super) fn import_character(&mut self, choice: characters::OnDuplicate) -> Task<Message> {
        let Some(prepared) = self.prepared_character.clone() else {
            return Task::none();
        };
        let id = self.begin_character_job();
        let job = self.character_job.as_ref().unwrap().clone();
        let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
        character_work(id, move || {
            prepared
                .install(&taken, choice, &job)
                .map(|r| CharacterWork::Imported(r, false))
        })
    }

    /// Results of file pickers and character background work.
    pub(super) fn picked(
        &mut self,
        message: Message,
        out: &mut Outcomes,
        tasks: &mut Vec<Task<Message>>,
    ) {
        match message {
            Message::PressedPicked(path) => {
                if let (Some(path), Some(draft)) = (path, self.pointer_draft.as_mut()) {
                    self.notice = packs::set_pressed(draft, &path).err();
                }
            }
            Message::PackTapPicked(path) => {
                if let (Some(path), Some(draft)) = (path, self.pointer_draft.as_mut()) {
                    match audio::decode_wav(&path) {
                        Ok(clip) => {
                            draft.tap = Some((clip, path));
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
            Message::PackPicked(path) => {
                if let Some(path) = path {
                    let taken: Vec<_> = self.packs.iter().map(|p| p.id).collect();
                    match packs::import_folder(&path, &taken) {
                        Ok(pack) => {
                            let id = pack.id;
                            self.packs.push(pack);
                            out.sets.push((point::PACK, Value::Choice(id)));
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
            Message::ForearmPicked(path) => {
                if let Some(path) = path {
                    match packs::forearm_from_file(&path) {
                        Ok(arm) => {
                            if let Some(draft) = &mut self.pointer_draft {
                                draft.forearm = Some(arm);
                            }
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(format!("Could not load the forearm: {e}")),
                    }
                }
            }
            Message::CharacterPicked(Some(path)) => {
                self.character_draft = None;
                self.prepared_character = None;
                self.character_conflict = None;
                self.page = Page::Character;
                let id = self.begin_character_job();
                tasks.push(character_work(id, move || {
                    characters::draft_from_png(&path).map(CharacterWork::Draft)
                }));
            }
            Message::CharacterFolderPicked(Some(path)) => {
                self.character_draft = None;
                self.prepared_character = None;
                self.character_conflict = None;
                self.page = Page::Character;
                let id = self.begin_character_job();
                let request = self.reference.clone();
                tasks.push(character_work(id, move || {
                    if let Some(request) = request {
                        request.validate_folder(&path)?;
                    }
                    characters::prepare_folder(&path).map(CharacterWork::Prepared)
                }));
            }
            Message::CharacterWorked(id, result) => {
                self.character_jobs.remove(&id);
                if id != self.character_request
                    && let Err(e) = &result
                    && e != "Import cancelled"
                {
                    self.notice = Some(format!("Background character import failed: {e}"));
                }
                // A completed commit always updates the catalog, even after cancellation.
                if let Ok(CharacterWork::Imported(characters::Imported::Installed(c), show)) =
                    &result
                {
                    if !self.session.settings().bool(character::ENABLED)
                        && (!show || id != self.character_request)
                    {
                        c.release_prepared_rig();
                    }
                    self.add_character(c.clone());
                    #[cfg(windows)]
                    if self.mascot.as_ref().is_some_and(|m| m.character() == c.id) {
                        let rig = self.character_rig(c.id);
                        if let Some(m) = &mut self.mascot {
                            m.set_rig(c.id, rig);
                        }
                    }
                }
                if id == self.character_request {
                    self.character_job = None;
                    self.notice = None;
                    match result {
                        Ok(CharacterWork::Draft(draft)) => self.character_draft = Some(draft),
                        Ok(CharacterWork::Editor(editor, errors)) => {
                            use view::settings::CharacterTab;
                            self.art = Some(*editor);
                            // An opened editor always shows in Arrange.
                            if !matches!(
                                self.character_tab,
                                CharacterTab::Artwork | CharacterTab::Advanced
                            ) {
                                self.character_tab = CharacterTab::Artwork;
                            }
                            if !errors.is_empty() {
                                self.notice = Some(errors.join("; "));
                            }
                        }
                        Ok(CharacterWork::Prepared(prepared)) => {
                            self.reference = None;
                            self.reference_thumb = None;
                            self.prepared_character = Some(prepared);
                            tasks.push(self.import_character(characters::OnDuplicate::Ask));
                        }
                        Ok(CharacterWork::Imported(
                            characters::Imported::Duplicate { name },
                            _,
                        )) => {
                            self.character_conflict = Some(name);
                        }
                        Ok(CharacterWork::Imported(characters::Imported::Installed(c), show)) => {
                            self.character_conflict = None;
                            self.prepared_character = None;
                            self.character_draft = None;
                            self.art = None;
                            // One Save, two writes: the pack is committed, so
                            // now the behavior draft. A failure keeps it open.
                            if self.art_saving.take() == Some(c.id) {
                                match self.save_behavior() {
                                    Ok(()) => {
                                        // Rebuilt from the saved pack and overrides.
                                        if self
                                            .motion_preview
                                            .as_ref()
                                            .is_some_and(|p| p.id == c.id)
                                        {
                                            self.motion_preview = None;
                                        }
                                        if let Some(to) = self.after_save.take() {
                                            tasks.push(self.finish_leave(to, out));
                                        }
                                    }
                                    Err(e) => {
                                        self.after_save = None;
                                        self.notice = Some(format!(
                                            "The artwork is saved, but the behavior changes could not be: {e}. They are still open; try Save again."
                                        ));
                                    }
                                }
                            }
                            out.sets.push((character::CHARACTER, Value::Choice(c.id)));
                            if show {
                                out.sets.push((character::ENABLED, Value::Bool(true)));
                            }
                        }
                        Err(e) => {
                            self.art_saving = None;
                            self.after_save = None;
                            self.notice = Some(format!("Could not import that character: {e}"))
                        }
                    }
                }
            }
            Message::ReactionWavPicked(e, Some(path)) => {
                if let Some(p) = &mut self.motion_preview {
                    match characters::import_reaction_sound(p.id, &path) {
                        Ok(r) => {
                            use crate::mascot::reactions::{MAX_FILES, Sound};
                            // The limit counts what would play, pack and user.
                            if p.files_if(e, &r) > MAX_FILES {
                                self.notice = Some(format!(
                                    "A character can use at most {MAX_FILES} sound files; clear one first."
                                ));
                            } else {
                                p.set_reaction_sound(e, Some(Sound::File(r)));
                                self.notice = None;
                            }
                        }
                        // The row keeps its previous, valid sound.
                        Err(err) => self.notice = Some(format!("Could not use that sound: {err}")),
                    }
                }
            }
            Message::ReactionWavPicked(_, None) => {}
            Message::ReactionPicturePicked(e, Some(path)) => {
                use crate::mascot::reactions::{Art, Event};
                if let Some(p) = &mut self.motion_preview {
                    let canvas = p.rig().size();
                    let canvas = (canvas.width as u32, canvas.height as u32);
                    match characters::import_reaction_image(p.id, &path, canvas) {
                        Ok(file) => {
                            let art = Art::new(file.clone());
                            let c = self.characters.iter().find(|c| c.id == p.id);
                            if let Some(img) = c.and_then(|c| c.reaction_image(&art)) {
                                p.add_picture(file, &img);
                            }
                            if e == Event::Pickup {
                                p.set_held(Some(art));
                                p.test(crate::mascot::preview::Test::DragRight);
                            } else {
                                p.set_art(e, Some(art));
                            }
                            self.notice = None;
                        }
                        Err(err) => {
                            self.notice = Some(format!("Could not use that picture: {err}"))
                        }
                    }
                }
            }
            Message::ReactionPicturePicked(_, None) => {}
            Message::ReferencePicked(Some(path)) => {
                // A thumbnail only; the reference stays where it is.
                self.reference_thumb = ::image::open(&path).ok().map(|img| {
                    let img = img.thumbnail(240, 240).to_rgba8();
                    image::Handle::from_rgba(img.width(), img.height(), img.into_raw())
                });
                self.reference = Some(crate::mascot::reference::Request::new(path));
                self.character_draft = None;
                self.character_tab = view::settings::CharacterTab::Library;
            }
            Message::ReferencePicked(None) => {}
            Message::ReferenceResults(paths) => {
                if let Some(r) = &mut self.reference {
                    r.add_results(paths);
                }
            }
            Message::DraftRig(request, rig) => {
                if request == self.draft_rig_request
                    && let Some(e) = &self.art
                    && let Some(p) = &mut self.motion_preview
                    && p.id == e.id
                {
                    p.set_rig(*rig);
                }
            }
            Message::CharacterExportPicked(Some(into)) => {
                if !self.character_jobs.is_empty() {
                    self.notice = Some(
                        "Character work is still finishing. Try Export when it completes.".into(),
                    );
                } else {
                    let c = self.current_character().clone();
                    self.notice = Some(match characters::export(&c, &into) {
                        Ok(folder) => format!("Exported {} to {}", c.name, folder.display()),
                        Err(e) => format!("Could not export {}: {e}", c.name),
                    });
                }
            }
            Message::ArtBodyPicked(generation, Some(path))
                if generation == self.character_request =>
            {
                let id = self.begin_character_job();
                let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
                tasks.push(character_work(id, move || {
                    crate::mascot::editor::Editor::from_body(&path, &taken)
                        .map(|e| CharacterWork::Editor(Box::new(e), Vec::new()))
                }));
            }
            Message::ArtFilesPicked(generation, target, paths)
                if generation == self.character_request =>
            {
                if !paths.is_empty()
                    && let Some(mut editor) = self.art.clone()
                {
                    let id = self.begin_character_job();
                    tasks.push(character_work(id, move || {
                        let errors = editor.set_images(target, paths);
                        Ok(CharacterWork::Editor(Box::new(editor), errors))
                    }));
                }
            }
            Message::ArtBodyPicked(_, _) | Message::ArtFilesPicked(_, _, _) => {}
            Message::LineSoundPicked(t, i, Some(path)) => {
                if let Some(p) = &mut self.motion_preview {
                    match characters::import_line_sound(p.id, &path) {
                        Ok(sound) => {
                            if let Some(line) = p.lines.get_mut(t).get_mut(i) {
                                line.sound = Some(sound);
                            }
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(format!("Could not use that sound: {e}")),
                    }
                }
            }
            Message::LineSoundPicked(_, _, None) => {}
            Message::MotionTick => {
                let now_i = Instant::now();
                let dt = self
                    .motion_ticked
                    .map_or(0.0, |t| now_i.duration_since(t).as_secs_f32());
                self.motion_ticked = Some(now_i);
                let o = self.motion_options();
                let music = self.music_level();
                let beat = self.beat_phase();
                if let Some(p) = &mut self.motion_preview {
                    p.tick(dt, o.0, o.1, music, beat);
                }
            }
            Message::CharacterPicked(None)
            | Message::CharacterFolderPicked(None)
            | Message::CharacterExportPicked(None) => {}
            Message::PointerPicked(path) => {
                if let Some(path) = path {
                    match packs::draft_from_file(&path) {
                        Ok(draft) => {
                            self.pointer_draft = Some(draft);
                            self.editing_pack = None;
                            self.page = Page::Pointer;
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(format!("Could not use that image: {e}")),
                    }
                }
            }
            Message::WavPicked(path) => {
                if let Some(path) = path {
                    match audio::import_wav(&path) {
                        Ok(clip) => {
                            self.audio.set_custom(clip);
                            self.notice = None;
                            out.sets.push((sound::TAP, Value::Choice("custom")));
                        }
                        Err(e) => self.notice = Some(format!("Could not use that WAV: {e}")),
                    }
                }
            }
            _ => {}
        }
    }

    /// The Settings character studio: Art, Motion, Lines, Reactions and
    /// Create from reference.
    pub(super) fn character_ui(
        &mut self,
        ui: UiMsg,
        now: Millis,
        out: &mut Outcomes,
    ) -> Task<Message> {
        match ui {
            UiMsg::CharacterTab(tab) => {
                use view::settings::CharacterTab;
                // Links from other pages land on the Character tab directly.
                self.go_page(Page::Character);
                self.character_tab = tab;
                // Test character shows the unsaved artwork: build it on a
                // worker by revision, keeping the last preview meanwhile.
                if tab == CharacterTab::Behavior
                    && let Some(e) = self.art.clone()
                    && self.motion_preview.as_ref().is_some_and(|p| p.id == e.id)
                    && e.can_undo()
                {
                    self.draft_rig_request += 1;
                    let request = self.draft_rig_request;
                    let (tx, rx) = iced::futures::channel::oneshot::channel();
                    std::thread::spawn(move || {
                        let _ = tx.send(e.draft_rig());
                    });
                    return Task::perform(rx, move |r| match r {
                        Ok(rig) => Message::DraftRig(request, Box::new(rig)),
                        Err(_) => Message::DraftRig(0, Box::new(crate::mascot::rig::Rig::load())),
                    });
                }
                // Artwork and Advanced open the editor for an installed
                // character; built-in Yohta offers Duplicate to edit instead.
                if matches!(tab, CharacterTab::Artwork | CharacterTab::Advanced)
                    && self.art.is_none()
                    && self.character_job.is_none()
                    && self.character_draft.is_none()
                    && !self.current_character().is_builtin()
                {
                    return self.ui_message(UiMsg::EditArt, now, out);
                }
            }
            UiMsg::Nav(page) => {
                if page != Page::Character && self.character_unsaved() {
                    self.leaving = Some(view::settings::Leave::Page(page));
                } else {
                    self.go_page(page);
                }
            }
            UiMsg::PickCharacter(id) => {
                if id != self.current_character().id && self.character_unsaved() {
                    self.leaving = Some(view::settings::Leave::Character(id));
                } else {
                    self.pick_character(id, out);
                }
            }
            UiMsg::Leave(choice) => {
                use view::settings::LeaveChoice;
                let Some(to) = self.leaving.take() else {
                    return Task::none();
                };
                match choice {
                    LeaveChoice::Stay => {}
                    LeaveChoice::Discard => {
                        self.discard_character();
                        return self.finish_leave(to, out);
                    }
                    LeaveChoice::Save if self.art.is_some() => {
                        self.after_save = Some(to);
                        return self.ui_message(UiMsg::ArtSave, now, out);
                    }
                    LeaveChoice::Save => match self.save_behavior() {
                        Ok(()) => return self.finish_leave(to, out),
                        Err(e) => self.notice = Some(format!("Could not save: {e}")),
                    },
                }
            }
            UiMsg::CharacterSave => {
                if self.art.is_some() {
                    return self.ui_message(UiMsg::ArtSave, now, out);
                }
                if let Err(e) = self.save_behavior() {
                    self.notice = Some(format!("Could not save: {e}"));
                }
            }
            UiMsg::CharacterDiscard => self.discard_character(),
            UiMsg::AddCharacter => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a transparent PNG of your character")
                        .add_filter("PNG image", &["png"])
                        .pick_file(),
                    |file| Message::CharacterPicked(file.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ImportCharacter => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Import a character folder")
                        .pick_folder(),
                    |f| Message::CharacterFolderPicked(f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ExportCharacter => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose where to save the character folder")
                        .pick_folder(),
                    |f| Message::CharacterExportPicked(f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::RemoveCharacter => {
                if !self.character_jobs.is_empty() {
                    self.notice = Some(
                        "Character work is still finishing. Try Remove when it completes.".into(),
                    );
                    return Task::none();
                }
                let c = self.current_character().clone();
                match characters::remove(&c) {
                    Ok(()) => {
                        self.characters.retain(|x| x.id != c.id);
                        out.sets
                            .push((character::CHARACTER, Value::Choice(characters::BUILTIN)));
                        self.notice = None;
                    }
                    Err(e) => self.notice = Some(e),
                }
            }
            UiMsg::CharacterName(name) => {
                if let Some(d) = &mut self.character_draft {
                    d.name = name.chars().take(60).collect();
                }
            }
            UiMsg::CancelCharacter => {
                self.cancel_character_job();
                self.character_draft = None;
            }
            UiMsg::SaveCharacter => {
                if self.character_job.is_none()
                    && let Some(draft) = self.character_draft.clone()
                {
                    let id = self.begin_character_job();
                    let job = self.character_job.as_ref().unwrap().clone();
                    let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
                    return character_work(id, move || {
                        characters::install(&draft, &taken, &job).map(|c| {
                            CharacterWork::Imported(characters::Imported::Installed(c), true)
                        })
                    });
                }
            }
            UiMsg::ResolveCharacter(keep_both) => {
                if self.character_conflict.take().is_some() {
                    return self.import_character(if keep_both {
                        characters::OnDuplicate::KeepBoth
                    } else {
                        characters::OnDuplicate::Replace
                    });
                }
            }
            UiMsg::MotionPreset(i, preset) => {
                if let Some(p) = &mut self.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { preset, ..spec });
                }
            }
            UiMsg::MotionStrength(i, strength) => {
                if let Some(p) = &mut self.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { strength, ..spec });
                }
            }
            UiMsg::MotionSpeed(i, speed) => {
                if let Some(p) = &mut self.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { speed, ..spec });
                }
            }
            UiMsg::MotionTest(test) => {
                if let Some(p) = &mut self.motion_preview {
                    p.test(test);
                }
                if let Some(sound) = self.motion_preview.as_mut().and_then(|p| p.take_sound())
                    && let Some(seconds) = self.play_line_sound(&sound)
                    && let Some(p) = &mut self.motion_preview
                {
                    p.hold_speech(seconds);
                }
            }
            UiMsg::MusicBop(bop) => {
                if let Some(p) = &mut self.motion_preview {
                    p.set_bop(bop);
                    // Show it straight away, even without music.
                    p.test(crate::mascot::preview::Test::Music);
                }
            }
            UiMsg::LineText(t, i, text) => {
                if let Some(line) = self
                    .motion_preview
                    .as_mut()
                    .and_then(|p| p.lines.get_mut(t).get_mut(i))
                {
                    line.text = text.chars().take(crate::mascot::lines::MAX_CHARS).collect();
                }
            }
            UiMsg::LineAdd(t) => {
                if let Some(p) = &mut self.motion_preview
                    && p.lines.total() < crate::mascot::lines::MAX_LINES
                {
                    p.lines.get_mut(t).push(crate::mascot::lines::Line {
                        text: String::new(),
                        sound: None,
                    });
                }
            }
            UiMsg::LineRemove(t, i) => {
                if let Some(p) = &mut self.motion_preview
                    && i < p.lines.get(t).len()
                {
                    p.lines.get_mut(t).remove(i);
                }
            }
            UiMsg::LineClearSound(t, i) => {
                if let Some(line) = self
                    .motion_preview
                    .as_mut()
                    .and_then(|p| p.lines.get_mut(t).get_mut(i))
                {
                    line.sound = None;
                }
            }
            UiMsg::LineSound(t, i) => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a WAV for this line")
                        .add_filter("WAV sound", &["wav"])
                        .pick_file(),
                    move |f| Message::LineSoundPicked(t, i, f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::BubbleBackground(c) => {
                if let Some(p) = &mut self.motion_preview {
                    p.lines.bubble.background = c;
                }
            }
            UiMsg::BubbleText(c) => {
                if let Some(p) = &mut self.motion_preview {
                    p.lines.bubble.text = c;
                }
            }
            UiMsg::BubbleWidth(w) => {
                if let Some(p) = &mut self.motion_preview {
                    p.lines.bubble.max_width = w.clamp(
                        crate::mascot::lines::MIN_WIDTH,
                        crate::mascot::lines::MAX_WIDTH,
                    );
                }
            }
            UiMsg::LinesReset => {
                if let Some(p) = &mut self.motion_preview {
                    p.reset_lines();
                }
            }
            UiMsg::MotionReset => {
                if let Some(p) = &mut self.motion_preview {
                    p.reset();
                }
            }
            UiMsg::CancelCharacterImport => {
                self.cancel_character_job();
                self.character_conflict = None;
                self.prepared_character = None;
            }
            UiMsg::EditArt => {
                let mut c = self.current_character().clone();
                // Bundled Yohta stays built in: edit an installed copy of him.
                if c.is_builtin() {
                    let taken: Vec<&str> = self.characters.iter().map(|c| c.id).collect();
                    match characters::copy_builtin(&taken) {
                        Ok(copy) => {
                            out.sets
                                .push((character::CHARACTER, Value::Choice(copy.id)));
                            self.add_character(copy.clone());
                            c = copy;
                        }
                        Err(e) => {
                            self.notice = Some(format!("Could not copy Yohta to edit: {e}"));
                            return Task::none();
                        }
                    }
                }
                let id = self.begin_character_job();
                self.character_draft = None;
                return character_work(id, move || {
                    crate::mascot::editor::Editor::open(&c)
                        .map(|e| CharacterWork::Editor(Box::new(e), Vec::new()))
                });
            }
            UiMsg::ArtFromParts => {
                self.cancel_character_job();
                let generation = self.character_request;
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose the body PNG, then add its separate parts")
                        .add_filter("PNG image", &["png"])
                        .pick_file(),
                    move |f| Message::ArtBodyPicked(generation, f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ArtPick(target) => {
                use crate::mascot::editor::{EyeSlot, FACE_LABELS, Target};
                let title = match target {
                    Target::Body => "Choose the body PNG".to_owned(),
                    Target::NewPart => {
                        "Choose a part PNG: head, tail, ear, wing or hair".to_owned()
                    }
                    Target::EyePart => "Choose complete eye or eyebrow PNGs".to_owned(),
                    Target::Held => "Choose the held pose PNG".to_owned(),
                    Target::Face(_, slot) => {
                        format!(
                            "Choose the {} face PNG",
                            FACE_LABELS[slot.min(2)].to_lowercase()
                        )
                    }
                    Target::Eye(_, slot) => format!(
                        "Choose the eye's {} PNG",
                        match slot {
                            EyeSlot::White => "background",
                            EyeSlot::Pupil => "moving pupil",
                            EyeSlot::Mask => "pupil area (mask)",
                            EyeSlot::Blink => "blink",
                        }
                    ),
                };
                let generation = self.character_request;
                return Task::perform(
                    async move {
                        let picker = rfd::AsyncFileDialog::new()
                            .set_title(title)
                            .add_filter("PNG image", &["png"]);
                        if matches!(target, Target::NewPart | Target::EyePart) {
                            picker
                                .pick_files()
                                .await
                                .unwrap_or_default()
                                .into_iter()
                                .map(|f| f.path().to_path_buf())
                                .collect()
                        } else {
                            picker
                                .pick_file()
                                .await
                                .into_iter()
                                .map(|f| f.path().to_path_buf())
                                .collect()
                        }
                    },
                    move |paths| Message::ArtFilesPicked(generation, target, paths),
                );
            }
            UiMsg::ArtSelect(s) => {
                if let Some(e) = &mut self.art {
                    e.select(s);
                }
            }
            UiMsg::ArtKind(i, kind) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.set_kind(i, kind);
                }
            }
            UiMsg::ArtOnHead(i, on) => {
                if let Some(p) = self.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.on_head = on;
                }
            }
            UiMsg::ArtBehind(i, behind) => {
                if let Some(p) = self.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.behind = behind;
                }
            }
            UiMsg::ArtRemovePart(i) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.remove_part(i);
                }
            }
            UiMsg::ArtRemoveFace(i, slot) => {
                if let Some(p) = self.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.faces[slot.min(2)] = None;
                }
            }
            UiMsg::ArtStaticEye(i) => {
                if let Some(e) = &mut self.art {
                    self.notice = e.keep_eye_static(i).err();
                }
            }
            UiMsg::ArtAddEye => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.add_eye();
                }
            }
            UiMsg::ArtRemoveEye(i) => {
                if let Some(e) = &mut self.art
                    && i < e.eyes.len()
                {
                    e.eyes.remove(i);
                }
            }
            UiMsg::ArtRemoveBlink(i) => {
                if let Some(eye) = self.art.as_mut().and_then(|e| e.eyes.get_mut(i)) {
                    eye.blink = None;
                }
            }
            UiMsg::ArtTravel(i, travel) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint_gesture(crate::mascot::editor::Gesture::Travel(i));
                }
                if let Some(eye) = self.art.as_mut().and_then(|e| e.eyes.get_mut(i)) {
                    eye.travel = travel.clamp(0.0, 0.05);
                }
            }
            UiMsg::ArtPupilSize(i, scale) => {
                if let Some(e) = &mut self.art {
                    self.notice = e.set_pupil_size(i, scale).err();
                }
            }
            UiMsg::ArtClick(x, y) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.click(x, y);
                }
            }
            UiMsg::ArtName(name) => {
                if let Some(e) = &mut self.art {
                    e.name = name.chars().take(60).collect();
                }
            }
            UiMsg::ArtCancel => self.discard_character(),
            UiMsg::ArtSave => {
                if self.character_job.is_none()
                    && let Some(editor) = self.art.clone()
                {
                    self.art_saving = Some(editor.id);
                    let id = self.begin_character_job();
                    let job = self.character_job.as_ref().unwrap().clone();
                    return character_work(id, move || {
                        editor.save(&job).map(|c| {
                            CharacterWork::Imported(
                                characters::Imported::Installed(c),
                                editor.new_install,
                            )
                        })
                    });
                }
            }
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
            UiMsg::ReferenceStart => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a picture of the character to use as a reference")
                        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                        .pick_file(),
                    |f| Message::ReferencePicked(f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ReferenceStyle(style) => {
                if let Some(r) = &mut self.reference {
                    r.style = style.chars().take(200).collect();
                }
            }
            UiMsg::ReferenceCanvas(c) => {
                if let Some(r) = &mut self.reference {
                    r.canvas = c;
                }
            }
            UiMsg::ReferenceTail(on) => {
                if let Some(r) = &mut self.reference {
                    r.tail = on;
                }
            }
            UiMsg::ReferenceHead(on) => {
                if let Some(r) = &mut self.reference {
                    r.head = on;
                }
            }
            UiMsg::ReferenceEyes(on) => {
                if let Some(r) = &mut self.reference {
                    r.eyes = on;
                    if on {
                        r.tracking = false;
                        r.blink = false;
                        r.faces = false;
                    }
                }
            }
            UiMsg::ReferenceTracking(on) => {
                if let Some(r) = &mut self.reference {
                    r.tracking = on;
                    if on {
                        r.eyes = false;
                    }
                }
            }
            UiMsg::ReferenceBrows(on) => {
                if let Some(r) = &mut self.reference {
                    r.brows = on;
                }
            }
            UiMsg::ReferenceBlink(on) => {
                if let Some(r) = &mut self.reference {
                    r.blink = on;
                    if on {
                        r.eyes = false;
                    }
                }
            }
            UiMsg::ReferenceFaces(on) => {
                if let Some(r) = &mut self.reference {
                    r.faces = on;
                    if on {
                        r.eyes = false;
                    }
                }
            }
            UiMsg::ReferenceHeld(on) => {
                if let Some(r) = &mut self.reference {
                    r.held = on;
                }
            }
            UiMsg::ReferenceCopy(role) => {
                if let Some(r) = &self.reference {
                    if !r.fits_canvas() {
                        self.notice =
                            Some("Choose a smaller canvas for this many feature images.".into());
                        return Task::none();
                    }
                    let text = match role {
                        Some(role) => r.prompt(role),
                        None => r.brief(),
                    };
                    self.notice = Some(
                        "Copied. Attach the reference picture in your image tool and paste.".into(),
                    );
                    return iced::clipboard::write(text);
                }
            }
            UiMsg::ReferenceImport => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose the PNGs your image tool made")
                        .add_filter("PNG image", &["png"])
                        .pick_files(),
                    |files| {
                        Message::ReferenceResults(
                            files
                                .unwrap_or_default()
                                .into_iter()
                                .map(|f| f.path().to_path_buf())
                                .collect(),
                        )
                    },
                );
            }
            UiMsg::ReferenceAssign(i, role) => {
                if let Some(r) = &mut self.reference {
                    r.assign(i, role);
                }
            }
            UiMsg::ReferenceCancel => {
                self.reference = None;
                self.reference_thumb = None;
            }
            UiMsg::ReferenceOpen => {
                use crate::mascot::editor::{Editor, Target};
                use crate::mascot::reference::Role;
                let Some(r) = self.reference.clone() else {
                    return Task::none();
                };
                if !r.missing().is_empty() || !r.fits_canvas() {
                    self.notice = Some(
                        "Import every requested feature image and choose a supported canvas first."
                            .into(),
                    );
                    return Task::none();
                }
                let Some(body) = r.file_for(Role::Body).map(PathBuf::from) else {
                    self.notice = Some("Assign a file to Body first.".into());
                    return Task::none();
                };
                let head = r.file_for(Role::Head).map(PathBuf::from);
                let tail = r.file_for(Role::Tail).map(PathBuf::from);
                let held = r.file_for(Role::Held).map(PathBuf::from);
                let head_faces: Vec<(usize, PathBuf)> = r
                    .roles()
                    .into_iter()
                    .filter_map(|role| Some((role.face_slot()?, PathBuf::from(r.file_for(role)?))))
                    .collect();
                let faces: Vec<(Role, PathBuf)> = r
                    .roles()
                    .into_iter()
                    .filter(|role| role.is_face())
                    .filter_map(|role| r.file_for(role).map(|p| (role, PathBuf::from(p))))
                    .collect();
                let tracking: Vec<_> = r
                    .roles()
                    .into_iter()
                    .filter_map(|role| {
                        if let Role::Tracking(right, layer) = role {
                            Some((right, layer, r.file_for(role)?.to_owned()))
                        } else {
                            None
                        }
                    })
                    .collect();
                self.reference = None;
                self.reference_thumb = None;
                let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
                let id = self.begin_character_job();
                // The assembled draft opens in Arrange for alignment; Save
                // exports through the ordinary validated pack path.
                return character_work(id, move || {
                    let mut editor = Editor::from_body(&body, &taken)?;
                    let mut errors = Vec::new();
                    // The head first, so eyes and eyebrows ride on it.
                    if let Some(head) = head {
                        let before = editor.parts.len();
                        errors.extend(editor.set_images(Target::NewPart, vec![head]));
                        if editor.parts.len() == before {
                            return Err(errors.join("; "));
                        }
                        if let Some(p) = editor.parts.last_mut() {
                            p.kind = crate::mascot::motion::PartType::Head;
                            p.motion = crate::mascot::motion::MotionSpec::default_for(p.kind);
                            p.name = "Head".into();
                        }
                        // Its faces: eyes closed, pressed, being carried.
                        let index = editor.parts.len() - 1;
                        for (slot, path) in head_faces {
                            if let Err(e) = editor.set_image(Target::Face(index, slot), &path) {
                                errors.push(format!("{}: {e}", path.display()));
                            }
                        }
                    }
                    for (role, path) in faces {
                        let before = editor.parts.len();
                        errors.extend(editor.set_images(Target::EyePart, vec![path]));
                        if editor.parts.len() > before
                            && let Some(p) = editor.parts.last_mut()
                        {
                            p.name = role.label().into();
                        }
                    }
                    if !tracking.is_empty() {
                        use crate::mascot::editor::EyeSlot;
                        use crate::mascot::reference::EyeLayer;
                        editor.add_eye();
                        editor.add_eye();
                        for (right, layer, path) in tracking {
                            let slot = match layer {
                                EyeLayer::White => EyeSlot::White,
                                EyeLayer::Pupil => EyeSlot::Pupil,
                                EyeLayer::Mask => EyeSlot::Mask,
                                EyeLayer::Blink => EyeSlot::Blink,
                            };
                            if let Err(e) =
                                editor.set_image(Target::Eye(usize::from(right), slot), &path)
                            {
                                errors.push(format!("{}: {e}", path.display()));
                            }
                        }
                    }
                    if let Some(tail) = tail {
                        errors.extend(editor.set_images(Target::NewPart, vec![tail]));
                        if let Some(p) = editor.parts.last_mut() {
                            p.kind = crate::mascot::motion::PartType::Tail;
                            p.name = "Tail".into();
                        }
                    }
                    if let Some(held) = held
                        && let Err(e) = editor.set_image(Target::Held, &held)
                    {
                        errors.push(format!("held pose: {e}"));
                    }
                    Ok(CharacterWork::Editor(Box::new(editor), errors))
                });
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
            UiMsg::ArtJoint(part) => {
                if let Some(e) = &mut self.art {
                    match part {
                        Some(i) => e.adjust_joint(i),
                        None => e.finish_joint(),
                    }
                }
            }
            UiMsg::ArtRename(i, name) => {
                if let Some(e) = &mut self.art {
                    e.rename(i, name);
                }
            }
            UiMsg::ArtEyeStyle(style) => {
                if let Some(e) = &mut self.art {
                    e.eye_style = Some(style);
                }
            }
            UiMsg::ArtRest(action) => {
                if let Some(e) = &mut self.art {
                    use crate::mascot::editor::RestAction;
                    match action {
                        RestAction::Adjust => e.adjust_rest(),
                        RestAction::Done => e.finish_rest(),
                        RestAction::Reset => e.reset_rest(),
                    }
                }
            }
            UiMsg::ArtField(i, text) => {
                if let Some(e) = &mut self.art {
                    e.field(i, text);
                }
            }
            UiMsg::ArtCanvas(i, text) => {
                if let Some(e) = &mut self.art {
                    e.resize_canvas(i, text);
                }
            }
            UiMsg::ArtDragStart(selected) => {
                if let Some(e) = &mut self.art {
                    e.select(selected);
                    e.checkpoint();
                }
            }
            UiMsg::ArtDrag(dx, dy) => {
                if let Some(e) = &mut self.art {
                    e.translate(dx, dy);
                }
            }
            UiMsg::ArtNudge(dx, dy) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint_gesture(crate::mascot::editor::Gesture::Nudge);
                    e.nudge(dx, dy);
                }
            }
            UiMsg::ArtOrder(i, up) => {
                if let Some(e) = &mut self.art {
                    e.reorder(i, up);
                }
            }
            UiMsg::ArtUndo => {
                if let Some(e) = &mut self.art {
                    e.undo();
                }
            }
            UiMsg::ArtRedo => {
                if let Some(e) = &mut self.art {
                    e.redo();
                }
            }
            UiMsg::ArtZoom(zoom) => {
                if let Some(e) = &mut self.art {
                    e.zoom = zoom.clamp(0.5, 3.0);
                }
            }
            UiMsg::ArtBackground(bg) => {
                if let Some(e) = &mut self.art {
                    e.background = bg;
                }
            }
            _ => {}
        }
        Task::none()
    }
}
