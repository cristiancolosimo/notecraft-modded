//! The working file of a page's ink: strokes as compact binary records, appended as they are
//! finished.
//!
//! An SVG of a drawing is a picture of it with the outline of every stroke worked out, and that is
//! slow to make and large to write (about 160 bytes a reading). A stroke that has just been
//! finished does not need to be a picture yet; it needs to be safe. So it is appended here as
//! what the pen gave (about 12 bytes a reading, nothing worked out), and the SVG is made from
//! them later, in the background, when there are enough or the pen has rested.
//!
//! Layout: the bytes `NCJ1` and the number of the first entry (u64), then records, each a u32
//! length, a u32 checksum of the payload and the payload: the tool's name (u8 length and bytes), the
//! colour (3 bytes), width and gain (f32 each) and the readings (u32 count, then x, y, pressure as
//! f32). Everything is little endian. A record that is cut short or fails its checksum, as the
//! last one may after a crash, ends the file: it and what follows are not strokes.

use crate::{Dab, MAX_DABS, Rgb, Stroke, Tool};

const MAGIC: &[u8; 4] = b"NCJ1";
/// The bytes before the first record.
pub const HEADER: usize = 12;

/// The start of a journal whose first entry is number `base`.
pub fn header(base: usize) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&(base as u64).to_le_bytes());
    out
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5_u32, |h, b| (h ^ u32::from(*b)).wrapping_mul(0x0100_0193))
}

/// One stroke as a record, or `None` for a stroke with no readings.
pub fn encode(stroke: &Stroke) -> Option<Vec<u8>> {
    if stroke.dabs.is_empty() {
        return None;
    }
    let tool = stroke.tool.id().as_bytes();
    let mut payload = Vec::with_capacity(24 + tool.len() + stroke.dabs.len() * 12);
    payload.push(u8::try_from(tool.len()).unwrap_or(0));
    payload.extend_from_slice(tool);
    payload.extend_from_slice(&[stroke.colour.0, stroke.colour.1, stroke.colour.2]);
    payload.extend_from_slice(&stroke.width.to_le_bytes());
    payload.extend_from_slice(&stroke.gain.to_le_bytes());
    payload.extend_from_slice(&(stroke.dabs.len() as u32).to_le_bytes());
    for d in &stroke.dabs {
        for v in [d.x, d.y, d.pressure] {
            payload.extend_from_slice(&v.to_le_bytes());
        }
    }
    let mut record = Vec::with_capacity(payload.len() + 8);
    record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    record.extend_from_slice(&checksum(&payload).to_le_bytes());
    record.extend_from_slice(&payload);
    Some(record)
}

/// What a journal holds.
#[derive(Debug, PartialEq)]
pub struct Journal {
    /// The number of its first entry.
    pub base: usize,
    pub strokes: Vec<Stroke>,
    /// How many bytes are whole records. Anything after is a torn end that can be cut off.
    pub valid: usize,
}

/// Reads a journal. `None` when the bytes are not one (no header).
pub fn decode(bytes: &[u8]) -> Option<Journal> {
    if bytes.len() < HEADER || &bytes[..4] != MAGIC {
        return None;
    }
    let base = u64::from_le_bytes(bytes[4..HEADER].try_into().ok()?) as usize;
    let mut at = HEADER;
    let mut strokes = Vec::new();
    while let Some(head) = bytes.get(at..at + 8) {
        let len = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
        let sum = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        let Some(payload) = bytes.get(at + 8..at + 8 + len) else { break };
        if checksum(payload) != sum {
            break;
        }
        match parse(payload) {
            Some(stroke) => strokes.push(stroke),
            None => break,
        }
        at += 8 + len;
    }
    Some(Journal { base, strokes, valid: at })
}

