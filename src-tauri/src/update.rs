//! Updates from the GitHub Releases, only ever when the user says so. A check (once a day if allowed in
//! Settings, or "Check now") notes a newer release, which the popup shows as a banner. "Install update"
//! downloads that release's installer, makes sure it's the file GitHub lists (size and SHA-256), and runs
//! it with just a progress window; the app closes for it and the installer starts the new version.
//! Nothing is downloaded or installed by itself.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::banners::write_atomic;
use crate::config::{RELEASES_LATEST_URL, RELEASE_DOWNLOAD_PREFIX, UPDATE_DOWNLOAD_TIMEOUT_SECS};
use crate::net;
use crate::time::{parse_iso, Ts};

/// This build's version (from Cargo.toml, kept the same as tauri.conf.json).
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// The installer's options for updating in place: a progress window only (/P), keep the existing install
/// and its shortcuts (/UPDATE), and start the app again when done (/R).
pub const INSTALLER_ARGS: [&str; 3] = ["/P", "/UPDATE", "/R"];

/// A newer release, as listed by GitHub.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,        // "0.3.0"
    pub page: String,           // the release page, for "What's new"
    pub installer: String,      // download URL of its setup .exe
    pub size: u64,
    pub sha256: Option<String>, // lower-case hex, when GitHub lists it
    pub published: Option<Ts>,
}

/// "v0.2.1" or "0.2.1" as numbers.
pub fn version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u32>().ok());
    let found = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(found)
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((version(candidate), version(current)), (Some(a), Some(b)) if a > b)
}

/// Read GitHub's "latest release" answer. The installer must come from this repository's releases.
pub fn parse_latest(payload: &Value) -> Result<Release, String> {
    let tag = payload["tag_name"].as_str().ok_or("no version in the release")?;
    let version = version(tag).map(|(a, b, c)| format!("{a}.{b}.{c}")).ok_or("unexpected version in the release")?;
    let asset = payload["assets"].as_array().into_iter().flatten()
        .find(|a| a["name"].as_str().is_some_and(|n| n.ends_with("_x64-setup.exe")))
        .ok_or("the release has no installer")?;
    let installer = asset["browser_download_url"].as_str().unwrap_or_default();
    if !installer.starts_with(RELEASE_DOWNLOAD_PREFIX) {
        return Err("the installer isn't on this app's release page".into());
    }
    Ok(Release {
        version,
        page: payload["html_url"].as_str().unwrap_or_default().to_string(),
        installer: installer.to_string(),
        size: asset["size"].as_u64().unwrap_or(0),
        sha256: asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_ascii_lowercase),
        published: payload["published_at"].as_str().and_then(parse_iso),
    })
}

/// Ask GitHub for the latest release: Some if it's newer than this build.
pub fn check() -> Result<Option<Release>, String> {
    let body = net::get(RELEASES_LATEST_URL)?;
    let payload: Value = serde_json::from_slice(&body).map_err(|e| format!("not JSON ({e})"))?;
    let latest = parse_latest(&payload)?;
    Ok(is_newer(&latest.version, CURRENT).then_some(latest))
}

/// The SHA-256 from `certutil -hashfile` output (older Windows puts spaces between the bytes).
pub fn certutil_hash(output: &str) -> Option<String> {
    output.lines().nth(1).map(|l| l.replace(' ', "").to_ascii_lowercase())
        .filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()))
}

pub(crate) fn sha256(path: &Path) -> Result<String, String> {
    let output = net::hidden("certutil.exe").arg("-hashfile").arg(path).arg("SHA256").output()
        .map_err(|e| format!("could not run certutil ({e})"))?;
    certutil_hash(&String::from_utf8_lossy(&output.stdout)).ok_or_else(|| "could not check the download".into())
}

/// Download the release's installer into `dir` and check it's exactly the file GitHub lists.
pub fn download(release: &Release, dir: &Path) -> Result<PathBuf, String> {
    let data = net::get_within(&release.installer, UPDATE_DOWNLOAD_TIMEOUT_SECS)?;
    if !data.starts_with(b"MZ") || (release.size > 0 && data.len() as u64 != release.size) {
        return Err("the download wasn't the installer. Please try again".into());
    }
    let path = dir.join(format!("GachaSlop_{}_x64-setup.exe", release.version));
    write_atomic(&path, &data).map_err(|e| e.to_string())?;
    if let Some(expected) = &release.sha256
        && &sha256(&path)? != expected
    {
        let _ = std::fs::remove_file(&path);
        return Err("the download didn't match the release. Please try again".into());
    }
    Ok(path)
}

/// Start the installer in update mode. The caller then quits so the installer can replace the app.
pub fn run_installer(path: &Path) -> Result<(), String> {
    std::process::Command::new(path).args(INSTALLER_ARGS).spawn().map(|_| ()).map_err(|e| e.to_string())
}
