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
                if let Some(r) = &mut self.studio.reference {
                    r.dance = on;
                    if on {
                        r.tracking = true;
                        r.eyes = false;
                        r.blink = true;
                    }
                }
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
                use crate::mascot::editor::Editor;
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
                let Some(_body) = r.file_for(Role::Body).map(PathBuf::from) else {
                    self.notice = Some("Assign a file to Body first.".into());
                    return Task::none();
                };
                let request = r.clone();
                self.studio.reference = None;
                self.studio.reference_thumb = None;
                let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
                let id = self.begin_character_job();
                return character_work(id, move || {
                    Editor::from_reference_request(&request, &taken)
                        .map(|editor| CharacterWork::Editor(Box::new(editor), Vec::new()))
                });
            }
            other => return self.reaction_ui(other, now, out),
        }
        Task::none()
    }
}
