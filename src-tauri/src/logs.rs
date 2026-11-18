//! A tiny log: timestamped lines appended to GachaSlop.log in the app's log folder (and the terminal in
//! debug builds).
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::config::MAX_LOG_BYTES;
use crate::time::{fmt_iso, now};

static FILE: OnceLock<PathBuf> = OnceLock::new();

/// Start logging into `dir`. A log over MAX_LOG_BYTES is started afresh.
pub fn init(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("GachaSlop.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = std::fs::write(&path, "");
    }
    let _ = FILE.set(path);
}

fn write(level: &str, message: &str) {
    let line = format!("{} {level} {message}\n", fmt_iso(now()));
    if cfg!(debug_assertions) {
        eprint!("{line}");
    }
    if let Some(path) = FILE.get()
        && let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

pub fn info(message: impl AsRef<str>) {
    write("INFO", message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    write("WARN", message.as_ref());
}
