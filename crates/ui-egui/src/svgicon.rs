//! Icons drawn from the path data of a 24 by 24 stroke icon set, so a picture made as an SVG is the
//! picture on the screen: the same coordinates, the same stroke width, round caps and joins.
//!
//! Only what such icons use is read: `M L H V A Z` and their lower case forms, plus a rectangle and a
//! circle as primitives. Arcs are cut into short straight pieces; the stroke is laid down with round
//! joins by putting a disc at every vertex.

use egui::{Color32, Painter, Pos2, Stroke, pos2};

/// One drawn thing in the 24 by 24 box.
#[derive(Clone, Copy, Debug)]
pub enum Prim {
    /// SVG path data.
    Path(&'static str),
    /// A rectangle with rounded corners: x, y, width, height, radius.
    Rect(f32, f32, f32, f32, f32),
    /// A circle: centre x, centre y, radius.
    Circle(f32, f32, f32),
}

/// The stroke the set is drawn with, in units of the 24 box.
pub const STROKE: f32 = 1.75;

/// Draws `prims` with the box's top left corner at `at`, `size` points a side.
pub fn paint(painter: &Painter, at: Pos2, size: f32, prims: &[Prim], colour: Color32) {
    let k = size / 24.0;
    let width = STROKE * k;
    let stroke = Stroke::new(width, colour);
    let place = |p: (f32, f32)| pos2(at.x + p.0 * k, at.y + p.1 * k);
    for prim in prims {
        for (points, closed) in outlines(prim) {
            let points: Vec<Pos2> = points.into_iter().map(place).collect();
            if points.len() < 2 {
                continue;
            }
            painter.add(egui::Shape::line(if closed { close(&points) } else { points.clone() }, stroke));
            // Round caps and joins: a disc of half the stroke at every vertex.
            for p in &points {
                painter.circle_filled(*p, width / 2.0, colour);
            }
        }
    }
}

fn close(points: &[Pos2]) -> Vec<Pos2> {
    let mut all = points.to_vec();
    if let Some(first) = points.first() {
        all.push(*first);
    }
    all
}

/// The polylines of a primitive, in the 24 box, and whether each is closed.
fn outlines(prim: &Prim) -> Vec<(Vec<(f32, f32)>, bool)> {
    match *prim {
        Prim::Path(d) => path(d),
        Prim::Rect(x, y, w, h, r) => {
            let r = r.min(w / 2.0).min(h / 2.0);
            let mut p = Vec::new();
            for (cx, cy, from) in [(x + w - r, y + r, -90.0_f32), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)] {
                for step in 0..=6 {
                    let a = (from + 15.0 * step as f32).to_radians();
                    p.push((cx + r * a.cos(), cy + r * a.sin()));
                }
            }
            vec![(p, true)]
        }
        Prim::Circle(cx, cy, r) => {
            let p = (0..48)
                .map(|i| {
                    let a = i as f32 / 48.0 * std::f32::consts::TAU;
                    (cx + r * a.cos(), cy + r * a.sin())
                })
                .collect();
            vec![(p, true)]
        }
    }
}

struct Scanner<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Scanner<'_> {
    fn skip(&mut self) {
        while self.bytes.get(self.at).is_some_and(|b| b.is_ascii_whitespace() || *b == b',') {
            self.at += 1;
        }
    }

    fn command(&mut self) -> Option<char> {
        self.skip();
        let b = *self.bytes.get(self.at)?;
        b.is_ascii_alphabetic().then(|| {
            self.at += 1;
            b as char
        })
    }

    fn more_numbers(&mut self) -> bool {
        self.skip();
        self.bytes.get(self.at).is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.'))
    }

    fn number(&mut self) -> Option<f32> {
        self.skip();
        let start = self.at;
        if matches!(self.bytes.get(self.at), Some(b'-' | b'+')) {
            self.at += 1;
        }
        let mut dot = false;
        while let Some(b) = self.bytes.get(self.at) {
            if b.is_ascii_digit() {
                self.at += 1;
            } else if *b == b'.' && !dot {
                dot = true;
                self.at += 1;
            } else {
                break;
            }
        }
        std::str::from_utf8(&self.bytes[start..self.at]).ok()?.parse().ok()
    }

    /// An arc's flag: one character, `0` or `1`, which may run straight into the next number.
    fn flag(&mut self) -> Option<bool> {
        self.skip();
        let b = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(b == b'1')
    }
}

