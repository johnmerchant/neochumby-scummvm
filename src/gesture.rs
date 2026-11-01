//! Finger to mouse. In a game:
//!
//! - A touch moves the pointer. Lifted where it came down, it is a left click,
//!   sent a moment after the pointer got there: these games work out what is
//!   under the pointer once a frame or so, and a click that arrives with the
//!   move would land on what was there before.
//! - Held still, it is a right button, held down until the finger lifts: Lure's
//!   and Nippon's menus stay up for as long as the right button does, and open
//!   on it, and a release chooses.
//! - A finger that slides is only a pointer: it shows the names of things
//!   without clicking them.
//!
//! In ScummVM's GUI, which has no hover worth the wait and has scrollbars to
//! drag, the left button goes down with the finger and up with it.

use std::collections::VecDeque;

/// How long a finger stays still to be a right button.
const HOLD_MS: u64 = 600;
/// How far a resistive panel wanders under a finger that thinks it is still.
const SLOP: i32 = 8;
/// From the pointer arriving to the left button going down.
const SETTLE_MS: u64 = 120;
/// How long the left button stays down.
const CLICK_MS: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    Move(i32, i32),
    LeftDown(i32, i32),
    LeftUp(i32, i32),
    RightDown(i32, i32),
    RightUp(i32, i32),
}

#[derive(Default)]
pub struct Gesture {
    /// Where the touch came down, when, and where it is now.
    down: Option<(i32, i32, u64, i32, i32)>,
    slid: bool,
    right: bool,
    gui_left: bool,
    /// Events not to be sent before their time.
    queue: VecDeque<(u64, Out)>,
    /// The next tap is a right click (the RCLK soft button).
    pub arm_right: bool,
}

impl Gesture {
    pub fn touch(&mut self, now: u64, x: i32, y: i32, gui: bool) {
        match self.down.as_mut() {
            None => {
                self.down = Some((x, y, now, x, y));
                self.slid = false;
                self.right = false;
                self.queue.push_back((now, Out::Move(x, y)));
                if gui {
                    self.gui_left = true;
                    self.queue.push_back((now, Out::LeftDown(x, y)));
                }
            }
            Some(d) => {
                if (x, y) != (d.3, d.4) {
                    d.3 = x;
                    d.4 = y;
                    self.queue.push_back((now, Out::Move(x, y)));
                }
                if (x - d.0).abs() > SLOP || (y - d.1).abs() > SLOP {
                    self.slid = true;
                }
            }
        }
    }

    pub fn lift(&mut self, now: u64) {
        let Some((.., came, x, y)) = self.down.take() else { return };
        if self.gui_left {
            self.gui_left = false;
            self.queue.push_back((now, Out::LeftUp(x, y)));
        } else if self.right {
            self.queue.push_back((now, Out::RightUp(x, y)));
        } else if !self.slid {
            let at = now.max(came + SETTLE_MS);
            let (down, up) = if std::mem::take(&mut self.arm_right) {
                (Out::RightDown(x, y), Out::RightUp(x, y))
            } else {
                (Out::LeftDown(x, y), Out::LeftUp(x, y))
            };
            self.queue.push_back((at, down));
            self.queue.push_back((at + CLICK_MS, up));
        }
    }

    /// The next event due at `now`, in order.
    pub fn next(&mut self, now: u64) -> Option<Out> {
        if let Some((_, _, came, x, y)) = self.down
            && !self.slid
            && !self.right
            && !self.gui_left
            && now >= came + HOLD_MS
        {
            self.right = true;
            self.queue.push_back((now, Out::RightDown(x, y)));
        }
        if self.queue.front().is_some_and(|(at, _)| *at <= now) { self.queue.pop_front().map(|(_, e)| e) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(g: &mut Gesture, now: u64) -> Vec<Out> {
        std::iter::from_fn(|| g.next(now)).collect()
    }

    #[test]
    fn a_tap_moves_then_clicks_after_settling() {
        let mut g = Gesture::default();
        g.touch(1000, 50, 60, false);
        assert_eq!(drain(&mut g, 1000), [Out::Move(50, 60)]);
        g.lift(1040);
        assert_eq!(drain(&mut g, 1040), []);
        assert_eq!(drain(&mut g, 1120), [Out::LeftDown(50, 60)]);
        assert_eq!(drain(&mut g, 1180), [Out::LeftUp(50, 60)]);
    }

    #[test]
    fn a_hold_is_a_right_button_until_the_lift() {
        let mut g = Gesture::default();
        g.touch(0, 10, 10, false);
        drain(&mut g, 0);
        assert_eq!(drain(&mut g, 599), []);
        assert_eq!(drain(&mut g, 600), [Out::RightDown(10, 10)]);
        g.touch(700, 40, 40, false);
        assert_eq!(drain(&mut g, 700), [Out::Move(40, 40)]);
        g.lift(800);
        assert_eq!(drain(&mut g, 800), [Out::RightUp(40, 40)]);
        assert_eq!(drain(&mut g, 2000), []);
    }

    #[test]
    fn a_slide_only_moves() {
        let mut g = Gesture::default();
        g.touch(0, 10, 10, false);
        g.touch(50, 30, 10, false);
        g.lift(100);
        assert_eq!(drain(&mut g, 5000), [Out::Move(10, 10), Out::Move(30, 10)]);
    }

    #[test]
    fn wander_within_the_slop_is_still_a_tap() {
        let mut g = Gesture::default();
        g.touch(0, 10, 10, false);
        g.touch(50, 14, 12, false);
        g.lift(200);
        assert_eq!(
            drain(&mut g, 5000),
            [Out::Move(10, 10), Out::Move(14, 12), Out::LeftDown(14, 12), Out::LeftUp(14, 12)]
        );
    }

    #[test]
    fn the_gui_presses_with_the_finger() {
        let mut g = Gesture::default();
        g.touch(0, 5, 6, true);
        assert_eq!(drain(&mut g, 0), [Out::Move(5, 6), Out::LeftDown(5, 6)]);
        assert_eq!(drain(&mut g, 2000), []);
        g.lift(2000);
        assert_eq!(drain(&mut g, 2000), [Out::LeftUp(5, 6)]);
    }

    #[test]
    fn armed_the_next_tap_is_a_right_click() {
        let mut g = Gesture { arm_right: true, ..Gesture::default() };
        g.touch(0, 1, 2, false);
        g.lift(200);
        assert_eq!(drain(&mut g, 5000), [Out::Move(1, 2), Out::RightDown(1, 2), Out::RightUp(1, 2)]);
        assert!(!g.arm_right);
    }
}
