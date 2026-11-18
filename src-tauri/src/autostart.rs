//! "Start with Windows": an entry under the current user's Run key, written with Windows' own reg.exe.
use std::path::Path;

use crate::config::AUTOSTART_NAME;
use crate::net::hidden;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

pub fn enable_args(exe: &Path) -> Vec<String> {
    let command = format!("\"{}\" --minimised", exe.display());
    ["add", RUN_KEY, "/v", AUTOSTART_NAME, "/t", "REG_SZ", "/d", &command, "/f"].map(String::from).to_vec()
}

pub fn disable_args() -> Vec<String> {
    ["delete", RUN_KEY, "/v", AUTOSTART_NAME, "/f"].map(String::from).to_vec()
}

pub fn query_args() -> Vec<String> {
    ["query", RUN_KEY, "/v", AUTOSTART_NAME].map(String::from).to_vec()
}

fn reg(args: Vec<String>) -> bool {
    hidden("reg.exe").args(args).output().is_ok_and(|out| out.status.success())
}

pub fn is_enabled() -> bool {
    reg(query_args())
}

/// Turn starting with Windows on or off for this copy of the app.
pub fn set(enabled: bool) -> Result<(), String> {
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        reg(enable_args(&exe)).then_some(()).ok_or_else(|| "Windows wouldn't add the startup entry.".into())
    } else if !is_enabled() || reg(disable_args()) {
        Ok(())
    } else {
        Err("Windows wouldn't remove the startup entry.".into())
    }
}
