//! The resident app's side of the Slint Settings window. The window is its own
//! process (`tinypaw --settings`); it reads the settings with `settings-state`
//! and changes them with the same `set` command the CLI uses, so every control
//! saves at once through the one path.

use super::*;

impl App {
    /// Shows the first-run card, a window of its own process.
    pub(super) fn open_first_run(&mut self) {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let mut command = std::process::Command::new(exe);
        command.arg("--first-run");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let _ = command.spawn();
    }

    /// Opens the Settings process, or asks the open one to come forward.
    pub(super) fn open_slint_settings(&mut self) {
        self.open_slint_settings_on("");
    }

    /// As `open_slint_settings`, on the named page when it has to start.
    pub(super) fn open_slint_settings_on(&mut self, page: &str) {
        if let Some(child) = &mut self.settings_child {
            if matches!(child.try_wait(), Ok(None)) {
                self.settings_focus = true;
                return;
            }
            self.settings_child = None;
        }
        let Ok(exe) = std::env::current_exe() else {
            self.notice = Some("Could not start Settings.".into());
            return;
        };
        #[cfg(windows)]
        {
            // The lens page lists the windows open now.
            self.sources = platform::windows::window_sources();
        }
        let mut command = std::process::Command::new(exe);
        command.arg("--settings");
        if !page.is_empty() {
            command.arg(page);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window for the GUI child.
            command.creation_flags(0x0800_0000);
        }
        match command.spawn() {
            Ok(child) => self.settings_child = Some(child),
            Err(e) => self.notice = Some(format!("Could not start Settings: {e}")),
        }
    }

    /// The `settings-state` reply: every setting, the displays and whether a
    /// custom tap sound exists. Reading it clears a pending focus request.
    pub(super) fn settings_state(&mut self) -> String {
        let displays: Vec<_> = self
            .displays
            .iter()
            .map(|d| serde_json::json!({ "name": d.name, "text": d.to_string() }))
            .collect();
        let taken = self
            .hotkeys
            .as_ref()
            .map_or(&[][..], |h| h.taken.as_slice());
        let mut shortcuts = Vec::new();
        for (context, name) in [
            (crate::registry::ShortcutContext::Global, "global"),
            (crate::registry::ShortcutContext::Overlay, "overlay"),
        ] {
            for (shortcut, def) in self.registry.shortcuts(context) {
                shortcuts.push(serde_json::json!({
                    "id": def.id,
                    "label": def.label,
                    "binding": shortcut.label(),
                    "context": name,
                    "taken": taken.contains(&def.id),
                }));
            }
        }
        let windows: Vec<_> = self
            .sources
            .iter()
            .map(|w| serde_json::json!({ "id": w.id, "title": w.title }))
            .collect();
        let packs: Vec<_> = self
            .packs
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "name": p.name,
                    "accent": p.accent.map(|[r, g, b, _]| format!("#{r:02x}{g:02x}{b:02x}")),
                    "custom": p.folder.is_some(),
                    "forearm": p.forearm.is_some(),
                })
            })
            .collect();
        let characters: Vec<_> = self
            .characters
            .iter()
            .map(|c| serde_json::json!({ "id": c.id, "name": c.name, "builtin": c.is_builtin() }))
            .collect();
        let music = {
            let (status, level) = self.music_status();
            let label = |s: &crate::mascot::music::Source| serde_json::json!({ "id": s.id(), "label": s.to_string() });
            let rows: Vec<_> = self
                .music_rows()
                .iter()
                .map(|(s, _, playing)| {
                    serde_json::json!({ "id": s.id(), "label": s.to_string(), "playing": playing })
                })
                .collect();
            let inputs: Vec<_> = self.music_inputs().iter().map(label).collect();
            serde_json::json!({
                "status": status,
                "level": level,
                "beat": self.beat_status(),
                "rows": rows,
                "inputs": inputs,
            })
        };
        let state = serde_json::json!({
            "characters": characters,
            "music": music,
            "packs": packs,
            "windows": windows,
            "shortcuts": shortcuts,
            "values": crate::preferences::to_json(self.session.settings()),
            "displays": displays,
            "custom_sound": self.audio.has_custom(),
            "focus": std::mem::take(&mut self.settings_focus),
        });
        format!("ok: {state}")
    }

    /// A button on a Slint settings page.
    pub(super) fn settings_action(
        &mut self,
        name: &str,
        now: Millis,
        out: &mut Outcomes,
        tasks: &mut Vec<Task<Message>>,
    ) -> String {
        match name {
            "sound.preview" => tasks.push(self.ui_message(UiMsg::PreviewSound, now, out)),
            "sound.import" => tasks.push(self.ui_message(UiMsg::ImportWav, now, out)),
            "open-classic" => tasks.push(self.open_classic_settings()),
            "shortcuts.reset" => self.reset_shortcuts(),
            "pointer.add" => tasks.push(self.ui_message(UiMsg::AddPointer, now, out)),
            "pointer.import" => tasks.push(self.ui_message(UiMsg::ImportPack, now, out)),
            "pointer.reaim" => {
                tasks.push(self.ui_message(UiMsg::EditPack, now, out));
                if self.pointer_draft.is_some() {
                    self.page = crate::view::settings::Page::Pointer;
                    tasks.push(self.open_classic_settings());
                }
            }
            "pointer.remove" => tasks.push(self.ui_message(UiMsg::RemovePack, now, out)),
            "pointer.prompt" => tasks.push(self.ui_message(UiMsg::CopyPointerPrompt, now, out)),
            "pointer.arm-prompt" => tasks.push(self.ui_message(UiMsg::CopyForearmPrompt, now, out)),
            "first-run.turn-on" => {
                if !self.session.is_enabled() {
                    out.collect(
                        self.dispatch(crate::features::system::MASTER_TOGGLE, now)
                            .ok(),
                    );
                }
            }
            "first-run.settings" => self.open_slint_settings(),
            "characters.edit" => {
                self.page = crate::view::settings::Page::Character;
                tasks.push(self.open_classic_settings());
            }
            "character.reset" => tasks.push(self.ui_message(UiMsg::ResetMascotPosition, now, out)),
            "music.reset-hand" => {
                use crate::features::mascot as character;
                for (id, value) in [
                    (character::WAVE_X, 0.0),
                    (character::WAVE_Y, 0.0),
                    (character::WAVE_ANGLE, 0.0),
                    (character::WAVE_RANGE, 100.0),
                ] {
                    out.sets.push((id, Value::Float(value)));
                }
            }
            "lens.refresh" => {
                #[cfg(windows)]
                {
                    self.sources = platform::windows::window_sources();
                }
            }
            _ => return format!("error: unknown settings action `{name}`"),
        }
        "ok".into()
    }
}

impl App {
    /// `shortcut-bind <command> <binding>` from the Settings window.
    pub(super) fn settings_bind(&mut self, rest: &str) -> String {
        let Some((id, binding)) = rest.split_once(char::is_whitespace) else {
            return "error: usage: shortcut-bind <command> <binding>".into();
        };
        let Some(command) = self.registry.command(id).map(|d| d.id) else {
            return format!("error: unknown command `{id}`");
        };
        match self.rebind_shortcut(command, binding.trim()) {
            Ok(()) => "ok".into(),
            Err(why) => format!("error: {why}"),
        }
    }
}
