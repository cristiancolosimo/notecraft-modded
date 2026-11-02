//! Kinetic scrolling: a page or a strip pushed with a finger keeps moving after the finger
//! lifts, and slows to a stop, as it does everywhere else on a touch screen.
//!
//! [`Fling`] is the arithmetic. [`step`] ties it to a frame: call it every frame with the drag in
//! progress (if any) and scroll by what it returns.

use egui::{Context, Id, Vec2, vec2};

/// How fast the speed dies away: it is multiplied by `exp(-DECAY * seconds)`, so about a
/// third of it is left after a third of a second.
const DECAY: f32 = 3.0;
/// Under this speed (points a second) the page stops rather than creeping.
const STOP_BELOW: f32 = 40.0;
/// A fast flick is capped, so a stray flourish does not send the page across the sheet.
const MAX_SPEED: f32 = 6000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fling {
    /// Points a second, the way the content is moving.
    vel: Vec2,
    /// A finger is on the glass and moving the content.
    dragging: bool,
}

impl Fling {
    /// A finger moved the content by `delta` this frame, at `speed` points a second.
    pub fn drag(&mut self, speed: Vec2) {
        self.vel = clamp(speed);
        self.dragging = true;
    }

    /// The finger lifted, or touched down again: no more coasting unless a drag came first.
    pub fn stop(&mut self) {
        self.vel = Vec2::ZERO;
        self.dragging = false;
    }

    /// One frame with no finger moving the content: how far it coasts, and slows down.
    pub fn coast(&mut self, dt: f32) -> Vec2 {
        self.dragging = false;
        if self.vel.length() < STOP_BELOW {
            self.vel = Vec2::ZERO;
            return Vec2::ZERO;
        }
        let moved = self.vel * dt;
        self.vel *= (-DECAY * dt).exp();
        moved
    }

    pub fn moving(&self) -> bool {
        self.vel != Vec2::ZERO
    }
}

fn clamp(v: Vec2) -> Vec2 {
    let speed = v.length();
    if speed > MAX_SPEED { v * (MAX_SPEED / speed) } else { v }
}

/// Call every frame. `drag` is how far a finger moved the content this frame, or `None` when none
/// is. Returns how far to scroll this frame: the drag itself, or the coasting after it.
/// Only the axes in `axes` (1 or 0 each) ever move, so a strip that scrolls sideways ignores
/// the up and down of a finger.
pub fn step(ctx: &Context, id: Id, drag: Option<Vec2>, axes: Vec2) -> Vec2 {
    let mut fling: Fling = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    // The time the frame really took, not the smoothed estimate: the page has to move by the speed
    // times the time that went by, or it speeds up and slows down with the frame rate.
    let (dt, pressed, speed) = ctx.input(|i| (i.unstable_dt.clamp(0.0, 0.1), i.pointer.any_pressed(), i.pointer.velocity()));
    let moved = match drag {
        Some(delta) => {
            fling.drag(speed * axes);
            delta * axes
        }
        None if pressed => {
            // A new touch catches a coasting page.
            fling.stop();
            Vec2::ZERO
        }
        None => fling.coast(dt),
    };
    if fling.moving() {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, fling));
    moved
}

/// Ends any coasting at once (a palm that turned out not to be a scroll).
pub fn cancel(ctx: &Context, id: Id) {
    ctx.data_mut(|d| d.insert_temp(id, Fling::default()));
}

/// A convenience for the common case of both axes.
pub fn both() -> Vec2 {
    vec2(1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flick_keeps_moving_after_the_finger_lifts_and_then_stops() {
        let mut f = Fling::default();
        f.drag(vec2(0.0, 1200.0));
        let mut travelled = 0.0;
        let mut frames = 0;
        while f.moving() {
            travelled += f.coast(1.0 / 60.0).y;
            frames += 1;
            assert!(frames < 600, "it must come to rest");
        }
        assert!(travelled > 150.0, "it carried on: {travelled}");
        assert!((1..600).contains(&frames));
    }

    #[test]
    fn it_slows_down_and_a_slow_drag_does_not_coast_at_all() {
        let mut f = Fling::default();
        f.drag(vec2(900.0, 0.0));
        let first = f.coast(1.0 / 60.0).x;
        let second = f.coast(1.0 / 60.0).x;
        assert!(first > second && second > 0.0, "{first} then {second}");

        let mut slow = Fling::default();
        slow.drag(vec2(10.0, 0.0));
        assert_eq!(slow.coast(1.0 / 60.0), Vec2::ZERO, "a finger resting is not a flick");
    }

    #[test]
    fn a_touch_catches_it_and_a_wild_flick_is_capped() {
        let mut f = Fling::default();
        f.drag(vec2(0.0, 2000.0));
        f.stop();
        assert_eq!(f.coast(1.0 / 60.0), Vec2::ZERO);

        let mut wild = Fling::default();
        wild.drag(vec2(0.0, 90_000.0));
        assert!(wild.coast(1.0).y <= MAX_SPEED + 1.0);
    }
}