/// Reads path data into polylines.
pub fn path(d: &str) -> Vec<(Vec<(f32, f32)>, bool)> {
    let mut s = Scanner { bytes: d.as_bytes(), at: 0 };
    let mut out: Vec<(Vec<(f32, f32)>, bool)> = Vec::new();
    let (mut cur, mut start) = ((0.0_f32, 0.0_f32), (0.0_f32, 0.0_f32));
    let mut points: Vec<(f32, f32)> = Vec::new();
    let flush = |points: &mut Vec<(f32, f32)>, out: &mut Vec<(Vec<(f32, f32)>, bool)>, closed: bool| {
        if points.len() > 1 {
            out.push((std::mem::take(points), closed));
        } else {
            points.clear();
        }
    };
    while let Some(c) = s.command() {
        let rel = c.is_ascii_lowercase();
        match c.to_ascii_uppercase() {
            'M' => {
                flush(&mut points, &mut out, false);
                let mut first = true;
                while first || s.more_numbers() {
                    let (Some(x), Some(y)) = (s.number(), s.number()) else { break };
                    cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
                    if first {
                        start = cur;
                        points.push(cur);
                    } else {
                        points.push(cur);
                    }
                    first = false;
                }
            }
            'L' => {
                while s.more_numbers() {
                    let (Some(x), Some(y)) = (s.number(), s.number()) else { break };
                    cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
                    points.push(cur);
                }
            }
            'H' => {
                while s.more_numbers() {
                    let Some(x) = s.number() else { break };
                    cur.0 = if rel { cur.0 + x } else { x };
                    points.push(cur);
                }
            }
            'V' => {
                while s.more_numbers() {
                    let Some(y) = s.number() else { break };
                    cur.1 = if rel { cur.1 + y } else { y };
                    points.push(cur);
                }
            }
            'A' => {
                while s.more_numbers() {
                    let (Some(rx), Some(ry), Some(rot)) = (s.number(), s.number(), s.number()) else { break };
                    let (Some(large), Some(sweep)) = (s.flag(), s.flag()) else { break };
                    let (Some(x), Some(y)) = (s.number(), s.number()) else { break };
                    let to = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
                    points.extend(arc(cur, rx, ry, rot, large, sweep, to));
                    cur = to;
                }
            }
            'Z' => {
                // Closing the figure: back to where it began, and a new one may start from there.
                flush(&mut points, &mut out, true);
                cur = start;
                points.push(cur);
            }
            _ => break,
        }
    }
    flush(&mut points, &mut out, false);
    out
}

/// An SVG elliptical arc from `from` to `to`, as short straight pieces (the end point included).
fn arc(from: (f32, f32), rx: f32, ry: f32, rot: f32, large: bool, sweep: bool, to: (f32, f32)) -> Vec<(f32, f32)> {
    if rx == 0.0 || ry == 0.0 || from == to {
        return vec![to];
    }
    let phi = rot.to_radians();
    let (cos, sin) = (phi.cos(), phi.sin());
    let (dx, dy) = ((from.0 - to.0) / 2.0, (from.1 - to.1) / 2.0);
    let (x1, y1) = (cos * dx + sin * dy, -sin * dx + cos * dy);
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    let scale = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if scale > 1.0 {
        rx *= scale.sqrt();
        ry *= scale.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let (cxp, cyp) = (coef * rx * y1 / ry, -coef * ry * x1 / rx);
    let (cx, cy) = (cos * cxp - sin * cyp + (from.0 + to.0) / 2.0, sin * cxp + cos * cyp + (from.1 + to.1) / 2.0);
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 { -a } else { a }
    };
    let theta = angle(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut delta = angle((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
    if !sweep && delta > 0.0 {
        delta -= std::f32::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f32::consts::TAU;
    }
    let steps = ((delta.abs() / std::f32::consts::TAU) * 48.0).ceil().max(2.0) as usize;
    let mut out = Vec::with_capacity(steps);
    for i in 1..=steps {
        let t = theta + delta * i as f32 / steps as f32;
        let (px, py) = (rx * t.cos(), ry * t.sin());
        out.push((cos * px - sin * py + cx, sin * px + cos * py + cy));
    }
    if let Some(last) = out.last_mut() {
        *last = to;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_relative_moves_are_read() {
        let p = path("M6 15l6-6 6 6");
        assert_eq!(p, vec![(vec![(6.0, 15.0), (12.0, 9.0), (18.0, 15.0)], false)]);
    }

    #[test]
    fn several_figures_and_h_v_are_read() {
        let p = path("M4 9V4h5M20 9V4h-5");
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].0, vec![(4.0, 9.0), (4.0, 4.0), (9.0, 4.0)]);
        assert_eq!(p[1].0, vec![(20.0, 9.0), (20.0, 4.0), (15.0, 4.0)]);
    }

    #[test]
    fn arc_flags_may_run_into_the_next_number() {
        // The moon: two arcs written with the flags and the end point joined, `019.5`.
        let p = path("M20 14.5A8 8 0 019.5 4a8 8 0 1010.5 10.5z");
        assert_eq!(p.len(), 1);
        assert!(p[0].1, "closed");
        let (first, last) = (p[0].0[0], p[0].0[p[0].0.len() - 1]);
        assert_eq!(first, (20.0, 14.5));
        assert!((last.0 - 20.0).abs() < 0.01 && (last.1 - 14.5).abs() < 0.01, "ends where it began: {last:?}");
        assert!(p[0].0.iter().any(|(x, y)| (x - 9.5).abs() < 0.01 && (y - 4.0).abs() < 0.01), "passes the first arc's end");
    }
}
