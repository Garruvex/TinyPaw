//! The resident app's side of the Slint Settings window. The window is its own
//! process (`tinypaw --settings`); it reads the settings with `settings-state`
//! and changes them with the same `set` command the CLI uses, so every control
//! saves at once through the one path.

use super::*;

impl App {
    /// Whether Settings opens the Slint window. Opt-in while pages move over.
    pub(super) fn slint_settings_wanted() -> bool {
        std::env::var_os("TINYPAW_SLINT_SETTINGS").is_some()
    }

    /// Opens the Settings process, or asks the open one to come forward.
    pub(super) fn open_slint_settings(&mut self) {
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
        let mut command = std::process::Command::new(exe);
        command.arg("--settings");
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
        let state = serde_json::json!({
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
            _ => return format!("error: unknown settings action `{name}`"),
        }
        "ok".into()
    }
}