fn parse(p: &[u8]) -> Option<Stroke> {
    let name_len = usize::from(*p.first()?);
    let name = std::str::from_utf8(p.get(1..1 + name_len)?).ok()?;
    let mut at = 1 + name_len;
    let colour = Rgb(*p.get(at)?, *p.get(at + 1)?, *p.get(at + 2)?);
    at += 3;
    let f32_at = |at: usize| p.get(at..at + 4).and_then(|b| b.try_into().ok()).map(f32::from_le_bytes);
    let width = f32_at(at)?;
    let gain = f32_at(at + 4)?;
    let count = u32::from_le_bytes(p.get(at + 8..at + 12)?.try_into().ok()?) as usize;
    at += 12;
    if count > MAX_DABS || p.len() != at + count * 12 {
        return None;
    }
    let dabs = (0..count)
        .map(|i| Dab::new(f32_at(at + i * 12).unwrap_or(0.0), f32_at(at + i * 12 + 4).unwrap_or(0.0), f32_at(at + i * 12 + 8).unwrap_or(1.0)))
        .collect();
    Some(Stroke { tool: Tool::parse(name).unwrap_or_default(), colour, width, gain, dabs }.sane())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(n: usize) -> Stroke {
        let mut s = Stroke::new(Tool::Highlighter, Rgb(1, 2, 3), 7.5);
        s.gain = 1.5;
        for i in 0..n {
            s.add(Dab::new(i as f32 * 2.5, -(i as f32), 0.25 + i as f32 * 0.01));
        }
        s
    }

    #[test]
    fn strokes_come_back_from_the_journal_as_they_went_in() {
        let mut bytes = header(5);
        for n in [3, 40, 1] {
            bytes.extend(encode(&stroke(n)).expect("record"));
        }
        let got = decode(&bytes).expect("a journal");
        assert_eq!(got.base, 5);
        assert_eq!(got.valid, bytes.len());
        assert_eq!(got.strokes.len(), 3);
        assert_eq!(got.strokes[1].dabs.len(), 40);
        assert_eq!(got.strokes[0].tool, Tool::Highlighter);
        assert_eq!(got.strokes[0].colour, Rgb(1, 2, 3));
        assert_eq!(got.strokes[0].width, 7.5);
        assert_eq!(got.strokes[0].gain, 1.5);
        assert_eq!(got.strokes[1].dabs[39], stroke(40).dabs[39]);
    }

    #[test]
    fn a_record_cut_short_or_damaged_ends_the_journal_and_nothing_after_it_counts() {
        let mut bytes = header(0);
        bytes.extend(encode(&stroke(10)).expect("record"));
        let whole = bytes.len();
        bytes.extend(encode(&stroke(10)).expect("record"));
        // Cut in the middle of the second record, as a crash would.
        let torn = &bytes[..bytes.len() - 20];
        let got = decode(torn).expect("a journal");
        assert_eq!((got.strokes.len(), got.valid), (1, whole), "the whole record stays, the torn one goes");
        // A damaged byte fails the checksum.
        let mut bad = bytes.clone();
        bad[whole + 20] ^= 0xff;
        let got = decode(&bad).expect("a journal");
        assert_eq!(got.strokes.len(), 1);
        // And a record that is itself good but follows a bad one is not trusted.
        let mut bad_first = bytes.clone();
        bad_first[HEADER + 20] ^= 0xff;
        assert_eq!(decode(&bad_first).expect("a journal").strokes.len(), 0);
        assert!(decode(b"not a journal").is_none());
        assert!(encode(&Stroke::new(Tool::Pen, Rgb::BLACK, 3.0)).is_none(), "no readings, no record");
    }

    #[test]
    fn a_record_is_a_small_fraction_of_the_svg_of_the_same_stroke() {
        let s = stroke(100);
        let record = encode(&s).expect("record").len();
        let line = crate::svg::path_line(&s, crate::svg::Flavour::Editable).expect("line").len();
        assert!(record * 8 < line, "{record} bytes against {line}");
    }
}
