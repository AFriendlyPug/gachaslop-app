//! Running Windows' own tools without a console window flashing up, and HTTP GET through Windows'
//! built-in curl.exe (Windows 10 1803 and later), so the app needs no HTTP library.
use std::path::PathBuf;
use std::process::Command;

use crate::config::{HTTP_TIMEOUT_SECS, USER_AGENT};

/// A program in Windows' System32 folder.
pub fn system32(program: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(root).join("System32").join(program)
}

/// A command that runs without opening a console window.
pub fn hidden(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(system32(program));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Arguments for one quiet GET that fails on HTTP errors, follows redirects and accepts gzip, giving up
/// after `seconds`.
pub fn curl_args(url: &str, seconds: u32) -> Vec<String> {
    ["-sS", "-f", "-L", "--compressed", "--max-time", &seconds.to_string(), "-A", USER_AGENT, "--", url]
        .map(String::from).to_vec()
}

/// Download `url`. Any failure (no network, HTTP error, curl missing) comes back as a short message.
pub fn get(url: &str) -> Result<Vec<u8>, String> {
    get_within(url, HTTP_TIMEOUT_SECS)
}

/// get(), allowing longer for something big (an update's installer).
pub fn get_within(url: &str, seconds: u32) -> Result<Vec<u8>, String> {
    let output = hidden("curl.exe").args(curl_args(url, seconds)).output()
        .map_err(|e| format!("could not run curl ({e})"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if err.is_empty() { format!("curl failed ({})", output.status) } else { err });
    }
    Ok(output.stdout)
}
