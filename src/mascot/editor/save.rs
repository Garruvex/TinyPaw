//! Writing the edited character back to its pack.

use super::*;

impl Editor {
    /// Writes the edited character over its installed copy.
    pub fn save(&self, job: &std::sync::atomic::AtomicU8) -> Result<Character, String> {
        let root = pack::user_dir().ok_or("no per-user data folder")?;
        self.save_into_guarded(&root, Some(job))
    }

    #[cfg(test)]
    pub(super) fn save_into(&self, root: &Path) -> Result<Character, String> {
        self.save_into_guarded(root, None)
    }

    pub(super) fn save_into_guarded(
        &self,
        root: &Path,
        job: Option<&std::sync::atomic::AtomicU8>,
    ) -> Result<Character, String> {
        // Bundled Yohta's pack is never rewritten; Edit a copy instead.
        if self.id == pack::BUILTIN {
            return Err("Yohta is built in; edit a copy instead".into());
        }
        if let Some(error) = self.validation_error() {
            return Err(error);
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err("give the character a name".into());
        }
        let head = self.head();
        // Write each image under a name from its role, then the manifest.
        let dir = &self.pose;
        let mut files: Vec<(String, Layer)> = vec![(format!("{dir}/body.png"), self.body.clone())];
        // A held pose made here is pack art: Pick up shows it while carried.
        let mut reactions = self.reactions.clone();
        if self.held.is_some() {
            use super::super::reactions::{Art, Entry, Event};
            let file = HELD_FILE.to_owned();
            let mut entry = reactions
                .entry(Event::Pickup)
                .cloned()
                .unwrap_or_else(Entry::none);
            entry.held = Some(Art::new(super::super::lines::SoundRef::Pack(file)));
            reactions.set_entry(Event::Pickup, Some(entry));
        }
        // Display names travel in the editor metadata, by image.
        let mut names: Vec<(String, String)> = Vec::new();
        let mut parts = Vec::new();
        for (n, p) in self.parts.iter().enumerate() {
            let base = format!("{dir}/part-{}-{}", n + 1, p.kind.name());
            names.push((format!("{base}.png"), p.name.clone()));
            files.push((format!("{base}.png"), p.layer.clone()));
            let mut faces = [None, None, None, None];
            for (slot, face) in p.faces.iter().enumerate() {
                if let Some(face) = face {
                    let file = format!("{base}-{}.png", FACE_NAMES[slot].to_lowercase());
                    files.push((file.clone(), face.clone()));
                    faces[slot] = Some(file);
                }
            }
            parts.push(PartDef {
                kind: p.kind,
                image: format!("{base}.png"),
                pivot: p.pivot,
                parent: if p.on_head && p.kind != PartType::Head {
                    head
                } else {
                    None
                },
                behind: p.behind,
                motion: p.motion,
                faces,
            });
        }
        let mut eyes = Vec::new();
        for (n, e) in self.eyes.iter().enumerate() {
            let (Some(white), Some(pupil), Some(mask)) = (&e.white, &e.pupil, &e.mask) else {
                return Err(format!(
                    "eye {} needs a white, a pupil and a mask image",
                    n + 1
                ));
            };
            let base = format!("{dir}/eye-{}", n + 1);
            let mut eye = EyeDef {
                white: format!("{base}-white.png"),
                pupil: format!("{base}-pupil.png"),
                mask: format!("{base}-mask.png"),
                blink: None,
                max_offset: (e.travel, e.travel * 0.75),
            };
            files.push((eye.white.clone(), white.clone()));
            files.push((eye.pupil.clone(), pupil.clone()));
            files.push((eye.mask.clone(), mask.clone()));
            if let Some(blink) = &e.blink {
                eye.blink = Some(format!("{base}-blink.png"));
                files.push((format!("{base}-blink.png"), blink.clone()));
            }
            eyes.push(eye);
        }
        let ground = match self.rest {
            RestMode::Auto => self.auto_rest().unwrap_or(self.ground),
            RestMode::Manual => self.ground,
        };
        let mut poses = vec![Pose {
            // Layered poses give each part its own motion.
            whole: None,
            name: self.pose.clone(),
            ground,
            body: format!("{dir}/body.png"),
            parts,
            eyes,
        }];
        poses.extend(self.others.iter().cloned());
        let runtime_bytes = u64::from(self.canvas.0)
            * u64::from(self.canvas.1)
            * 4
            * poses
                .iter()
                .map(|pose| pose.assets().len() as u64)
                .sum::<u64>();
        let source_bytes: u64 = files
            .iter()
            .map(|(_, layer)| u64::from(layer.image.width()) * u64::from(layer.image.height()) * 8)
            .chain(
                self.other_sources
                    .iter()
                    .map(|record| u64::from(record.size.0) * u64::from(record.size.1) * 8),
            )
            .sum();
        let compact = runtime_bytes + source_bytes > 128 * 1024 * 1024;
        let sources: Vec<Layer> = files
            .iter()
            .map(|(_, layer)| {
                if compact {
                    layer.compact_source()
                } else {
                    layer.clone()
                }
            })
            .collect();
        let manifest = Manifest {
            id: self.id.to_owned(),
            name: name.chars().take(60).collect(),
            author: self.author.clone(),
            lines: self.lines.clone(),
            bop: self.bop,
            reactions,
            canvas: self.canvas,
            default_pose: self.pose.clone(),
            poses,
            source: self.source,
            rest: self.rest,
            editor: files
                .iter()
                .zip(&sources)
                .enumerate()
                .map(|(i, ((file, _), layer))| SourceRecord {
                    name: names
                        .iter()
                        .find(|(f, _)| f == file)
                        .map(|(_, n)| n.clone()),
                    target: file.clone(),
                    source: format!("editor/sources/{dir}/layer-{i}.png"),
                    size: layer.image.dimensions(),
                    placement: layer.placement,
                })
                .chain(self.other_sources.iter().cloned())
                .collect(),
        };
        // Other poses' images come from the installed copy.
        let installed = root.join(self.id);
        let mut kept: Vec<String> = self
            .others
            .iter()
            .flat_map(|p| {
                p.assets()
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        kept.extend(self.other_sources.iter().map(|r| r.source.clone()));
        let write = |stage: &Path| {
            for (file, image) in &files {
                let to = stage.join(file);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                placement::bake(&image.image, image.placement, self.canvas)
                    .save_with_format(&to, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
            }
            if let Some(held) = &self.held {
                let to = stage.join(HELD_FILE);
                std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
                placement::bake(&held.image, held.placement, self.canvas)
                    .save_with_format(&to, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
            }
            for (layer, record) in sources.iter().zip(&manifest.editor) {
                let to = stage.join(&record.source);
                std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
                layer
                    .image
                    .save_with_format(to, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
            }
            for asset in &kept {
                let to = stage.join(asset);
                if to.exists() {
                    continue;
                }
                let dimensions = manifest
                    .resources()
                    .into_iter()
                    .find(|(name, _)| *name == asset)
                    .map(|(_, dimensions)| dimensions)
                    .ok_or("undeclared image in another pose")?;
                read_png(&installed, asset, dimensions)?;
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::copy(installed.join(asset), to).map_err(|e| e.to_string())?;
            }
            // Reaction pictures and line sounds come along unchanged.
            for image in manifest.reactions.images() {
                if let super::super::lines::SoundRef::Pack(asset) = image {
                    let to = stage.join(asset);
                    // Already written: a held pose made in this editor.
                    if to.exists() {
                        continue;
                    }
                    if let Some(parent) = to.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::copy(installed.join(asset), to).map_err(|e| e.to_string())?;
                }
            }
            for sound in manifest.sounds() {
                let to = stage.join(sound);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::copy(installed.join(sound), to).map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        let replace = (!self.new_install).then_some(self.id);
        if let Some(job) = job {
            pack::stage_and_install_guarded(root, &manifest, replace, write, || {
                job.compare_exchange(
                    0,
                    1,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .map(|_| ())
                .map_err(|_| "Import cancelled".into())
            })
        } else {
            stage_and_install(root, &manifest, replace, write)
        }
    }
}
