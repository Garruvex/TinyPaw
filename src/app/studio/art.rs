use super::*;

impl App {
    /// Art tab edits.
    pub(super) fn art_ui(&mut self, ui: UiMsg, now: Millis, out: &mut Outcomes) -> Task<Message> {
        match ui {
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
                self.studio.character_draft = None;
                return character_work(id, move || {
                    crate::mascot::editor::Editor::open(&c)
                        .map(|e| CharacterWork::Editor(Box::new(e), Vec::new()))
                });
            }
            UiMsg::ArtFromParts => {
                self.cancel_character_job();
                let generation = self.studio.character_request;
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
                let generation = self.studio.character_request;
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
                if let Some(e) = &mut self.studio.art {
                    e.select(s);
                }
            }
            UiMsg::ArtKind(i, kind) => {
                if let Some(e) = &mut self.studio.art {
                    e.checkpoint();
                    e.set_kind(i, kind);
                }
            }
            UiMsg::ArtOnHead(i, on) => {
                if let Some(p) = self.studio.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.on_head = on;
                }
            }
            UiMsg::ArtBehind(i, behind) => {
                if let Some(p) = self.studio.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.behind = behind;
                }
            }
            UiMsg::ArtRemovePart(i) => {
                if let Some(e) = &mut self.studio.art {
                    e.checkpoint();
                    e.remove_part(i);
                }
            }
            UiMsg::ArtRemoveFace(i, slot) => {
                if let Some(p) = self.studio.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.faces[slot.min(2)] = None;
                }
            }
            UiMsg::ArtStaticEye(i) => {
                if let Some(e) = &mut self.studio.art {
                    self.notice = e.keep_eye_static(i).err();
                }
            }
            UiMsg::ArtAddEye => {
                if let Some(e) = &mut self.studio.art {
                    e.checkpoint();
                    e.add_eye();
                }
            }
            UiMsg::ArtRemoveEye(i) => {
                if let Some(e) = &mut self.studio.art
                    && i < e.eyes.len()
                {
                    e.eyes.remove(i);
                }
            }
            UiMsg::ArtRemoveBlink(i) => {
                if let Some(eye) = self.studio.art.as_mut().and_then(|e| e.eyes.get_mut(i)) {
                    eye.blink = None;
                }
            }
            UiMsg::ArtTravel(i, travel) => {
                if let Some(e) = &mut self.studio.art {
                    e.checkpoint_gesture(crate::mascot::editor::Gesture::Travel(i));
                }
                if let Some(eye) = self.studio.art.as_mut().and_then(|e| e.eyes.get_mut(i)) {
                    eye.travel = travel.clamp(0.0, 0.05);
                }
            }
            UiMsg::ArtPupilSize(i, scale) => {
                if let Some(e) = &mut self.studio.art {
                    self.notice = e.set_pupil_size(i, scale).err();
                }
            }
            UiMsg::ArtClick(x, y) => {
                if let Some(e) = &mut self.studio.art {
                    e.checkpoint();
                    e.click(x, y);
                }
            }
            UiMsg::ArtName(name) => {
                if let Some(e) = &mut self.studio.art {
                    e.name = name.chars().take(60).collect();
                }
            }
            UiMsg::ArtCancel => self.discard_character(),
            UiMsg::ArtSave => {
                if self.studio.character_job.is_none()
                    && let Some(editor) = self.studio.art.clone()
                {
                    self.studio.art_saving = Some(editor.id);
                    let id = self.begin_character_job();
                    let job = self.studio.character_job.as_ref().unwrap().clone();
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
            UiMsg::ArtJoint(part) => {
                if let Some(e) = &mut self.studio.art {
                    match part {
                        Some(i) => e.adjust_joint(i),
                        None => e.finish_joint(),
                    }
                }
            }
            UiMsg::ArtRename(i, name) => {
                if let Some(e) = &mut self.studio.art {
                    e.rename(i, name);
                }
            }
            UiMsg::ArtEyeStyle(style) => {
                if let Some(e) = &mut self.studio.art {
                    e.eye_style = Some(style);
                }
            }
            UiMsg::ArtRest(action) => {
                if let Some(e) = &mut self.studio.art {
                    use crate::mascot::editor::RestAction;
                    match action {
                        RestAction::Adjust => e.adjust_rest(),
                        RestAction::Done => e.finish_rest(),
                        RestAction::Reset => e.reset_rest(),
                    }
                }
            }
            UiMsg::ArtField(i, text) => {
                if let Some(e) = &mut self.studio.art {
                    e.field(i, text);
                }
            }
            UiMsg::ArtCanvas(i, text) => {
                if let Some(e) = &mut self.studio.art {
                    e.resize_canvas(i, text);
                }
            }
            UiMsg::ArtDragStart(selected) => {
                if let Some(e) = &mut self.studio.art {
                    e.select(selected);
                    e.checkpoint();
                }
            }
            UiMsg::ArtDrag(dx, dy) => {
                if let Some(e) = &mut self.studio.art {
                    e.translate(dx, dy);
                }
            }
            UiMsg::ArtNudge(dx, dy) => {
                if let Some(e) = &mut self.studio.art {
                    e.checkpoint_gesture(crate::mascot::editor::Gesture::Nudge);
                    e.nudge(dx, dy);
                }
            }
            UiMsg::ArtOrder(i, up) => {
                if let Some(e) = &mut self.studio.art {
                    e.reorder(i, up);
                }
            }
            UiMsg::ArtUndo => {
                if let Some(e) = &mut self.studio.art {
                    e.undo();
                }
            }
            UiMsg::ArtRedo => {
                if let Some(e) = &mut self.studio.art {
                    e.redo();
                }
            }
            UiMsg::ArtZoom(zoom) => {
                if let Some(e) = &mut self.studio.art {
                    e.zoom = zoom.clamp(0.5, 3.0);
                }
            }
            UiMsg::ArtBackground(bg) => {
                if let Some(e) = &mut self.studio.art {
                    e.background = bg;
                }
            }
            other => return self.reference_ui(other, now, out),
        }
        Task::none()
    }
}
