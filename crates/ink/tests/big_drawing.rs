//! How long a large drawing takes to save, load and measure. Run with
//! `cargo test --release -p notecraft-ink --test big_drawing -- --ignored --nocapture`.

use notecraft_ink::{Dab, Drawing, Rgb, Stroke, Tool, shape, svg};
use std::time::Instant;

fn big(strokes: usize, dabs: usize) -> Drawing {
    let mut drawing = Drawing::new(820.0, 1100.0);
    for s in 0..strokes {
        let mut stroke = Stroke::new(Tool::Pen, Rgb(0x20, 0x20, 0x20), 3.0);
        let (ox, oy) = ((s % 60) as f32 * 40.0, (s / 60) as f32 * 30.0);
        for i in 0..dabs {
            let t = i as f32 / dabs as f32;
            stroke.add(Dab::new(ox + t * 120.0, oy + (t * 9.0).sin() * 14.0, 0.3 + 0.7 * t));
        }
        drawing.add(stroke);
    }
    drawing
}

#[test]
#[ignore = "a measurement, not a check"]
fn how_long_a_big_drawing_takes() {
    let drawing = big(250, 100);
    let t = Instant::now();
    let text = svg::write(&drawing, svg::Flavour::Editable);
    println!("write {:>7.1} ms  ({:.1} MB)", t.elapsed().as_secs_f64() * 1e3, text.len() as f64 / 1e6);
    let t = Instant::now();
    let (back, _) = svg::read(&text).expect("read");
    println!("read  {:>7.1} ms  ({} strokes)", t.elapsed().as_secs_f64() * 1e3, back.strokes.len());
    let one = &drawing.strokes[drawing.strokes.len() - 1];
    let t = Instant::now();
    let line = svg::path_line(one, svg::Flavour::Editable).expect("line");
    println!("one stroke's journal line {:>7.2} ms ({} bytes), instead of the whole file", t.elapsed().as_secs_f64() * 1e3, line.len());
    let t = Instant::now();
    let mut boxes = 0;
    for s in &drawing.strokes {
        boxes += usize::from(s.bounds().is_some());
    }
    println!("bounds of every stroke {:>7.2} ms ({boxes})", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let mut points = 0;
    for s in &drawing.strokes {
        points += shape::sides(s).0.len();
    }
    println!("outline of every stroke {:>7.1} ms ({points} points)", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let clones: usize = drawing.strokes.iter().map(|s| s.clone().dabs.len()).sum();
    println!("clone of every stroke {:>7.1} ms ({clones})", t.elapsed().as_secs_f64() * 1e3);
}
