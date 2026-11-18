//! Where the popup appears.
use crate::tray::{place, Area};

const WORK: Area = Area { x: 0, y: 0, w: 1920, h: 1040 }; // 1920x1080 screen, 40px taskbar at the bottom
const POPUP: (i32, i32) = (360, 540);

#[test]
fn centred_above_the_tray_icon() {
    let tray = Area { x: 1500, y: 1048, w: 24, h: 24 };
    assert_eq!(place(Some(tray), WORK, POPUP), (1500 + 12 - 180, 1040 - 540 - 12));
}

#[test]
fn kept_on_screen_near_the_right_edge() {
    let tray = Area { x: 1890, y: 1048, w: 24, h: 24 };
    assert_eq!(place(Some(tray), WORK, POPUP), (1920 - 360 - 12, 1040 - 540 - 12));
}

#[test]
fn below_a_taskbar_at_the_top() {
    let work = Area { x: 0, y: 40, w: 1920, h: 1040 };
    let tray = Area { x: 1500, y: 8, w: 24, h: 24 };
    assert_eq!(place(Some(tray), work, POPUP), (1332, 52));
}

#[test]
fn bottom_right_when_the_tray_position_is_unknown() {
    assert_eq!(place(None, WORK, POPUP), (1920 - 360 - 12, 1040 - 540 - 12));
    let second_screen = Area { x: 1920, y: 0, w: 2560, h: 1400 };
    assert_eq!(place(None, second_screen, POPUP), (1920 + 2560 - 372, 1400 - 552));
}

#[test]
fn a_tiny_work_area_still_gives_a_position() {
    let tiny = Area { x: 0, y: 0, w: 300, h: 300 };
    assert_eq!(place(None, tiny, POPUP), (12, 12));
}
