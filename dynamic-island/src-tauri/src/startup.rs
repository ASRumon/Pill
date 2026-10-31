//! Logon startup integration.
//!
//! The app used to rely on `tauri-plugin-autostart`. On Windows that plugin
//! writes a `Run` value whose data can end up pointing at a stale executable
//! after an update, and the entry is invisible in Task Manager unless the value
//! name matches the app. This module owns the registration directly:
//!
//! * `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` → `Pill` =
//!   `"<full path to Pill.exe>"` so Task Manager → Startup apps lists the app.
//! * `Explorer\StartupApproved\Run` is cleared on enable and deleted on disable,
//!   so a previously disabled entry does not silently stay off.
//! * Every launch re-writes the command when the executable moved (store
//!   updates, per-user vs machine installs), which keeps startup working after
//!   an update.
//!
//! The key strings include the hive: `reg.exe` rejects a bare
//! `Software\...` path with "Invalid key name".

use crate::state::AppState;
use std::process::Command;

const RUN_KEY: &str = r"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str =
    r"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VALUE_NAME: &str = "Pill";
/// Marker in the settings table so a removed registry entry can be rebuilt.
pub const MARKER_KEY: &str = "autostart_enabled";

/// Keeps `reg.exe` from flashing a console window at logon.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn reg(args: &[&str]) -> (bool, String) {
    let mut command = Command::new("reg.exe");
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    match command.output() {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            (output.status.success(), text)
        }
        Err(error) => (false, error.to_string()),
    }
}

/// The exact command Windows should run at logon: the current executable,
/// quoted so install paths with spaces survive the registry round trip.
fn launch_command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate Pill.exe: {e}"))?;
    let path = exe.to_string_lossy().to_string();
    Ok(format!("\"{path}\""))
}

fn read_run_value() -> Option<String> {
    let (ok, output) = reg(&["query", RUN_KEY, "/v", VALUE_NAME]);
    if !ok {
        return None;
    }
    for line in output.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(VALUE_NAME) else {
            continue;
        };
        if let Some(position) = rest.find("REG_SZ") {
            let value = rest[position + "REG_SZ".len()..].trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// Task Manager writes a binary blob under `StartupApproved` when the user
/// disables an entry. The low bit of the first byte is the enabled flag.
fn disabled_by_task_manager() -> bool {
    let (ok, output) = reg(&["query", APPROVED_KEY, "/v", VALUE_NAME]);
    if !ok {
        return false;
    }
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(VALUE_NAME) {
            continue;
        }
        let Some(position) = trimmed.find("REG_BINARY") else {
            continue;
        };
        let hex = trimmed[position + "REG_BINARY".len()..].trim();
        let first = hex.get(0..2).unwrap_or("");
        if let Ok(byte) = u8::from_str_radix(first, 16) {
            return byte & 1 == 1;
        }
    }
    false
}

/// Whether Windows will launch Pill at the next logon.
pub fn is_enabled() -> bool {
    read_run_value().is_some() && !disabled_by_task_manager()
}

fn write_entry(command: &str) -> Result<(), String> {
    let (ok, output) = reg(&[
        "add", RUN_KEY, "/v", VALUE_NAME, "/t", "REG_SZ", "/d", command, "/f",
    ]);
    if ok {
        Ok(())
    } else {
        Err(format!("startup registration failed: {}", output.trim()))
    }
}

fn clear_approval_flag() {
    // Removing the approval value makes Windows treat the entry as enabled.
    let _ = reg(&["delete", APPROVED_KEY, "/v", VALUE_NAME, "/f"]);
}

fn remove_entry() {
    let _ = reg(&["delete", RUN_KEY, "/v", VALUE_NAME, "/f"]);
    clear_approval_flag();
}

fn set_marker(state: &AppState, enabled: bool) {
    if let Ok(db) = state.db.lock() {
        let _ =
            crate::settings::write_setting(&db, MARKER_KEY, if enabled { "true" } else { "false" });
    }
}

fn stored_marker(state: &AppState) -> bool {
    state
        .db
        .lock()
        .ok()
        .and_then(|db| crate::settings::read_setting(&db, MARKER_KEY))
        .is_some_and(|value| value == "true")
}

/// Enables or disables logon startup and returns the resulting state.
pub fn set_enabled(state: &AppState, enabled: bool) -> Result<bool, String> {
    if enabled {
        let command = launch_command()?;
        write_entry(&command)?;
        // A stale Task Manager "disabled" flag would keep the entry from
        // running even though we just registered it.
        clear_approval_flag();
        let registered = is_enabled();
        set_marker(state, registered);
        Ok(registered)
    } else {
        remove_entry();
        set_marker(state, false);
        Ok(read_run_value().is_some())
    }
}

/// Repairs the registration after an update or a move.
///
/// The registry is the source of truth for "is it on", but the executable path
/// inside it goes stale whenever the app is reinstalled somewhere else. If the
/// user enabled startup previously, rewrite the command with the current path.
pub fn sync_on_launch(state: &AppState) {
    let exe = match launch_command() {
        Ok(command) => command,
        Err(_) => return,
    };

    match read_run_value() {
        Some(existing) => {
            if !stored_marker(state) {
                // Adopt an entry created by an older build so the path keeps
                // getting refreshed from now on.
                set_marker(state, true);
            }
            if existing != exe {
                let _ = write_entry(&exe);
            }
        }
        None => {
            if stored_marker(state) {
                let _ = write_entry(&exe);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keys_include_the_hive() {
        // `reg.exe` fails with "Invalid key name" without the hive prefix.
        assert!(RUN_KEY.starts_with("HKEY_CURRENT_USER\\"), "{RUN_KEY}");
        assert!(
            APPROVED_KEY.starts_with("HKEY_CURRENT_USER\\"),
            "{APPROVED_KEY}"
        );
        assert!(RUN_KEY.ends_with("\\Run"), "{RUN_KEY}");
    }

    #[cfg(windows)]
    #[test]
    fn run_key_resolves() {
        let (ok, output) = reg(&["query", RUN_KEY]);
        assert!(ok, "reg query failed: {output}");
        assert!(!output.contains("Invalid key name"), "{output}");
    }

    #[test]
    fn launch_command_is_quoted() {
        let command = launch_command().expect("current exe");
        assert!(
            command.starts_with('"') && command.ends_with('"'),
            "{command}"
        );
    }
}
