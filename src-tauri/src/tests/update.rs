//! Updates: versions, reading GitHub's latest release, checking the download, and when to check.
use super::{fixture, utc, TempDir};
use crate::config::{UPDATE_CHECK_EVERY, UPDATE_RETRY};
use crate::scheduler::{record_update_check, update_due};
use crate::settings::{Settings, State};
use crate::update::*;

#[test]
fn versions_are_compared_as_numbers() {
    assert_eq!(version("v0.2.1"), Some((0, 2, 1)));
    assert_eq!(version("0.10.0"), Some((0, 10, 0)));
    for bad in ["v1.2", "1.2.3.4", "latest", ""] {
        assert_eq!(version(bad), None, "{bad}");
    }
    assert!(is_newer("0.3.0", "0.2.1"));
    assert!(is_newer("v0.10.0", "0.9.9")); // not compared as text
    assert!(!is_newer("0.2.1", "0.2.1"));
    assert!(!is_newer("0.2.0", "0.2.1"));
    assert!(!is_newer("nonsense", "0.2.1"));
}

#[test]
fn the_latest_release_is_read_from_github() {
    // saved from api.github.com on 18 Nov 2026
    let release = parse_latest(&fixture("github_latest.json")).unwrap();
    assert_eq!(release, Release {
        version: "0.2.1".into(),
        page: "https://github.com/AFriendlyPug/gachaslop-app/releases/tag/v0.2.1".into(),
        installer: "https://github.com/AFriendlyPug/gachaslop-app/releases/download/v0.2.1/GachaSlop_0.2.1_x64-setup.exe".into(),
        size: 1_415_511,
        sha256: Some("e9c13c97d3b626f724a0c190ae03d494b873fb27297a3601215b74617548f321".into()),
        published: Some(utc(2026, 11, 18, 5, 35) + 50),
    });
}

#[test]
fn an_installer_from_anywhere_else_is_refused() {
    let mut payload = fixture("github_latest.json");
    payload["assets"][1]["browser_download_url"] = "https://example.com/GachaSlop_0.2.1_x64-setup.exe".into();
    assert!(parse_latest(&payload).is_err());
    let mut payload = fixture("github_latest.json");
    payload["assets"].as_array_mut().unwrap().remove(1); // only the plain .exe
    assert_eq!(parse_latest(&payload), Err("the release has no installer".into()));
}

#[test]
fn the_installer_updates_in_place_and_restarts_the_app() {
    assert_eq!(INSTALLER_ARGS, ["/P", "/UPDATE", "/R"]);
}

#[test]
fn certutil_output_is_read() {
    let new_style = "SHA256 hash of file x:\r\nba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\r\nCertUtil: done.\r\n";
    let old_style = "SHA256 hash of x:\r\nba 78 16 bf 8f 01 cf ea 41 41 40 de 5d ae 22 23 b0 03 61 a3 96 17 7a 9c b4 10 ff 61 f2 00 15 ad\r\n";
    let abc = Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string());
    assert_eq!(certutil_hash(new_style), abc);
    assert_eq!(certutil_hash(old_style), abc);
    assert_eq!(certutil_hash("CertUtil: -hashfile command FAILED"), None);
}

#[cfg(windows)]
#[test]
fn certutil_hashes_a_file() {
    let dir = TempDir::new("sha256");
    let path = dir.0.join("abc.txt");
    std::fs::write(&path, "abc").unwrap();
    assert_eq!(sha256(&path).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

#[test]
fn update_checks_are_daily_and_only_if_allowed() {
    let now = utc(2026, 11, 18, 12, 0);
    let mut state = State::default();
    assert_eq!(update_due(&Settings::default(), &state), Some(0)); // never checked: due
    assert!(Settings::default().update_checks); // on unless switched off
    assert_eq!(update_due(&Settings { update_checks: false, ..Settings::default() }, &state), None);
    assert_eq!(update_due(&Settings { offline: true, ..Settings::default() }, &state), None);

    record_update_check(&mut state, Ok(None), now);
    assert_eq!(update_due(&Settings::default(), &state), Some(now + UPDATE_CHECK_EVERY));
    let release = parse_latest(&fixture("github_latest.json")).unwrap();
    record_update_check(&mut state, Ok(Some(release.clone())), now);
    assert_eq!(state.update, Some(release.clone()));
    record_update_check(&mut state, Err("offline".into()), now);
    assert_eq!(state.update, Some(release)); // a failed check keeps what was known...
    assert_eq!(update_due(&Settings::default(), &state), Some(now + UPDATE_RETRY)); // ...and tries again sooner
}

#[test]
fn settings_from_before_updates_still_check() {
    let old: Settings = serde_json::from_str(r#"{"offline": false, "autostart": true}"#).unwrap();
    assert!(old.update_checks);
}

/// Real download of the published installer (network): `cargo test -- --ignored real_installer`.
/// Checks the download and verification only; nothing is run.
#[test]
#[ignore]
fn real_installer_downloads_and_matches_github() {
    let dir = TempDir::new("update-download");
    let release = parse_latest(&serde_json::from_slice(&crate::net::get(crate::config::RELEASES_LATEST_URL).unwrap()).unwrap()).unwrap();
    let path = download(&release, &dir.0).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), release.size);
    assert_eq!(Some(sha256(&path).unwrap()), release.sha256);
}
