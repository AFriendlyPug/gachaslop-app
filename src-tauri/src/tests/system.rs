//! The Windows tools the app runs (curl.exe, reg.exe) and the one-copy-at-a-time handshake.
use std::path::Path;

use crate::autostart::{disable_args, enable_args, query_args};
use crate::config::USER_AGENT;
use crate::instance::{OK, SHOW};
use crate::net::{curl_args, system32};

#[test]
fn curl_makes_one_quiet_get_that_fails_on_http_errors() {
    let args = curl_args("https://example.com/a?b=c", 15);
    assert_eq!(args, ["-sS", "-f", "-L", "--compressed", "--max-time", "15", "-A", USER_AGENT, "--",
                      "https://example.com/a?b=c"]);
    // the URL comes after "--", so it can never be read as an option
    assert_eq!(curl_args("-o evil", 15)[args.len() - 1], "-o evil");
    assert!(system32("curl.exe").ends_with("System32\\curl.exe") || system32("curl.exe").ends_with("System32/curl.exe"));
}

#[test]
fn start_with_windows_uses_the_current_users_run_key() {
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let exe = Path::new(r"C:\Users\Me\AppData\Local\GachaSlop\gachaslop-app.exe");
    assert_eq!(enable_args(exe), ["add", key, "/v", "GachaSlop", "/t", "REG_SZ", "/d",
                                  r#""C:\Users\Me\AppData\Local\GachaSlop\gachaslop-app.exe" --minimised"#, "/f"]);
    assert_eq!(disable_args(), ["delete", key, "/v", "GachaSlop", "/f"]);
    assert_eq!(query_args(), ["query", key, "/v", "GachaSlop"]);
}

#[test]
fn single_instance_messages_are_distinct_lines() {
    assert_ne!(SHOW, OK);
    assert!(SHOW.ends_with(b"\n") && OK.ends_with(b"\n"));
    assert!(SHOW.starts_with(b"gachaslop:") && OK.starts_with(b"gachaslop:"));
}

#[test]
fn image_links_must_be_https() {
    assert_eq!(crate::net::get_image("http://example.com/a.png", 1000), Err("The link must start with https://".into()));
    assert!(crate::net::get_image("file:///C:/Windows/win.ini", 1000).is_err());
}
