//! All of the app's Rust tests, kept apart from the code. They live inside the crate (not in a top-level
//! tests/ folder) so they can use its private modules. Run with `cargo test` in src-tauri.
mod banners;
mod commands;
mod games;
mod scheduler;
mod settings;
mod system;
mod time;
mod toast;
mod tray;
mod widget;

use std::path::{Path, PathBuf};

use crate::banners::{FeedKind, Icon, Pick, Rotation};
use crate::time::{days_from_civil, Ts, DAY, HOUR, MINUTE};

/// A UTC moment from its calendar parts.
pub fn utc(year: i64, month: u32, day: u32, hour: i64, minute: i64) -> Ts {
    days_from_civil(year, month, day) * DAY + hour * HOUR + minute * MINUTE
}

/// A test JSON file from src-tauri/fixtures.
pub fn fixture(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A rotation with one pick, whose icon file is "<feed>/<name>.png".
pub fn rotation(kind: FeedKind, start: Ts, end: Ts, name: &str) -> Rotation {
    let icon = Icon { file: format!("{}/{name}.png", kind.key()), url: String::new() };
    let pick = Pick { name: name.into(), marks: String::new(), icon: Some(icon), engine: None };
    Rotation::new(kind, start, end, vec![pick], vec![], vec![])
}

/// An empty folder of its own for one test, removed when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gachaslop-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
