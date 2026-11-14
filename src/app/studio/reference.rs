use super::*;
use crate::mascot::reference::Role;

impl App {
    /// Create from reference.
    pub(super) fn reference_ui(
        &mut self,
        ui: UiMsg,
        now: Millis,
        out: &mut Outcomes,
    ) -> Task<Message> {
        match ui {
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
                if let Some(r) = &mut self.studio.reference {
                    r.style = style.chars().take(200).collect();
                }
            }
            UiMsg::ReferenceCanvas(c) => {
                if let Some(r) = &mut self.studio.reference {
                    r.canvas = c;
                }
            }
            UiMsg::ReferenceTail(on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.tail = on;
                }
            }
            UiMsg::ReferenceDance(on) => {
                if let Some(r) = &mut self.studio.reference { r.dance = on; }
            }
            UiMsg::ReferenceHead(on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.head = on;
                }
            }
            UiMsg::ReferenceEyes(on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.eyes = on;
                    if on {
                        r.extras.retain(|r| *r != Role::HeadPetted);
                        r.tracking = false;
                        r.blink = false;
                        r.faces = false;
                    }
                }
            }
            UiMsg::ReferenceTracking(on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.tracking = on;
                    if on {
                        r.eyes = false;
                    }
                }
            }
            UiMsg::ReferenceBrows(on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.brows = on;
                }
            }
            UiMsg::ReferenceBlink(on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.blink = on;
                    if on {
                        r.eyes = false;
                    }
                }
            }
            UiMsg::ReferenceFaces(on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.faces = on;
                    if on {
                        r.eyes = false;
                    }
                }
            }
            UiMsg::ReferenceExtra(role, on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.extras.retain(|existing| *existing != role);
                    if on {
                        if role == Role::HeadPetted {
                            r.eyes = false;
                            r.extras.retain(|r| *r != Role::Petted);
                        }
                        if role == Role::Petted {
                            r.extras.retain(|r| *r != Role::HeadPetted);
                        }
                        r.extras.push(role);
                    }
                }
            }
            UiMsg::ReferenceHeld(on) => {
                if let Some(r) = &mut self.studio.reference {
                    r.held = on;
                }
            }
            UiMsg::ReferenceCopy(role) => {
                if let Some(r) = &self.studio.reference {
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
                if let Some(r) = &mut self.studio.reference {
                    r.assign(i, role);
                }
            }
            UiMsg::ReferenceCancel => {
                self.studio.reference = None;
                self.studio.reference_thumb = None;
            }
            UiMsg::ReferenceOpen => {
                use crate::mascot::editor::{Editor, Target};
                use crate::mascot::reference::Role;
                let Some(r) = self.studio.reference.clone() else {
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
                let extra_art: Vec<_> = r
                    .roles()
                    .into_iter()
                    .filter_map(|role| Some((role.reaction()?, r.file_for(role)?.to_owned())))
                    .collect();
                let petting = r
                    .extras
                    .iter()
                    .any(|role| matches!(role, Role::Petted | Role::HeadPetted));
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
                self.studio.reference = None;
                self.studio.reference_thumb = None;
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
                    for (event, path) in extra_art {
                        if let Err(e) = editor.set_reference_reaction(event, &path) {
                            errors.push(e);
                        }
                    }
                    if petting {
                        editor.enable_reference_headpat();
                    }
                    Ok(CharacterWork::Editor(Box::new(editor), errors))
                });
            }
            other => return self.reaction_ui(other, now, out),
        }
        Task::none()
    }
}
