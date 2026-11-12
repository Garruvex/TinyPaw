use super::*;

mod art;
mod reactions;
mod reference;

/// State of the Settings character studio.
#[derive(Default)]
pub(super) struct Studio {
    /// "Add character" in progress.
    pub(super) character_draft: Option<characters::Draft>,
    /// A folder import waiting on Keep both or Replace: its display name.
    pub(super) character_conflict: Option<String>,
    pub(super) prepared_character: Option<Arc<characters::PreparedFolder>>,
    pub(super) character_request: u64,
    pub(super) character_job: Option<Arc<AtomicU8>>,
    pub(super) character_jobs: std::collections::BTreeMap<u64, Arc<AtomicU8>>,
    /// The Art tab's open character, while editing.
    pub(super) art: Option<crate::mascot::editor::Editor>,
    /// The Motion tab's live preview, while Settings shows Character.
    pub(super) motion_preview: Option<crate::mascot::preview::MotionPreview>,
    /// Somewhere the user asked to go with unsaved character changes,
    /// waiting on Save, Discard or Stay.
    pub(super) leaving: Option<view::settings::Leave>,
    /// The character whose artwork Save is writing, so its behavior
    /// overrides follow once the pack is committed.
    pub(super) art_saving: Option<&'static str>,
    /// Where to go once that save finishes, from Save in the leave prompt.
    pub(super) after_save: Option<view::settings::Leave>,
    /// A character whose preview failed to load, so it isn't retried
    /// on every update.
    pub(super) motion_failed: Option<&'static str>,
    pub(super) motion_ticked: Option<Instant>,
    /// The beat phase and when it was last sampled, to find its pace.
    pub(super) motion_beat: Option<(f32, Instant)>,
    /// Create from reference in progress, and its reference thumbnail.
    pub(super) reference: Option<crate::mascot::reference::Request>,
    pub(super) reference_thumb: Option<image::Handle>,
    /// The latest draft-rig request; older results are dropped.
    pub(super) draft_rig_request: u64,
    /// The import and export panel, while open.
    pub(super) transfer: Option<view::settings::Transfer>,
}

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
        self.studio
            .motion_preview
            .as_ref()
            .is_some_and(|p| p.unsaved())
            || self
                .studio
                .art
                .as_ref()
                .is_some_and(|e| e.new_install || e.can_undo())
    }

    /// Writes the behavior draft to the character's overrides in one file,
    /// then shows it on the desktop. Nothing to write is a success.
    pub(super) fn save_behavior(&mut self) -> Result<(), String> {
        let Some(p) = &mut self.studio.motion_preview else {
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
        self.studio.art = None;
        self.studio.art_saving = None;
        if let Some(p) = &mut self.studio.motion_preview {
            p.discard();
            // Sounds the discarded draft imported are no longer needed.
            characters::prune_line_sounds(p.id, p.saved_lines(), Some(p.saved_reactions()));
        }
    }

    pub(super) fn pick_character(&mut self, id: &'static str, out: &mut Outcomes) {
        self.cancel_character_job();
        self.studio.prepared_character = None;
        self.studio.character_conflict = None;
        self.studio.character_draft = None;
        self.studio.art = None;
        self.studio.transfer = None;
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
        self.studio.character_request += 1;
        if let Some(job) = self.studio.character_job.take() {
            let _ = job.compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst);
        }
    }

    pub(super) fn begin_character_job(&mut self) -> u64 {
        self.cancel_character_job();
        self.studio.character_job = Some(Arc::new(AtomicU8::new(0)));
        self.studio.character_jobs.insert(
            self.studio.character_request,
            self.studio.character_job.as_ref().unwrap().clone(),
        );
        self.notice = Some("Preparing character…".into());
        self.studio.character_request
    }

    /// The panel's Import: fits and imports a character folder, or brings a
    /// dialogue file's lines into the behavior draft for Save.
    fn confirm_transfer(&mut self) -> Task<Message> {
        use view::settings::Transfer;
        match self.studio.transfer.take() {
            Some(Transfer::Fit { path, choice, .. }) => {
                let id = self.begin_character_job();
                character_work(id, move || {
                    characters::prepare_folder(&path, Some(choice)).map(CharacterWork::Prepared)
                })
            }
            Some(Transfer::Dialogue {
                dialogue,
                merge,
                layout,
            }) => {
                let Some(p) = &mut self.studio.motion_preview else {
                    self.notice = Some(
                        "The character's preview isn't ready yet; try again in a moment.".into(),
                    );
                    return Task::none();
                };
                let (lines, failed) = dialogue.import_sounds(p.id);
                let dropped = p.lines.merge(&lines, merge, layout);
                let mut notes = vec![format!(
                    "Imported {} lines. Save to keep them.",
                    lines.total() - dropped
                )];
                if dropped > 0 {
                    notes.push(format!(
                        "{dropped} didn't fit: a character has at most {} lines.",
                        crate::mascot::lines::MAX_LINES
                    ));
                }
                let missing = failed + dialogue.missing_sounds;
                if missing > 0 {
                    notes.push(format!(
                        "{missing} sounds were missing or unreadable, so those lines have none."
                    ));
                }
                self.notice = Some(notes.join(" "));
                self.character_tab = view::settings::CharacterTab::Behavior;
                Task::none()
            }
            Some(choose @ Transfer::Choose { .. }) => {
                self.studio.transfer = Some(choose);
                Task::none()
            }
            None => Task::none(),
        }
    }

    pub(super) fn import_character(&mut self, choice: characters::OnDuplicate) -> Task<Message> {
        let Some(prepared) = self.studio.prepared_character.clone() else {
            return Task::none();
        };
        let id = self.begin_character_job();
        let job = self.studio.character_job.as_ref().unwrap().clone();
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
            Message::PackPicked(Some(path)) => {
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
            Message::ForearmPicked(Some(path)) => match packs::forearm_from_file(&path) {
                Ok(arm) => {
                    if let Some(draft) = &mut self.pointer_draft {
                        draft.forearm = Some(arm);
                    }
                    self.notice = None;
                }
                Err(e) => self.notice = Some(format!("Could not load the forearm: {e}")),
            },
            Message::CharacterPicked(Some(path)) => {
                self.studio.character_draft = None;
                self.studio.prepared_character = None;
                self.studio.character_conflict = None;
                self.page = Page::Character;
                let id = self.begin_character_job();
                tasks.push(character_work(id, move || {
                    characters::draft_from_png(&path).map(CharacterWork::Draft)
                }));
            }
            Message::CharacterFolderPicked(Some(path)) => {
                self.studio.character_draft = None;
                self.studio.prepared_character = None;
                self.studio.character_conflict = None;
                self.page = Page::Character;
                let id = self.begin_character_job();
                let request = self.studio.reference.clone();
                tasks.push(character_work(id, move || {
                    if let Some(request) = request {
                        request.validate_folder(&path)?;
                    }
                    // Images that don't match the canvas: ask how to fit them.
                    let found = characters::inspect_folder(&path)?;
                    if !found.misfits.is_empty() {
                        return Ok(CharacterWork::Misfits(path, found));
                    }
                    characters::prepare_folder(&path, None).map(CharacterWork::Prepared)
                }));
            }
            Message::DialoguePicked(Some(path)) => match characters::read_dialogue(&path) {
                Ok(dialogue) => {
                    self.notice = None;
                    self.studio.transfer = Some(view::settings::Transfer::Dialogue {
                        dialogue,
                        merge: crate::mascot::lines::Merge::Replace,
                        layout: true,
                    });
                }
                Err(e) => self.notice = Some(format!("Could not read that dialogue file: {e}")),
            },
            Message::DialogueExportPicked(template, Some(into)) => {
                let c = self.current_character().clone();
                self.notice = Some(if template {
                    match characters::write_template(&into) {
                        Ok(file) => format!(
                            "Wrote a dialogue template to {}. Fill it in, then Import… → Dialogue.",
                            file.display()
                        ),
                        Err(e) => format!("Could not write the template: {e}"),
                    }
                } else {
                    match characters::export_dialogue(&c, &c.lines(), &into) {
                        Ok(folder) => format!(
                            "Exported {}'s dialogue to {}",
                            c.name,
                            folder.join(characters::DIALOGUE).display()
                        ),
                        Err(e) => format!("Could not export {}'s dialogue: {e}", c.name),
                    }
                });
            }
            Message::DialoguePicked(None) | Message::DialogueExportPicked(_, None) => {}
            Message::CharacterWorked(id, result) => {
                self.studio.character_jobs.remove(&id);
                if id != self.studio.character_request
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
                        && (!show || id != self.studio.character_request)
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
                if id == self.studio.character_request {
                    self.studio.character_job = None;
                    self.notice = None;
                    match result {
                        Ok(CharacterWork::Draft(draft)) => {
                            self.studio.character_draft = Some(draft)
                        }
                        Ok(CharacterWork::Editor(editor, errors)) => {
                            use view::settings::CharacterTab;
                            self.studio.art = Some(*editor);
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
                        Ok(CharacterWork::Misfits(path, found)) => {
                            let choice = found.suggested.unwrap_or(characters::Fit::Bottom);
                            self.studio.transfer = Some(view::settings::Transfer::Fit {
                                path,
                                found,
                                choice,
                            });
                        }
                        Ok(CharacterWork::Prepared(prepared)) => {
                            self.studio.reference = None;
                            self.studio.reference_thumb = None;
                            self.studio.prepared_character = Some(prepared);
                            tasks.push(self.import_character(characters::OnDuplicate::Ask));
                        }
                        Ok(CharacterWork::Imported(
                            characters::Imported::Duplicate { name },
                            _,
                        )) => {
                            self.studio.character_conflict = Some(name);
                        }
                        Ok(CharacterWork::Imported(characters::Imported::Installed(c), show)) => {
                            self.studio.character_conflict = None;
                            self.studio.prepared_character = None;
                            self.studio.character_draft = None;
                            self.studio.art = None;
                            // One Save, two writes: the pack is committed, so
                            // now the behavior draft. A failure keeps it open.
                            if self.studio.art_saving.take() == Some(c.id) {
                                match self.save_behavior() {
                                    Ok(()) => {
                                        // Rebuilt from the saved pack and overrides.
                                        if self
                                            .studio
                                            .motion_preview
                                            .as_ref()
                                            .is_some_and(|p| p.id == c.id)
                                        {
                                            self.studio.motion_preview = None;
                                        }
                                        if let Some(to) = self.studio.after_save.take() {
                                            tasks.push(self.finish_leave(to, out));
                                        }
                                    }
                                    Err(e) => {
                                        self.studio.after_save = None;
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
                            self.studio.art_saving = None;
                            self.studio.after_save = None;
                            self.notice = Some(format!("Could not import that character: {e}"))
                        }
                    }
                }
            }
            Message::ReactionWavPicked(e, Some(path)) => {
                if let Some(p) = &mut self.studio.motion_preview {
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
                if let Some(p) = &mut self.studio.motion_preview {
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
                self.studio.reference_thumb = ::image::open(&path).ok().map(|img| {
                    let img = img.thumbnail(240, 240).to_rgba8();
                    image::Handle::from_rgba(img.width(), img.height(), img.into_raw())
                });
                self.studio.reference = Some(crate::mascot::reference::Request::new(path));
                self.studio.character_draft = None;
                self.character_tab = view::settings::CharacterTab::Library;
            }
            Message::ReferencePicked(None) => {}
            Message::ReferenceResults(paths) => {
                if let Some(r) = &mut self.studio.reference {
                    r.add_results(paths);
                }
            }
            Message::DraftRig(request, rig) => {
                if request == self.studio.draft_rig_request
                    && let Some(e) = &self.studio.art
                    && let Some(p) = &mut self.studio.motion_preview
                    && p.id == e.id
                {
                    p.set_rig(*rig);
                }
            }
            Message::CharacterExportPicked(Some(into)) => {
                if !self.studio.character_jobs.is_empty() {
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
                if generation == self.studio.character_request =>
            {
                let id = self.begin_character_job();
                let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
                tasks.push(character_work(id, move || {
                    crate::mascot::editor::Editor::from_body(&path, &taken)
                        .map(|e| CharacterWork::Editor(Box::new(e), Vec::new()))
                }));
            }
            Message::ArtFilesPicked(generation, target, paths)
                if generation == self.studio.character_request =>
            {
                if !paths.is_empty()
                    && let Some(mut editor) = self.studio.art.clone()
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
                if let Some(p) = &mut self.studio.motion_preview {
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
            // The preview animates itself, one canvas redraw at a time (see
            // `MotionStage::update`), so frames never rebuild this page. This
            // only refreshes what it can't see: the settings it follows and
            // the music playing. It also lands when the speech bubble
            // appears or goes away, which the page does show.
            Message::MotionTick => {
                let o = self.motion_options();
                let beat = self.beat_phase();
                let sampled = Instant::now();
                // Cycles per second, from the last two samples.
                let beat_rate = match (beat, self.studio.motion_beat) {
                    (Some(now), Some((before, at))) => {
                        let dt = sampled.duration_since(at).as_secs_f32().max(0.001);
                        (((now - before + 0.5).rem_euclid(1.0) - 0.5) / dt).clamp(0.0, 6.0)
                    }
                    _ => 0.0,
                };
                self.studio.motion_beat = beat.map(|b| (b, sampled));
                let control = crate::mascot::preview::Control {
                    idle: o.0,
                    reduced: o.1,
                    music: self.music_level(),
                    beat,
                    beat_rate,
                    sampled: Some(sampled),
                    period: crate::features::mascot::frame_period(
                        self.session
                            .settings()
                            .choice(crate::features::mascot::FRAME_RATE),
                    ),
                };
                if let Some(p) = &self.studio.motion_preview {
                    p.set_control(control);
                }
            }
            Message::CharacterPicked(None)
            | Message::CharacterFolderPicked(None)
            | Message::CharacterExportPicked(None) => {}
            Message::PointerPicked(Some(path)) => match packs::draft_from_file(&path) {
                Ok(draft) => {
                    self.pointer_draft = Some(draft);
                    self.editing_pack = None;
                    self.page = Page::Pointer;
                    self.notice = None;
                }
                Err(e) => self.notice = Some(format!("Could not use that image: {e}")),
            },
            Message::WavPicked(Some(path)) => match audio::import_wav(&path) {
                Ok(clip) => {
                    self.audio.set_custom(clip);
                    self.notice = None;
                    out.sets.push((sound::TAP, Value::Choice("custom")));
                }
                Err(e) => self.notice = Some(format!("Could not use that WAV: {e}")),
            },
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
                    && let Some(e) = self.studio.art.clone()
                    && self
                        .studio
                        .motion_preview
                        .as_ref()
                        .is_some_and(|p| p.id == e.id)
                    && e.can_undo()
                {
                    self.studio.draft_rig_request += 1;
                    let request = self.studio.draft_rig_request;
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
                    && self.studio.art.is_none()
                    && self.studio.character_job.is_none()
                    && self.studio.character_draft.is_none()
                    && !self.current_character().is_builtin()
                {
                    return self.ui_message(UiMsg::EditArt, now, out);
                }
            }
            UiMsg::Nav(page) => {
                if page != Page::Character && self.character_unsaved() {
                    self.studio.leaving = Some(view::settings::Leave::Page(page));
                } else {
                    self.go_page(page);
                }
            }
            UiMsg::PickCharacter(id) => {
                if id != self.current_character().id && self.character_unsaved() {
                    self.studio.leaving = Some(view::settings::Leave::Character(id));
                } else {
                    self.pick_character(id, out);
                }
            }
            UiMsg::Leave(choice) => {
                use view::settings::LeaveChoice;
                let Some(to) = self.studio.leaving.take() else {
                    return Task::none();
                };
                match choice {
                    LeaveChoice::Stay => {}
                    LeaveChoice::Discard => {
                        self.discard_character();
                        return self.finish_leave(to, out);
                    }
                    LeaveChoice::Save if self.studio.art.is_some() => {
                        self.studio.after_save = Some(to);
                        return self.ui_message(UiMsg::ArtSave, now, out);
                    }
                    LeaveChoice::Save => match self.save_behavior() {
                        Ok(()) => return self.finish_leave(to, out),
                        Err(e) => self.notice = Some(format!("Could not save: {e}")),
                    },
                }
            }
            UiMsg::CharacterSave => {
                if self.studio.art.is_some() {
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
                if !self.studio.character_jobs.is_empty() {
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
                if let Some(d) = &mut self.studio.character_draft {
                    d.name = name.chars().take(60).collect();
                }
            }
            UiMsg::CancelCharacter => {
                self.cancel_character_job();
                self.studio.character_draft = None;
            }
            UiMsg::SaveCharacter => {
                if self.studio.character_job.is_none()
                    && let Some(draft) = self.studio.character_draft.clone()
                {
                    let id = self.begin_character_job();
                    let job = self.studio.character_job.as_ref().unwrap().clone();
                    let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
                    return character_work(id, move || {
                        characters::install(&draft, &taken, &job).map(|c| {
                            CharacterWork::Imported(characters::Imported::Installed(c), true)
                        })
                    });
                }
            }
            UiMsg::ResolveCharacter(keep_both) => {
                if self.studio.character_conflict.take().is_some() {
                    return self.import_character(if keep_both {
                        characters::OnDuplicate::KeepBoth
                    } else {
                        characters::OnDuplicate::Replace
                    });
                }
            }
            UiMsg::MotionPreset(i, preset) => {
                if let Some(p) = &mut self.studio.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { preset, ..spec });
                }
            }
            UiMsg::MotionStrength(i, strength) => {
                if let Some(p) = &mut self.studio.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { strength, ..spec });
                }
            }
            UiMsg::MotionSpeed(i, speed) => {
                if let Some(p) = &mut self.studio.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { speed, ..spec });
                }
            }
            UiMsg::MotionTest(test) => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.test(test);
                }
                if let Some(sound) = self
                    .studio
                    .motion_preview
                    .as_mut()
                    .and_then(|p| p.take_sound())
                    && let Some(seconds) = self.play_line_sound(&sound)
                    && let Some(p) = &mut self.studio.motion_preview
                {
                    p.hold_speech(seconds);
                }
            }
            UiMsg::MusicBop(bop) => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.set_bop(bop);
                    // Show it straight away, even without music.
                    p.test(crate::mascot::preview::Test::Music);
                }
            }
            UiMsg::LineText(t, i, text) => {
                if let Some(line) = self
                    .studio
                    .motion_preview
                    .as_mut()
                    .and_then(|p| p.lines.get_mut(t).get_mut(i))
                {
                    line.text = text.chars().take(crate::mascot::lines::MAX_CHARS).collect();
                }
            }
            UiMsg::LineAdd(t) => {
                if let Some(p) = &mut self.studio.motion_preview
                    && p.lines.total() < crate::mascot::lines::MAX_LINES
                {
                    p.lines.get_mut(t).push(crate::mascot::lines::Line {
                        text: String::new(),
                        sound: None,
                    });
                }
            }
            UiMsg::LineRemove(t, i) => {
                if let Some(p) = &mut self.studio.motion_preview
                    && i < p.lines.get(t).len()
                {
                    p.lines.get_mut(t).remove(i);
                }
            }
            UiMsg::LineClearSound(t, i) => {
                if let Some(line) = self
                    .studio
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
                if let Some(p) = &mut self.studio.motion_preview {
                    p.lines.bubble.background = c;
                }
            }
            UiMsg::BubbleText(c) => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.lines.bubble.text = c;
                }
            }
            UiMsg::BubbleWidth(w) => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.lines.bubble.max_width = w.clamp(
                        crate::mascot::lines::MIN_WIDTH,
                        crate::mascot::lines::MAX_WIDTH,
                    );
                }
            }
            UiMsg::BubbleSide(side) => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.lines.bubble.side = side;
                    p.test(crate::mascot::preview::Test::Speak);
                }
            }
            UiMsg::BubbleAnchor(at) => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.lines.bubble.anchor = at.map(|(x, y)| (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
                    p.placing_bubble = false;
                }
            }
            UiMsg::BubblePlace => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.placing_bubble = !p.placing_bubble;
                }
            }
            UiMsg::MenuSide(side) => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.lines.menu.side = side;
                }
            }
            UiMsg::MenuAlign(align) => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.lines.menu.align = align;
                }
            }
            UiMsg::LinesReset => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.reset_lines();
                }
            }
            UiMsg::TransferOpen(export) => {
                self.notice = None;
                self.studio.transfer = Some(view::settings::Transfer::Choose { export });
            }
            UiMsg::TransferCancel => self.studio.transfer = None,
            UiMsg::TransferPick(export, scope) => {
                use view::settings::TransferScope;
                self.studio.transfer = None;
                return match (export, scope) {
                    (false, TransferScope::Character) => {
                        self.ui_message(UiMsg::ImportCharacter, now, out)
                    }
                    (true, TransferScope::Character) => {
                        self.ui_message(UiMsg::ExportCharacter, now, out)
                    }
                    (false, _) => Task::perform(
                        rfd::AsyncFileDialog::new()
                            .set_title("Import dialogue: a lines.json or character.json")
                            .add_filter("Dialogue (JSON)", &["json"])
                            .pick_file(),
                        |f| Message::DialoguePicked(f.map(|f| f.path().to_path_buf())),
                    ),
                    (true, scope) => {
                        let template = scope == TransferScope::Template;
                        Task::perform(
                            rfd::AsyncFileDialog::new()
                                .set_title(if template {
                                    "Choose where to save the dialogue template"
                                } else {
                                    "Choose where to save the dialogue folder"
                                })
                                .pick_folder(),
                            move |f| {
                                Message::DialogueExportPicked(
                                    template,
                                    f.map(|f| f.path().to_path_buf()),
                                )
                            },
                        )
                    }
                };
            }
            UiMsg::TransferFit(fit) => {
                if let Some(view::settings::Transfer::Fit { choice, .. }) =
                    &mut self.studio.transfer
                {
                    *choice = fit;
                }
            }
            UiMsg::TransferMerge(m) => {
                if let Some(view::settings::Transfer::Dialogue { merge, .. }) =
                    &mut self.studio.transfer
                {
                    *merge = m;
                }
            }
            UiMsg::TransferLayout(on) => {
                if let Some(view::settings::Transfer::Dialogue { layout, .. }) =
                    &mut self.studio.transfer
                {
                    *layout = on;
                }
            }
            UiMsg::TransferConfirm => return self.confirm_transfer(),
            UiMsg::MotionReset => {
                if let Some(p) = &mut self.studio.motion_preview {
                    p.reset();
                }
            }
            UiMsg::CancelCharacterImport => {
                self.cancel_character_job();
                self.studio.character_conflict = None;
                self.studio.prepared_character = None;
            }
            other => return self.art_ui(other, now, out),
        }
        Task::none()
    }
}
