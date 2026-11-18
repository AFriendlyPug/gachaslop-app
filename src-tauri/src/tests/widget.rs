//! The taskbar widget's text, placement maths and drawing helpers.
use super::utc;
use crate::games::{built_in, CustomGame};
use crate::time::{fmt_clock, DAY, HOUR, MINUTE};
use crate::widget::*;

#[test]
fn clock_countdowns() {
    assert_eq!(fmt_clock(0), "00:00:00");
    assert_eq!(fmt_clock(-5), "00:00:00");
    assert_eq!(fmt_clock(2 * HOUR + 15 * MINUTE + 7), "02:15:07");
    assert_eq!(fmt_clock(DAY - 1), "23:59:59");
    assert_eq!(fmt_clock(DAY), "1d 00:00:00");
    assert_eq!(fmt_clock(5 * DAY + HOUR + 2 * MINUTE + 3), "5d 01:02:03");
}

#[test]
fn shows_the_two_soonest_games() {
    let mut games = built_in();
    let now = utc(2026, 11, 18, 16, 44) + 53; // BA resets at 19:00, ZZZ at 03:00 tomorrow
    assert_eq!(widget_lines(&games, now), [("BA".to_string(), "02:15:07".to_string()),
                                           ("ZZZ".to_string(), "10:15:07".to_string())]);
    games.push(CustomGame { id: "custom-1".into(), name: "Honkai Star Rail".into(), icon: None, hour: 17, minute: 0,
                            weekly_day: None, offset_minutes: 0 }.to_game());
    let lines = widget_lines(&games, now);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], ("HSR".to_string(), "00:15:07".to_string()));
    assert_eq!(lines[1].0, "BA");
    assert_eq!(widget_lines(&games[1..2], now).len(), 1);
    assert!(widget_lines(&[], now).is_empty());
}

#[test]
fn long_names_are_shortened() {
    assert_eq!(label("BA"), "BA");
    assert_eq!(label("NIKKE"), "NIK");
    assert_eq!(label("Honkai: Star Rail (EU)"), "HSR");
    assert_eq!(label("Arknights"), "ARK");
    assert_eq!(label("ZZZ"), "ZZZ");
}

#[test]
fn placement_maths() {
    assert_eq!(strip_left(1700, 90, 2), 1608);
    // task buttons reaching under the strip are narrowed to end where it starts
    assert_eq!(rebar_width(100, 1600, 1608), Some(1508));
    assert_eq!(rebar_width(100, 1400, 1608), None); // already clear
    assert_eq!(rebar_width(1700, 50, 1608), None);  // starts after the strip (not ours to move)
}

#[test]
fn text_follows_the_taskbar_theme() {
    assert_eq!(text_colour(false), (0xFF, 0xFF, 0xFF));
    assert_ne!(text_colour(true), (0xFF, 0xFF, 0xFF));
}

#[test]
fn white_text_becomes_coloured_with_premultiplied_alpha() {
    // BGRA: empty, full white, half grey
    let mut pixels = [0, 0, 0, 0, 255, 255, 255, 0, 128, 128, 128, 0];
    premultiply(&mut pixels, (255, 0, 0), [0, 0, 0, 0]); // red text on nothing
    assert_eq!(pixels, [0, 0, 0, 0, 0, 0, 255, 255, 0, 0, 128, 128]);
    let mut dark = [255, 255, 255, 0];
    premultiply(&mut dark, (0x1C, 0x1C, 0x1C), background(true, false));
    assert_eq!(dark, [0x1C, 0x1C, 0x1C, 255]);
}

#[test]
fn the_whole_strip_catches_clicks() {
    // Windows passes clicks on alpha-0 pixels through to the taskbar, so no pixel may be fully clear
    for (light, hovered) in [(false, false), (true, false), (false, true), (true, true)] {
        let bg = background(light, hovered);
        assert!(bg[3] >= 1, "{light} {hovered}");
        assert!(bg[..3].iter().all(|c| *c <= bg[3]), "premultiplied: colour can't exceed alpha");
        let mut empty = [0, 0, 0, 0];
        premultiply(&mut empty, text_colour(light), bg);
        assert_eq!(empty, bg);
    }
    assert_eq!(background(false, false), [0, 0, 0, 1]); // invisible when not hovered
    assert!(background(false, true)[3] > 1);             // a visible wash on hover
}

#[test]
fn text_over_the_hover_wash_blends() {
    let mut half = [128, 128, 128, 0];
    premultiply(&mut half, (255, 255, 255), background(false, true)); // white text, white wash
    assert_eq!(half[3], 140); // 128 + 26 * 127 / 255
    assert!(half[0] <= half[3]);
}

#[test]
fn lines_are_spaced_like_the_clock() {
    // Windows 10's 40px taskbar at 100%: Segoe UI 12px lines are 15px tall; the clock's time and date
    // digits start at rows 6 and 24, which is where these lines put them (digits sit 3px into the line)
    assert_eq!(line_tops(40, 15, 2, CLOCK_LINE_PITCH), [3, 21]);
    // 150% scale: everything scales
    assert_eq!(line_tops(60, 23, 2, 27), [5, 32]);
    // a shorter taskbar: as far apart as fits, never overlapping
    assert_eq!(line_tops(30, 15, 2, CLOCK_LINE_PITCH), [0, 15]);
    // one game: centred
    assert_eq!(line_tops(40, 15, 1, CLOCK_LINE_PITCH), [12]);
}
