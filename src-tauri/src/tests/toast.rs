//! Notification cards: merging, how long they stay, where they go, and the shape maths.
use std::time::Duration;

use crate::toast::*;
use crate::tray::Area;

fn card(game: &str, title: &str) -> Card {
    Card::new(Some(game), title, "")
}

#[test]
fn one_card_is_shown_as_it_is() {
    let one = Card::new(Some("ba"), "Blue Archive (Global) has reset", "Next reset in 1d 0h.");
    assert_eq!(merge(std::slice::from_ref(&one)), Some(one));
    assert_eq!(merge(&[]), None);
}

#[test]
fn cards_that_waited_together_become_one_list() {
    let merged = merge(&[card("ba", "BA reset"), card("zzz", "ZZZ reset")]).unwrap();
    assert_eq!(merged, Card::new(None, "2 notifications", "BA reset\nZZZ reset"));
    let same_game = merge(&[card("ba", "BA reset"), card("ba", "New BA banner")]).unwrap();
    assert_eq!(same_game.game.as_deref(), Some("ba")); // keeps the game's icon
}

#[test]
fn a_long_list_is_cut_short() {
    let cards: Vec<Card> = (1..=6).map(|i| card("ba", &format!("Item {i}"))).collect();
    let merged = merge(&cards).unwrap();
    assert_eq!(merged.title, "6 notifications");
    assert_eq!(merged.body, "Item 1\nItem 2\nItem 3\nand 3 more");
}

#[test]
fn longer_cards_stay_longer_up_to_a_limit() {
    let short = display_time(&Card::new(None, "Reset", "One line."));
    let long = display_time(&Card::new(None, "Several", "a\nb\nc"));
    assert_eq!(short, Duration::from_millis(7500));
    assert!(long > short);
    assert_eq!(display_time(&Card::new(None, "Many", "x\n".repeat(20))), Duration::from_secs(12));
}

const WORK: Area = Area { x: 0, y: 0, w: 1920, h: 1040 };

#[test]
fn a_card_sits_beside_the_open_popup() {
    let popup = Area { x: 1500, y: 500, w: 400, h: 528 };
    // to the left, bottoms lined up
    assert_eq!(position(Some(popup), None, WORK, (390, 120)), (1110, 908));
    // no room on the left (popup at the screen's left edge): to its right
    let left_edge = Area { x: 10, ..popup };
    assert_eq!(position(Some(left_edge), None, WORK, (390, 120)), (410, 908));
}

#[test]
fn without_the_popup_a_card_sits_above_the_widget() {
    let widget = Area { x: 1700, y: 1040, w: 80, h: 40 };
    let (x, y) = position(None, Some(widget), WORK, (390, 120));
    assert_eq!(y, 1040 - 120 - 12); // above the taskbar, like the popup
    assert!(x + 390 <= 1920 - 12);
    assert_eq!(position(None, None, WORK, (390, 120)), (1920 - 390 - 12, 1040 - 120 - 12));
}

#[test]
fn rounded_corners_are_smooth() {
    let (w, h, r) = (100.0, 50.0, 10.0);
    assert_eq!(coverage(50.5, 25.5, w, h, r), 1.0); // middle
    assert_eq!(coverage(-3.0, 25.0, w, h, r), 0.0); // outside
    assert_eq!(coverage(0.5, 0.5, w, h, r), 0.0); // the cut-off corner
    let edge = coverage(3.0, 3.0, w, h, r); // on the curve: partly covered
    assert!(edge > 0.0 && edge < 1.0, "{edge}");
    assert!(rounded_distance(50.0, 25.0, w, h, r) < 0.0);
}

#[test]
fn colours_and_badge_text() {
    assert_eq!(hex("#3498db"), Some((0x34, 0x98, 0xDB)));
    assert_eq!(hex("3498db"), None);
    assert_eq!(hex("#zz0000"), None);
    assert_eq!(text_on((0xF1, 0xC4, 0x0F)), (0x17, 0x16, 0x1C)); // ZZZ yellow: dark letters
    assert_eq!(text_on((0x34, 0x49, 0x5E)), (0xFF, 0xFF, 0xFF));
}

#[test]
fn cards_wait_while_a_full_screen_app_is_in_front() {
    for busy in [1, 2, 3, 4] {
        assert!(is_busy(busy), "{busy}");
    }
    for free in [5, 6, 7] {
        assert!(!is_busy(free), "{free}");
    }
}

#[test]
fn a_summary_lists_one_item_a_line() {
    assert_eq!(list(&["BA daily reset", "ZZZ daily reset"]), "BA daily reset\nZZZ daily reset");
    assert_eq!(list(&["a", "b", "c", "d"]), "a\nb\nc\nd");
    assert_eq!(list(&["a", "b", "c", "d", "e"]), "a\nb\nc\nand 2 more");
}
