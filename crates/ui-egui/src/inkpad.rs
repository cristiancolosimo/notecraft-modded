//! Ink on the page: the layer a pen writes on, over the words and between them.
//!
//! A page is a folder, so its ink is a file in that folder: `ink.svg`, beside `index.md` or
//! `index.html`. Nothing in either page format had to change to allow it, and the ink
//! travels, syncs and backs up with the page because it is simply another file in the same
//! place. A page with no `ink.svg` has never been drawn on.
//!
//! The file is ordinary SVG: filled outlines any browser renders, because SVG has no
//! variable-width stroke. It also carries the pen's own readings in our namespace, which is
//! what lets a stroke be rubbed out or redrawn later (`notecraft_ink::svg`).
//!
//! The pen writes anywhere on the page, as OneNote's does, rather than inside a box. The
//! strokes are kept in the page body's own coordinates, measured from its top left corner,
//! so they stay beside the words they were written against as the window is resized.

use crate::theme::Tokens;
use egui::{Sense, Ui, vec2};
use notecraft_ink::{Dab, Drawing, Rgb, Stroke, Tool, journal, pick, shape, svg};
use std::path::{Path, PathBuf};

/// What the pen is set to do, as the Draw tab has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    pub pointer: crate::ribbon::Pointer,
    pub tool: Tool,
    pub colour: Rgb,
    /// In points.
    pub width: f32,
    /// How readily this pen answers pressure; 1 leaves the readings as they came.
    pub gain: f32,
}

impl Default for Pen {
    fn default() -> Self {
        Self { pointer: crate::ribbon::Pointer::Write, tool: Tool::Pen, colour: Rgb::BLACK, width: 3.0, gain: 1.0 }
    }
}

/// The file a page's ink lives in, inside the page's own folder.
pub const FILE: &str = "ink.svg";

/// Whether an image block is the page's own ink, which the page names so that its words
/// and what is on it agree, and so another program opening the page sees the handwriting.
/// NoteCraft itself does not draw it as a picture: it draws the layer.
pub fn is_page_ink(src: &str) -> bool {
    src.trim() == FILE
}

/// Whether an image line in the page is one that names ink.
///
/// Said of a line whose file may be gone, so the file itself cannot be asked: a picture
/// beside the page file (not in `assets/`), an SVG, with `Ink` for its words. That is the
/// line NoteCraft writes, and no line a person would write by hand for a picture of
/// their own, which lives in `assets/` and says what it is a picture of.
pub fn is_ink_line(src: &str, alt: &str) -> bool {
    let src = src.trim();
    !src.contains('/')
        && !src.contains('\\')
        && !src.contains("://")
        && src.to_ascii_lowercase().ends_with(".svg")
        && alt.trim().eq_ignore_ascii_case("Ink")
}

/// An earlier version of NoteCraft put a drawing into the page as a picture, pointing at
/// `assets/drawing-1.svg`. Nothing draws those now, so such a page showed a broken picture
/// where its drawing had been. This says whether an image block is one of them.
pub fn is_old_sheet(src: &str) -> bool {
    let src = src.trim();
    !src.contains("://") && src.to_ascii_lowercase().starts_with("assets/drawing-") && src.to_ascii_lowercase().ends_with(".svg")
}

/// How large a page's ink sheet is said to be in the file's `viewBox`. The pen may write
/// past it; the number is only what another program uses to frame the picture.
pub const SHEET_W: f32 = 820.0;
pub const SHEET_H: f32 = 1100.0;

/// Whether a stroke can be on the screen: its box (the width of the line included) meets `visible`,
/// in the page's own coordinates. A stroke with no readings is never drawn.
pub fn may_show(stroke: &Stroke, visible: egui::Rect) -> bool {
    stroke.bounds().is_some_and(|(x0, y0, x1, y1)| x1 >= visible.min.x && x0 <= visible.max.x && y1 >= visible.min.y && y0 <= visible.max.y)
}

/// The pen's points in this frame's events, oldest first, each with its pressure. A touch with a
/// force is a pen (the window layer sends the in-between samples of a frame as touches of their
/// own); with none, the mouse's moves are used, at `fallback` pressure. Two samples in the same
/// place are one.
pub fn pen_samples(events: &[egui::Event], fallback: f32) -> Vec<(egui::Pos2, f32)> {
    let mut pen: Vec<(egui::Pos2, f32)> = events
        .iter()
        .filter_map(|e| match e {
            egui::Event::Touch { pos, force: Some(f), phase: egui::TouchPhase::Start | egui::TouchPhase::Move, .. } if f.is_finite() => {
                Some((*pos, f.clamp(0.0, 1.0)))
            }
            _ => None,
        })
        .collect();
    if pen.is_empty() {
        pen = events
            .iter()
            .filter_map(|e| match e {
                egui::Event::PointerMoved(pos) => Some((*pos, fallback)),
                _ => None,
            })
            .collect();
    }
    pen.dedup_by(|a, b| a.0 == b.0);
    pen
}

/// The journal beside `ink.svg`: the strokes finished since the file was last written out, as compact
/// binary records (see `notecraft_ink::journal`).
const JOURNAL: &str = "ink.journal";
/// Where an `ink.svg` that cannot be read is set aside, so it is not written over.
const UNREADABLE: &str = "ink.unreadable.svg";

/// What a background write leaves for the page to find: how much of the journal it folded in, or why
/// it failed.
type Writing = std::sync::Arc<std::sync::Mutex<Option<Result<usize, String>>>>;

/// A number that changes when any stroke does (how many readings, where the first and last are,
/// what it is drawn with), cheap enough to take on every save. It is how a stroke rubbed out, moved
/// or stretched is told from one added at the end.
fn fingerprint(strokes: &[Stroke]) -> u64 {
    strokes.iter().fold(0xcbf2_9ce4_8422_2325, |h, s| (h ^ stroke_fingerprint(s)).wrapping_mul(0x0000_0100_0000_01b3))
}

/// The same for one stroke.
fn stroke_fingerprint(s: &Stroke) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(s.dabs.len() as u64);
    mix(u64::from(s.width.to_bits()) << 32 | u64::from(s.colour.0) << 16 | u64::from(s.colour.1) << 8 | u64::from(s.colour.2));
    for d in [s.dabs.first(), s.dabs.last()].into_iter().flatten() {
        mix(u64::from(d.x.to_bits()) << 32 | u64::from(d.y.to_bits()));
    }
    h
}

/// How close to the right edge of the column ink has to come before the page grows sideways.
const EDGE: f32 = 120.0;

/// How large the ink canvas is, in points: `far` is how far the ink reaches (already scaled
/// by the zoom), `used` how tall the words are, `view` the visible area and `column` the width
/// of the page's column. The page is as wide as its column until ink comes within reach of
/// the right edge, and from then on it is half a screen wider than the furthest ink, so
/// there is always room to keep writing. Down, it is always at least the sheet and the words
/// and half a screen past the lowest ink. There is no limit either way.
pub fn canvas(far: (f32, f32), used: f32, view: egui::Vec2, column: f32) -> egui::Vec2 {
    let width = if far.0 > column - EDGE { far.0 + view.x * 0.5 } else { column };
    let height = SHEET_H.max(used + 240.0).max(far.1 + view.y * 0.5);
    vec2(width.max(column), height)
}

/// The ink of one page.
#[derive(Clone, Debug)]
pub struct Pad {
    /// Where the file is, whether or not it has been written yet.
    pub file: PathBuf,
    pub drawing: Drawing,
    /// Changed since it was last written.
    pub dirty: bool,
    /// How many strokes in the file were a picture only, with no readings to give back.
    pub flat: usize,
    /// The stroke being drawn now, if the pen is down.
    drawing_now: Option<Stroke>,
    /// The strokes being held, by their place in the drawing. Never written to the file:
    /// it is what someone has their hand on at this moment, not part of the page.
    held: Vec<usize>,
    /// The loop being drawn now by the lasso.
    loop_now: Vec<(f32, f32)>,
    /// Where a shape's drag began, which is one of the two corners it is drawn between.
    shape_from: Option<(f32, f32)>,
    /// Strokes taken back, newest last, which Redo can put back; valid only while the drawing still
    /// has `redo_len` strokes, that is, while nothing has been drawn or rubbed out since.
    redone: Vec<Stroke>,
    redo_len: usize,
    /// How the ink is kept on disk (see [`Pad::save`]): `ink.svg` plus a journal of strokes finished since
    /// it was last written out. Entries are numbered for as long as the page lives; the ones below
    /// `folded` are already in the file. `persisted` strokes of the drawing, whose fingerprint is `fp`,
    /// are on disk (in the file or the journal); `writing` is a full write going on in the background.
    next_entry: usize,
    folded: usize,
    persisted: usize,
    fp: u64,
    rewrite: bool,
    journal_bytes: usize,
    last_change: Option<web_time::Instant>,
    writing: Option<Writing>,
    /// The shapes of finished strokes already worked out, by place in the drawing, at the zoom they were made
    /// for; each says which stroke it is for, so a stroke that changed or moved is made again.
    shapes: std::collections::HashMap<usize, (u64, Vec<egui::Shape>)>,
    shapes_zoom: u32,
    /// Where the pointer was when a move or a stretch began, and the box as it was then.
    grabbed: Option<(egui::Pos2, (f32, f32, f32, f32), Grab)>,
}

/// What a press on held ink began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Grab {
    /// Moving the whole of it.
    Move,
    /// Stretching it by the corner opposite this one.
    Corner(bool, bool),
}

impl Pad {
    /// The ink of the page in `page_dir`: what is in its `ink.svg` and in its journal, or an empty
    /// layer when it has neither. Every page has a layer; the file appears the first time the page
    /// is drawn on, so a page nobody has drawn on stays as clean as it was.
    pub fn of_page(page_dir: &Path) -> Pad {
        let file = page_dir.join(FILE);
        let (mut drawing, flat, folded) = match std::fs::read_to_string(&file) {
            Ok(text) if text.trim().is_empty() => (Drawing::new(SHEET_W, SHEET_H), 0, 0),
            Ok(text) => match svg::read(&text) {
                Ok((drawing, flat)) => (drawing, flat, svg::folded(&text)),
                Err(e) => {
                    // A file that cannot be read must not be written over by the next stroke: it is
                    // set aside where the owner can find it.
                    log::error!("the ink of {} cannot be read ({e}); keeping it as {UNREADABLE}", file.display());
                    let _ = std::fs::rename(&file, file.with_file_name(UNREADABLE));
                    (Drawing::new(SHEET_W, SHEET_H), 0, 0)
                }
            },
            Err(_) => (Drawing::new(SHEET_W, SHEET_H), 0, 0),
        };
        // The journal: strokes finished since the file was last written out, in the order they were
        // finished. Entries the file already holds are not added twice, and a record torn by a
        // crash is cut off so the next stroke is not appended after it.
        let mut next_entry = folded;
        let mut journal_bytes = 0;
        let journal_file = page_dir.join(JOURNAL);
        if let Ok(bytes) = std::fs::read(&journal_file)
            && let Some(journal) = journal::decode(&bytes)
        {
            if journal.valid < bytes.len()
                && let Ok(f) = std::fs::OpenOptions::new().write(true).open(&journal_file)
            {
                let _ = f.set_len(journal.valid as u64);
            }
            journal_bytes = journal.valid;
            next_entry = (journal.base + journal.strokes.len()).max(folded);
            let skip = folded.saturating_sub(journal.base).min(journal.strokes.len());
            for stroke in journal.strokes.into_iter().skip(skip) {
                drawing.add(stroke);
            }
        }
        let persisted = drawing.strokes.len();
        let fp = fingerprint(&drawing.strokes);
        Pad {
            file,
            drawing,
            dirty: false,
            flat,
            drawing_now: None,
            held: Vec::new(),
            loop_now: Vec::new(),
            shape_from: None,
            redone: Vec::new(),
            redo_len: 0,
            next_entry,
            folded,
            persisted,
            fp,
            rewrite: false,
            journal_bytes,
            last_change: None,
            writing: None,
            shapes: std::collections::HashMap::new(),
            shapes_zoom: 0,
            grabbed: None,
        }
    }

    /// How far right and down the ink reaches, the stroke being drawn included (0, 0 when empty).
    pub fn far(&self) -> (f32, f32) {
        let mut far = self.drawing.bounds().map(|b| (b.2.max(0.0), b.3.max(0.0))).unwrap_or((0.0, 0.0));
        if let Some((_, _, x, y)) = self.drawing_now.as_ref().and_then(Stroke::bounds) {
            far = (far.0.max(x), far.1.max(y));
        }
        far
    }

    fn journal_file(&self) -> PathBuf {
        self.file.with_file_name(JOURNAL)
    }

    /// Makes what is on the page safe on disk, as cheaply as it can be done: a stroke finished at the
    /// end of the drawing is one line added to the page's journal, so a page with thousands of strokes
    /// costs no more to save than a page with one. Anything else (a stroke rubbed out, moved,
    /// taken back) changes what came before, and is written as a whole in the background.
    /// The first save of a page writes `ink.svg` itself, at once; a page with no ink loses its files.
    pub fn save(&mut self) -> std::io::Result<()> {
        // The sheet the file declares is at least as big as the ink, which may run on past the
        // usual page.
        let far = self.far();
        self.drawing.width = SHEET_W.max(far.0.ceil());
        self.drawing.height = SHEET_H.max(far.1.ceil());
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // A page drawn on and then rubbed clean loses its files rather than keeping empty ones, so
        // a page with no ink has no ink in its folder either.
        if self.drawing.is_empty() {
            for file in [self.file.clone(), self.journal_file()] {
                match std::fs::remove_file(&file) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
            (self.next_entry, self.folded, self.persisted, self.journal_bytes, self.rewrite) = (0, 0, 0, 0, false);
            self.fp = fingerprint(&self.drawing.strokes);
            self.dirty = false;
            return Ok(());
        }
        self.last_change = Some(web_time::Instant::now());
        if !self.file.exists() {
            self.write_whole_now()?;
        } else {
            let len = self.drawing.strokes.len();
            let kept = self.persisted.min(len);
            if self.persisted > len || fingerprint(&self.drawing.strokes[..kept]) != self.fp {
                self.rewrite = true;
            }
            if self.rewrite {
                self.start_whole_write();
            } else {
                self.append_new()?;
            }
        }
        self.dirty = false;
        Ok(())
    }

    /// The whole drawing into `ink.svg`, now. Everything in the journal is in it, so the journal goes.
    fn write_whole_now(&mut self) -> std::io::Result<()> {
        std::fs::write(&self.file, svg::write_folded(&self.drawing, svg::Flavour::Editable, self.next_entry))?;
        self.folded = self.next_entry;
        self.persisted = self.drawing.strokes.len();
        self.fp = fingerprint(&self.drawing.strokes);
        self.rewrite = false;
        self.drop_journal();
        Ok(())
    }

    fn drop_journal(&mut self) {
        let _ = std::fs::remove_file(self.journal_file());
        self.journal_bytes = 0;
    }

    /// The strokes finished since the last save, as lines at the end of the journal.
    fn append_new(&mut self) -> std::io::Result<()> {
        use std::io::Write;
        let fresh = &self.drawing.strokes[self.persisted.min(self.drawing.strokes.len())..];
        if fresh.is_empty() {
            return Ok(());
        }
        let records: Vec<Vec<u8>> = fresh.iter().filter_map(journal::encode).collect();
        let path = self.journal_file();
        let mut out = Vec::new();
        if std::fs::metadata(&path).map_or(true, |m| m.len() < journal::HEADER as u64) {
            out.extend(journal::header(self.next_entry));
        }
        for record in &records {
            out.extend_from_slice(record);
        }
        std::fs::OpenOptions::new().create(true).append(true).open(&path)?.write_all(&out)?;
        self.journal_bytes += out.len();
        self.next_entry += records.len();
        self.persisted = self.drawing.strokes.len();
        self.fp = fingerprint(&self.drawing.strokes);
        Ok(())
    }

    /// Writes the whole drawing to `ink.svg` on another thread, so a big page does not stall the one
    /// that is drawing. The copy written says how much of the journal it holds; more strokes may be
    /// added to the journal while it is written, and they are kept. Only one write at a time.
    fn start_whole_write(&mut self) {
        if self.writing.is_some() {
            return;
        }
        let (drawing, folded, file) = (self.drawing.clone(), self.next_entry, self.file.clone());
        self.persisted = drawing.strokes.len();
        self.fp = fingerprint(&drawing.strokes);
        self.rewrite = false;
        let slot: Writing = std::sync::Arc::new(std::sync::Mutex::new(None));
        let done = slot.clone();
        let work = move || {
            let tmp = file.with_extension("svg.tmp");
            let result = std::fs::write(&tmp, svg::write_folded(&drawing, svg::Flavour::Editable, folded))
                .and_then(|()| std::fs::rename(&tmp, &file))
                .map(|()| folded)
                .map_err(|e| e.to_string());
            if let Ok(mut slot) = done.lock() {
                *slot = Some(result);
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(work);
        #[cfg(target_arch = "wasm32")]
        work();
        self.writing = Some(slot);
    }

    /// Housekeeping, once a frame: takes in a finished background write, and starts the next one when
    /// the journal has grown (forty strokes, or two megabytes) or the pen has been still for three
    /// seconds. Returns how long until it next has something to do, so the window can wake for it.
    pub fn maintain(&mut self) -> Option<std::time::Duration> {
        use std::time::Duration;
        if let Some(slot) = &self.writing {
            let finished = slot.lock().ok().and_then(|mut s| s.take());
            match finished {
                None => return Some(Duration::from_millis(150)),
                Some(Ok(folded)) => {
                    self.writing = None;
                    self.folded = self.folded.max(folded);
                    if self.next_entry == self.folded {
                        self.drop_journal();
                    }
                }
                Some(Err(e)) => {
                    self.writing = None;
                    log::warn!("the ink of {} could not be written out: {e}", self.file.display());
                    self.rewrite = true;
                }
            }
        }
        if self.rewrite {
            self.start_whole_write();
            return Some(Duration::from_millis(150));
        }
        let unfolded = self.next_entry.saturating_sub(self.folded);
        if unfolded == 0 {
            return None;
        }
        let idle = self.last_change.map_or(Duration::MAX, |t| t.elapsed());
        const QUIET: Duration = Duration::from_secs(3);
        if unfolded >= 40 || self.journal_bytes >= 2_000_000 || idle >= QUIET {
            self.start_whole_write();
            return Some(Duration::from_millis(150));
        }
        Some(QUIET - idle)
    }

    /// Starts writing out whatever the journal holds, for when the page is about to be left.
    pub fn flush(&mut self) {
        if self.next_entry > self.folded || self.rewrite {
            self.start_whole_write();
        }
    }

    /// Whether a background write of the whole drawing is going on (for tests and the status).
    pub fn writing(&self) -> bool {
        self.writing.is_some()
    }

    /// Writes it again as a plain picture, beside the ink, for sending somewhere else.
    /// Returns where it was written.
    pub fn save_plain(&self) -> std::io::Result<PathBuf> {
        let plain = self.file.with_file_name("ink-picture.svg");
        if let Some(parent) = plain.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&plain, svg::write(&self.drawing, svg::Flavour::Plain))?;
        Ok(plain)
    }

    /// Moves the strokes of an old inserted sheet onto the page, and says how many came.
    /// The sheet's own file is left where it is: it is the owner's, and nothing here
    /// throws away a file it has only read.
    pub fn absorb(&mut self, page_dir: &Path, src: &str) -> usize {
        if !is_old_sheet(src) {
            return 0;
        }
        let file = page_dir.join(src.trim().replace('/', std::path::MAIN_SEPARATOR_STR));
        let Ok(text) = std::fs::read_to_string(&file) else { return 0 };
        let Ok((old, _)) = svg::read(&text) else { return 0 };
        let before = self.drawing.strokes.len();
        for stroke in old.strokes {
            self.drawing.add(stroke);
        }
        let came = self.drawing.strokes.len() - before;
        self.dirty |= came > 0;
        came
    }

    /// Takes the last stroke back.
    pub fn undo(&mut self) -> bool {
        // Anything drawn since the last undo replaces what could have been put back.
        if self.drawing.strokes.len() != self.redo_len {
            self.redone.clear();
        }
        let last = self.drawing.strokes.last().cloned();
        let went = self.drawing.undo();
        if went && let Some(stroke) = last {
            self.redone.push(stroke);
        }
        self.redo_len = self.drawing.strokes.len();
        self.dirty |= went;
        went
    }

    /// Whether Redo has a stroke to put back.
    pub fn can_redo(&self) -> bool {
        !self.redone.is_empty() && self.drawing.strokes.len() == self.redo_len
    }

    /// Puts back the stroke Undo took off last.
    pub fn redo(&mut self) -> bool {
        if !self.can_redo() {
            self.redone.clear();
            return false;
        }
        let Some(stroke) = self.redone.pop() else { return false };
        let put = self.drawing.add(stroke);
        self.redo_len = self.drawing.strokes.len();
        self.dirty |= put;
        put
    }

    pub fn strokes(&self) -> usize {
        self.drawing.strokes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.drawing.is_empty() && self.drawing_now.is_none()
    }

    /// How many strokes are being held.
    pub fn holding(&self) -> usize {
        self.held.len()
    }

    /// Lets go of whatever is held, which is what happens when the tool changes or the
    /// page does.
    pub fn let_go(&mut self) {
        self.held.clear();
        self.loop_now.clear();
        self.grabbed = None;
    }

    /// Throws away the held ink and says whether any went.
    pub fn delete_held(&mut self) -> bool {
        let went = pick::remove(&mut self.drawing, &self.held) > 0;
        self.let_go();
        self.dirty |= went;
        went
    }

    /// Copies of what is held, for the clipboard.
    pub fn copy_held(&self) -> Vec<Stroke> {
        pick::copy(&self.drawing, &self.held)
    }

    /// Takes what is held out of the page and hands it back for the clipboard.
    pub fn cut_held(&mut self) -> Vec<Stroke> {
        let kept = self.copy_held();
        self.delete_held();
        kept
    }

    /// Lays `strokes` down with their box's top left corner at (`x`, `y`), and holds them, so they can
    /// be moved at once. Says whether anything went down.
    pub fn put(&mut self, strokes: &[Stroke], x: f32, y: f32) -> bool {
        let at = pick::paste(&mut self.drawing, strokes, x, y);
        self.grabbed = None;
        self.dirty |= !at.is_empty();
        self.held = at;
        !self.held.is_empty()
    }

    /// Lays a copy of what is held just beside it, and holds the copy.
    pub fn duplicate_held(&mut self) -> bool {
        let (Some((x0, y0, ..)), copy) = (pick::bounds(&self.drawing, &self.held), self.copy_held()) else {
            return false;
        };
        self.put(&copy, x0 + DUPLICATE_STEP, y0 + DUPLICATE_STEP)
    }

    /// Holds every stroke on the page.
    pub fn select_all(&mut self) {
        self.grabbed = None;
        self.held = (0..self.drawing.strokes.len()).collect();
    }
}

/// How far a duplicate is laid from the ink it copies, in points, so it can be told apart.
const DUPLICATE_STEP: f32 = 18.0;

/// What Copy and Cut keep, which is shared by every page so ink can be carried to another one.
fn clipboard_id() -> egui::Id {
    egui::Id::new("ink-clipboard")
}

/// How near a corner of the box the pointer has to be to take hold of it.
const HANDLE: f32 = 9.0;

/// Draws a page's ink over the page, and takes what the pen does to it. `body` is the
/// rectangle the page's words were drawn in; the ink is kept in that rectangle's own
/// coordinates. Returns true when the ink changed.
pub fn show(ui: &mut Ui, body: egui::Rect, pad: &mut Pad, pen: Pen, pressure: f32, ink_allowed: bool, t: &Tokens, zoom: f32) -> bool {
    use crate::ribbon::Pointer;
    let _ = t;
    // Painting on the layer, not through the ui, so a stroke that runs past the words is
    // still drawn: the ui's own box ends where the text does.
    // Only what is on the screen: the canvas is larger than the window, and what is outside the
    // scroll area belongs to the ribbon and the panels around it.
    let painter = ui.ctx().layer_painter(ui.layer_id()).with_clip_rect(body.intersect(ui.clip_rect()));
    // Only the strokes that can be seen are worked out: a page can hold thousands, and making the
    // outline of each one every frame is the cost, not putting it on the screen.
    let seen = painter.clip_rect();
    let visible = egui::Rect::from_min_max(
        egui::pos2((seen.min.x - body.min.x) / zoom, (seen.min.y - body.min.y) / zoom),
        egui::pos2((seen.max.x - body.min.x) / zoom, (seen.max.y - body.min.y) / zoom),
    );
    // Finished strokes: the shapes of each are made once and moved into place, so drawing a page of
    // thousands costs what is in view and not what was ever drawn. A stroke that changed, moved or
    // was rubbed out has a different fingerprint and is made again; a new zoom starts afresh.
    if pad.shapes_zoom != zoom.to_bits() || pad.shapes.len() > 1500 {
        pad.shapes.clear();
        pad.shapes_zoom = zoom.to_bits();
    }
    let delta = body.min.to_vec2();
    for (i, stroke) in pad.drawing.strokes.iter().enumerate() {
        if !may_show(stroke, visible) {
            continue;
        }
        let key = stroke_fingerprint(stroke);
        let made = match pad.shapes.get(&i) {
            Some((k, shapes)) if *k == key => shapes,
            _ => {
                let mut scaled = stroke.clone();
                scaled.width *= zoom;
                for dab in &mut scaled.dabs {
                    dab.x *= zoom;
                    dab.y *= zoom;
                }
                &pad.shapes.entry(i).insert_entry((key, stroke_shapes(egui::Pos2::ZERO, &scaled))).into_mut().1
            }
        };
        for shape in made {
            let mut shape = shape.clone();
            shape.translate(delta);
            painter.add(shape);
        }
    }
    // The stroke being drawn changes every frame, so it is made every frame.
    if let Some(stroke) = pad.drawing_now.as_ref() {
        let mut scaled = stroke.clone();
        scaled.width *= zoom;
        for dab in &mut scaled.dabs {
            dab.x *= zoom;
            dab.y *= zoom;
        }
        paint_stroke(&painter, body.min, &scaled);
    }

    // A pen or a rubber in the hand means the ink is not being held any more.
    if matches!(pen.pointer, Pointer::Draw | Pointer::Erase) && !pad.held.is_empty() {
        pad.let_go();
    }
    // What is held is outlined wherever the pointer is, so it is plain what a move or a
    // delete would take.
    if !pad.held.is_empty()
        && let Some((x0, y0, x1, y1)) = pick::bounds(&pad.drawing, &pad.held)
    {
        let box_ = egui::Rect::from_min_max(body.min + vec2(x0 * zoom, y0 * zoom), body.min + vec2(x1 * zoom, y1 * zoom)).expand(3.0);
        painter.rect_stroke(box_, 2.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Outside);
        for corner in [box_.left_top(), box_.right_top(), box_.left_bottom(), box_.right_bottom()] {
            painter.rect_filled(egui::Rect::from_center_size(corner, vec2(7.0, 7.0)), 1.0, t.accent);
        }
    }
    if !pad.loop_now.is_empty() {
        let path: Vec<egui::Pos2> = pad.loop_now.iter().map(|(x, y)| body.min + vec2(*x * zoom, *y * zoom)).collect();
        painter.add(egui::Shape::line(path, egui::Stroke::new(1.2, t.accent)));
    }

    if pen.pointer == Pointer::Write {
        pad.let_go();
        // The tool was put down mid-stroke; keep what was drawn rather than losing it.
        if let Some(stroke) = pad.drawing_now.take() {
            let kept = pad.drawing.add(stroke);
            pad.dirty |= kept;
            return kept;
        }
        return false;
    }

    // While a pen is in hand the whole page belongs to it, so this goes on last and over
    // everything: a press must not land in the words underneath and move the caret.
    let resp = ui.interact(body, ui.id().with("ink-layer"), Sense::click_and_drag());
    ui.ctx().set_cursor_icon(match pen.pointer {
        Pointer::Erase => egui::CursorIcon::NotAllowed,
        Pointer::Pick => egui::CursorIcon::Grab,
        _ => egui::CursorIcon::Crosshair,
    });

    // Not egui's drag, which only begins once the pointer has travelled far enough: a pen
    // put down and lifted in one place is a dot, and a dot is ink.
    // `ink_allowed` is false for a finger when only the pen may write.
    let down = ink_allowed && resp.is_pointer_button_down_on();
    // A finger on the ink layer scrolls the page, as it would anywhere else.
    // It keeps coasting after the finger lifts, and a touch of the glass catches it.
    //
    // A hand laid down to write touches the glass before the pen does, and for a moment looks like
    // a finger dragging the page. What a finger has scrolled while it was still down is taken back
    // when the system calls that touch off (a palm) or the pen lands: the page goes back where it was.
    {
        let fling = ui.id().with("ink-fling");
        let palm = ui.id().with("palm-guard");
        let dragging = !ink_allowed && resp.dragged();
        let called_off = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Touch { phase: egui::TouchPhase::Cancel, force: None, .. })));
        let mut applied: egui::Vec2 = ui.data(|d| d.get_temp(palm)).unwrap_or_default();
        if (ink_allowed || called_off) && applied != egui::Vec2::ZERO {
            ui.scroll_with_delta_animation(-applied, egui::style::ScrollAnimation::none());
            crate::kinetic::cancel(ui.ctx(), fling);
            applied = egui::Vec2::ZERO;
        } else {
            let drag = dragging.then(|| resp.drag_delta());
            let moved = crate::kinetic::step(ui.ctx(), fling, drag, crate::kinetic::both());
            if moved != egui::Vec2::ZERO {
                ui.scroll_with_delta_animation(moved, egui::style::ScrollAnimation::none());
            }
            // Only a drag still in progress can be taken back; coasting after a release is kept.
            applied = if dragging { applied + moved } else { egui::Vec2::ZERO };
        }
        ui.data_mut(|d| d.insert_temp(palm, applied));
    }
    let at = ui.ctx().pointer_interact_pos().filter(|p| body.contains(*p)).map(|p| ((p.x - body.min.x) / zoom, (p.y - body.min.y) / zoom));
    let mut changed = false;
    match pen.pointer {
        Pointer::Erase => {
            if let Some((x, y)) = at.filter(|_| down) {
                changed = pad.drawing.erase_at(x, y, pen.width.max(6.0) / 2.0) > 0;
            }
        }
        Pointer::Draw => {
            if down {
                if pad.drawing_now.is_none() {
                    pad.drawing_now = Some(Stroke::with_gain(pen.tool, pen.colour, pen.width, pen.gain));
                }
                // Every sample the pen gave since the last frame, not only the latest: a pen
                // reports several hundred times a second and a frame is a sixtieth of one, so
                // one point a frame draws a coarse line that trails behind the pen.
                let samples = ui.input(|i| pen_samples(&i.events, pressure));
                if let Some(stroke) = pad.drawing_now.as_mut() {
                    if samples.is_empty()
                        && let Some((x, y)) = at
                    {
                        stroke.add(Dab::new(x, y, pressure));
                    }
                    for (point, force) in samples {
                        stroke.add(Dab::new((point.x - body.min.x) / zoom, (point.y - body.min.y) / zoom, force));
                    }
                    // The stroke is painted as it grows, so the line follows the pen
                    // rather than appearing when it is lifted.
                    ui.ctx().request_repaint();
                }
            } else if let Some(stroke) = pad.drawing_now.take() {
                changed = pad.drawing.add(stroke);
            }
        }
        Pointer::Shape(shape) => {
            // The drag gives the two corners the shape is drawn between: it follows the
            // pointer while the button is down, and is laid down when it is let go.
            if down {
                if let Some((x, y)) = at {
                    let from = *pad.shape_from.get_or_insert((x, y));
                    let dabs = notecraft_ink::shapes::trace(shape, from, (x, y), 0.6);
                    let mut stroke = Stroke::with_gain(pen.tool, pen.colour, pen.width, pen.gain);
                    for dab in dabs {
                        stroke.add(dab);
                    }
                    pad.drawing_now = Some(stroke);
                    ui.ctx().request_repaint();
                }
            } else {
                pad.shape_from = None;
                if let Some(stroke) = pad.drawing_now.take() {
                    changed = pad.drawing.add(stroke);
                }
            }
        }
        Pointer::Lasso => {
            if down {
                if let Some((x, y)) = at {
                    // Only every few points: a loop of every frame's position is far more
                    // than a ray-casting test needs, and it is thrown away at the end.
                    if pad.loop_now.last().is_none_or(|(lx, ly)| (lx - x).hypot(ly - y) > 2.0) {
                        pad.loop_now.push((x, y));
                    }
                    ui.ctx().request_repaint();
                }
            } else if !pad.loop_now.is_empty() {
                let loop_ = std::mem::take(&mut pad.loop_now);
                pad.held = pick::inside(&pad.drawing, &loop_);
            }
        }
        Pointer::Pick => {
            let held_box = pick::bounds(&pad.drawing, &pad.held).map(|(x0, y0, x1, y1)| {
                egui::Rect::from_min_max(body.min + vec2(x0 * zoom, y0 * zoom), body.min + vec2(x1 * zoom, y1 * zoom)).expand(3.0)
            });
            let here = ui.ctx().pointer_interact_pos().unwrap_or(egui::Pos2::ZERO);
            if down && pad.grabbed.is_none() {
                // A press on a corner stretches, inside the box moves, on a stroke picks
                // that stroke up, and anywhere else lets go.
                let corner = held_box.and_then(|b| grab_corner(b, here));
                match (corner, held_box, at) {
                    (Some(c), Some(b), _) => pad.grabbed = Some((here, as_tuple(b, body, zoom), c)),
                    (None, Some(b), _) if b.contains(here) => pad.grabbed = Some((here, as_tuple(b, body, zoom), Grab::Move)),
                    (_, _, Some((x, y))) => match pick::at(&pad.drawing, x, y, 3.0) {
                        Some(i) => {
                            pad.held = vec![i];
                            if let Some(b) = pick::bounds(&pad.drawing, &pad.held) {
                                pad.grabbed = Some((here, b, Grab::Move));
                            }
                        }
                        None => pad.let_go(),
                    },
                    _ => {}
                }
            }
            if let Some((from, was, grab)) = pad.grabbed {
                let (dx, dy) = ((here.x - from.x) / zoom, (here.y - from.y) / zoom);
                if down {
                    // The ink follows the pointer from where it was taken hold of, so a
                    // long drag cannot drift away from the hand.
                    let (x0, y0, x1, y1) = was;
                    match grab {
                        Grab::Move => {
                            let now = pick::bounds(&pad.drawing, &pad.held).unwrap_or(was);
                            pick::move_by(&mut pad.drawing, &pad.held, x0 + dx - now.0, y0 + dy - now.1);
                        }
                        Grab::Corner(right, bottom) => {
                            let (ox, oy) = (if right { x0 } else { x1 }, if bottom { y0 } else { y1 });
                            let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
                            let want_w = if right { w + dx } else { w - dx };
                            let want_h = if bottom { h + dy } else { h - dy };
                            let now = pick::bounds(&pad.drawing, &pad.held).unwrap_or(was);
                            let have_w = (now.2 - now.0).max(0.001);
                            let have_h = (now.3 - now.1).max(0.001);
                            pick::scale_about(&mut pad.drawing, &pad.held, ox, oy, want_w.max(4.0) / have_w, want_h.max(4.0) / have_h);
                        }
                    }
                    ui.ctx().request_repaint();
                } else {
                    pad.grabbed = None;
                    changed = true;
                }
            }
        }
        Pointer::Write => {}
    }
    // Delete throws away whatever is held, however it was picked up.
    if !pad.held.is_empty() && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        changed |= pad.delete_held();
    }
    changed |= actions(ui, body, pad, pen.pointer, zoom);
    pad.dirty |= changed;
    changed
}

/// The strip of actions over held ink (and Paste, when something has been copied): the same things the
/// keyboard does, for a tablet that has none. Returns true when the ink changed.
fn actions(ui: &mut Ui, body: egui::Rect, pad: &mut Pad, pointer: crate::ribbon::Pointer, zoom: f32) -> bool {
    use crate::ribbon::Pointer;
    if !matches!(pointer, Pointer::Lasso | Pointer::Pick) {
        return false;
    }
    let ctx = ui.ctx().clone();
    let clip: Vec<Stroke> = ctx.data(|d| d.get_temp(clipboard_id())).unwrap_or_default();
    let mut changed = false;
    // The keyboard: Ctrl or Command with C, X, V, D and A, so a desktop with a pen is as quick.
    let (copy, cut, paste, dup, all) = ctx.input_mut(|i| {
        let m = egui::Modifiers::COMMAND;
        (
            i.consume_key(m, egui::Key::C),
            i.consume_key(m, egui::Key::X),
            i.consume_key(m, egui::Key::V),
            i.consume_key(m, egui::Key::D),
            i.consume_key(m, egui::Key::A),
        )
    });
    let held = !pad.held.is_empty();
    let seen = body.intersect(ui.clip_rect());
    let view_top_left = ((seen.min.x - body.min.x) / zoom + 24.0, (seen.min.y - body.min.y) / zoom + 24.0);
    let mut action = None;
    if all {
        action = Some("all");
    } else if held && copy {
        action = Some("copy");
    } else if held && cut {
        action = Some("cut");
    } else if held && dup {
        action = Some("dup");
    } else if !clip.is_empty() && paste {
        action = Some("paste");
    }
    // While the ink is being moved or a loop drawn, the strip stays out of the way.
    if pad.grabbed.is_none() && pad.loop_now.is_empty() && (held || !clip.is_empty() || !pad.drawing.strokes.is_empty()) {
        let anchor = pick::bounds(&pad.drawing, &pad.held)
            .map(|(x0, y0, x1, _)| egui::pos2(body.min.x + (x0 + x1) / 2.0 * zoom, body.min.y + y0 * zoom - 16.0))
            .unwrap_or(egui::pos2(seen.center().x, seen.min.y + 28.0));
        let anchor = egui::pos2(
            anchor.x.clamp(seen.min.x + 120.0, (seen.max.x - 120.0).max(seen.min.x + 120.0)),
            anchor.y.clamp(seen.min.y + 26.0, seen.max.y - 26.0),
        );
        egui::Area::new(ui.id().with("ink-actions")).order(egui::Order::Foreground).pivot(egui::Align2::CENTER_BOTTOM).fixed_pos(anchor).show(
            &ctx,
            |ui| {
                egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if held {
                            for (name, id) in [("Duplicate", "dup"), ("Cut", "cut"), ("Copy", "copy"), ("Delete", "delete")] {
                                if ui.button(name).clicked() {
                                    action = Some(id);
                                }
                            }
                        }
                        if !clip.is_empty() && ui.button("Paste").clicked() {
                            action = Some("paste");
                        }
                        if !held && ui.button("Select all").clicked() {
                            action = Some("all");
                        }
                    });
                });
            },
        );
    }
    match action {
        Some("copy") => {
            let kept = pad.copy_held();
            ctx.data_mut(|d| d.insert_temp(clipboard_id(), kept));
        }
        Some("cut") => {
            let kept = pad.cut_held();
            ctx.data_mut(|d| d.insert_temp(clipboard_id(), kept));
            changed = true;
        }
        Some("delete") => changed |= pad.delete_held(),
        Some("dup") => changed |= pad.duplicate_held(),
        Some("paste") => changed |= pad.put(&clip, view_top_left.0, view_top_left.1),
        Some("all") => pad.select_all(),
        _ => {}
    }
    changed
}

/// The corner of `box_` the pointer has hold of, if it is near one. The two flags say
/// which way that corner moves: right, and down.
fn grab_corner(box_: egui::Rect, at: egui::Pos2) -> Option<Grab> {
    let near = |c: egui::Pos2| (c.x - at.x).abs() <= HANDLE && (c.y - at.y).abs() <= HANDLE;
    if near(box_.right_bottom()) {
        return Some(Grab::Corner(true, true));
    }
    if near(box_.left_bottom()) {
        return Some(Grab::Corner(false, true));
    }
    if near(box_.right_top()) {
        return Some(Grab::Corner(true, false));
    }
    if near(box_.left_top()) {
        return Some(Grab::Corner(false, false));
    }
    None
}

/// A rectangle on the screen as the ink's own coordinates.
fn as_tuple(b: egui::Rect, body: egui::Rect, zoom: f32) -> (f32, f32, f32, f32) {
    ((b.min.x - body.min.x) / zoom, (b.min.y - body.min.y) / zoom, (b.max.x - body.min.x) / zoom, (b.max.y - body.min.y) / zoom)
}

/// One stroke, as triangles between the two edges of its ribbon, with a round cap at each
/// end. The edges are the ones the saved outline is built from, so the screen and the file
/// show the same shape.
pub fn paint_stroke(painter: &egui::Painter, origin: egui::Pos2, stroke: &Stroke) {
    for shape in stroke_shapes(origin, stroke) {
        painter.add(shape);
    }
}

/// What it takes to draw one stroke: the ribbon of the line as a mesh, and a disc at each end. Making
/// these is the cost of drawing ink (the outline of the line), so a finished stroke's are kept (see
/// [`Pad::show`]) and moved into place rather than made again every frame.
pub fn stroke_shapes(origin: egui::Pos2, stroke: &Stroke) -> Vec<egui::Shape> {
    let colour = egui::Color32::from_rgb(stroke.colour.0, stroke.colour.1, stroke.colour.2).gamma_multiply(stroke.tool.opacity());
    let put = |x: f32, y: f32| egui::pos2(origin.x + x, origin.y + y);
    let (left, right) = shape::sides(stroke);
    if left.len() < 2 {
        // A dot: one reading, so there is no ribbon, only a blob.
        return stroke
            .dabs
            .first()
            .map(|d| egui::Shape::circle_filled(put(d.x, d.y), shape::width_at(stroke, d.pressure) / 2.0, colour))
            .into_iter()
            .collect();
    }
    let mut mesh = egui::Mesh::default();
    for (l, r) in left.iter().zip(&right) {
        mesh.colored_vertex(put(l.0, l.1), colour);
        mesh.colored_vertex(put(r.0, r.1), colour);
    }
    #[allow(clippy::cast_possible_truncation)]
    for i in 0..(left.len() as u32 - 1) {
        let (a, b, c, d) = (i * 2, i * 2 + 1, i * 2 + 2, i * 2 + 3);
        mesh.add_triangle(a, b, c);
        mesh.add_triangle(b, c, d);
    }
    let mut shapes = vec![egui::Shape::mesh(mesh)];
    // Round the two ends, which is what the saved outline does with its half circles.
    let line = shape::centre(stroke);
    for end in [line.first(), line.last()].into_iter().flatten() {
        shapes.push(egui::Shape::circle_filled(put(end.x, end.y), shape::width_at(stroke, end.pressure) / 2.0, colour));
    }
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;
    use notecraft_store::testing::TempDir;

    fn stroke(x: f32) -> Stroke {
        let mut s = Stroke::new(Tool::Pen, Rgb(0xd0, 0x21, 0x21), 4.0);
        s.dabs.push(Dab::new(x, 40.0, 0.3));
        s.dabs.push(Dab::new(x + 30.0, 40.0, 1.0));
        s
    }

    #[test]
    fn held_ink_can_be_duplicated_cut_and_put_back_on_another_page() {
        let mut pad = Pad::of_page(TempDir::new("ink-held-a").path());
        pad.drawing.add(stroke(10.0));
        pad.drawing.add(stroke(100.0));
        pad.select_all();
        assert_eq!(pad.holding(), 2);
        assert!(pad.duplicate_held());
        assert_eq!(pad.strokes(), 4);
        assert_eq!(pad.holding(), 2, "the copies are the ones held now");
        assert!((pad.drawing.strokes[2].dabs[0].x - 28.0).abs() < 0.5, "and lie beside the first, not on it");

        let kept = pad.cut_held();
        assert_eq!((kept.len(), pad.strokes(), pad.holding()), (2, 2, 0), "cut takes them off the page and lets go");
        assert!(!pad.delete_held(), "nothing is held, so nothing goes");

        let mut other = Pad::of_page(TempDir::new("ink-held-b").path());
        assert!(other.put(&kept, 50.0, 50.0));
        assert_eq!((other.strokes(), other.holding()), (2, 2));
        assert!(other.dirty, "so that it is saved");
    }

    #[test]
    fn a_page_with_no_ink_has_no_ink_file() {
        let t = TempDir::new("ink-layer");
        let page = t.path().to_path_buf();
        let pad = Pad::of_page(&page);
        assert!(pad.is_empty(), "a page nobody has drawn on starts clean");
        assert!(!pad.file.exists(), "and nothing was written to find that out");
        assert!(pad.file.ends_with(FILE), "the ink sits beside the page: {}", pad.file.display());
    }

    #[test]
    fn ink_is_written_beside_the_page_and_comes_back_as_strokes() {
        let t = TempDir::new("ink-layer-save");
        let page = t.path().to_path_buf();
        let mut pad = Pad::of_page(&page);
        assert!(pad.drawing.add(stroke(10.0)));
        pad.dirty = true;
        pad.save().expect("saved");
        assert!(page.join(FILE).exists(), "the file appeared the moment it was drawn on");

        let again = Pad::of_page(&page);
        assert_eq!(again.strokes(), 1);
        assert_eq!(again.flat, 0, "with its readings, so it can still be changed");
        let back = again.drawing.strokes.first().expect("one");
        assert_eq!(back.colour, Rgb(0xd0, 0x21, 0x21));
        assert_eq!(back.dabs.len(), 2);
        assert!((back.dabs[1].pressure - 1.0).abs() < 0.01, "and its pressure");

        // Saved as a picture it is beside the ink, and holds no readings.
        let plain = again.save_plain().expect("plain");
        assert!(plain.ends_with("ink-picture.svg"));
        let text = std::fs::read_to_string(&plain).expect("read");
        assert!(text.contains("<path"), "the picture is there");
        assert!(!text.contains("nc:dabs"), "but the readings are not");
    }

    #[test]
    fn rubbing_a_page_clean_takes_the_file_away_again() {
        let t = TempDir::new("ink-layer-clean");
        let page = t.path().to_path_buf();
        let mut pad = Pad::of_page(&page);
        pad.drawing.add(stroke(10.0));
        pad.save().expect("saved");
        assert!(page.join(FILE).exists());

        assert!(pad.undo());
        pad.save().expect("saved again");
        assert!(!page.join(FILE).exists(), "a page rubbed clean keeps no empty file");
        assert!(!pad.undo(), "and there is nothing more to take back");
        pad.save().expect("saving nothing is not an error");
    }

    #[test]
    fn a_damaged_ink_file_opens_as_an_empty_page_rather_than_failing() {
        let t = TempDir::new("ink-layer-junk");
        let page = t.path().to_path_buf();
        std::fs::write(page.join(FILE), "this is not a drawing").expect("write");
        let pad = Pad::of_page(&page);
        assert!(pad.is_empty(), "nothing was read, and nothing broke");
        assert!(pad.drawing.width >= 1.0 && pad.drawing.height >= 1.0);
    }

    #[test]
    fn the_canvas_grows_when_ink_reaches_the_edge_and_never_shrinks_below_the_column() {
        let view = vec2(1200.0, 800.0);
        let column = 780.0;
        // Nothing near the right edge: the page is just its column, as before.
        assert_eq!(canvas((100.0, 100.0), 300.0, view, column).x, column);
        // Ink within reach of the edge gives room beyond it, half a screen's worth.
        let near = canvas((column - 20.0, 100.0), 300.0, view, column);
        assert_eq!(near.x, column - 20.0 + 600.0);
        // Further right still: it keeps going, with no limit.
        let far = canvas((5000.0, 100.0), 300.0, view, column);
        assert_eq!(far.x, 5600.0);
        // Down: always the sheet, the text and some room, and half a screen past the lowest ink.
        assert_eq!(canvas((0.0, 0.0), 300.0, view, column).y, SHEET_H);
        assert_eq!(canvas((0.0, 3000.0), 300.0, view, column).y, 3400.0);
    }

    #[test]
    fn the_pad_knows_how_far_the_ink_reaches_and_the_file_says_so() {
        let t = TempDir::new("ink-layer-far");
        let page = t.path().to_path_buf();
        let mut pad = Pad::of_page(&page);
        assert_eq!(pad.far(), (0.0, 0.0));
        let mut s = Stroke::new(notecraft_ink::Tool::Pen, notecraft_ink::Rgb::BLACK, 2.0);
        s.add(Dab::new(3000.0, 2500.0, 1.0));
        pad.drawing.add(s);
        let (x, y) = pad.far();
        assert!(x >= 3000.0 && y >= 2500.0, "{x},{y}");
        pad.save().expect("saved");
        let text = std::fs::read_to_string(page.join(FILE)).expect("file");
        assert!(text.contains("viewBox=\"0 0 3"), "the sheet the file declares covers the ink: {}", &text[..text.len().min(300)]);
        assert!(Pad::of_page(&page).far().0 >= 3000.0, "and it comes back");
    }

    fn touch(x: f32, force: Option<f32>, phase: egui::TouchPhase) -> egui::Event {
        egui::Event::Touch { device_id: egui::TouchDeviceId(0), id: egui::TouchId(0), phase, pos: egui::pos2(x, 10.0), force }
    }

    #[test]
    fn every_sample_of_the_pen_in_a_frame_is_drawn_not_just_the_last() {
        use egui::TouchPhase::{End, Move, Start};
        let frame = [
            touch(10.0, Some(0.2), Start),
            egui::Event::PointerMoved(egui::pos2(10.0, 10.0)),
            touch(12.0, Some(0.4), Move),
            touch(12.0, Some(0.4), Move),
            touch(15.0, Some(0.9), Move),
            egui::Event::PointerMoved(egui::pos2(15.0, 10.0)),
            touch(16.0, Some(0.1), End),
        ];
        let got = pen_samples(&frame, 1.0);
        let xs: Vec<f32> = got.iter().map(|(p, _)| p.x).collect();
        assert_eq!(xs, [10.0, 12.0, 15.0], "in order, a repeat is one, and the lift is not a point");
        assert_eq!(got[2].1, 0.9, "each with its own pressure");
    }

    #[test]
    fn a_mouse_and_a_touch_without_force_draw_at_full_pressure() {
        let mouse = [egui::Event::PointerMoved(egui::pos2(5.0, 5.0)), egui::Event::PointerMoved(egui::pos2(6.0, 5.0))];
        assert_eq!(pen_samples(&mouse, 1.0).len(), 2);
        assert!(pen_samples(&mouse, 1.0).iter().all(|(_, f)| *f == 1.0));
        let finger = [touch(5.0, None, egui::TouchPhase::Move)];
        assert!(pen_samples(&finger, 1.0).is_empty(), "a finger is not a pen's sample");
        assert!(pen_samples(&[touch(5.0, Some(f32::NAN), egui::TouchPhase::Move)], 1.0).is_empty(), "junk is dropped");
    }

    #[test]
    fn a_stroke_taken_back_can_be_put_back_until_something_new_is_drawn() {
        let t = TempDir::new("ink-redo");
        let mut pad = Pad::of_page(t.path());
        pad.drawing.add(stroke(10.0));
        pad.drawing.add(stroke(50.0));
        assert!(!pad.can_redo(), "nothing was taken back yet");
        assert!(pad.undo() && pad.undo());
        assert_eq!(pad.strokes(), 0);
        assert!(pad.can_redo());
        assert!(pad.redo(), "the first stroke comes back first");
        assert_eq!(pad.drawing.strokes[0].dabs[0].x, 10.0);
        assert!(pad.redo());
        assert_eq!(pad.strokes(), 2);
        assert!(!pad.redo(), "and then there is nothing left to put back");

        // Drawing something new ends the chance to put the old strokes back.
        assert!(pad.undo());
        pad.drawing.add(stroke(90.0));
        assert!(!pad.can_redo(), "a new stroke replaced what could have come back");
        assert!(!pad.redo());
    }

    #[test]
    fn only_strokes_that_meet_the_visible_part_of_the_page_are_drawn() {
        let on_screen = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(800.0, 600.0));
        assert!(may_show(&stroke(10.0), on_screen), "a stroke in view");
        assert!(!may_show(&stroke(5000.0), on_screen), "one far to the right is skipped");
        let mut below = stroke(10.0);
        for d in &mut below.dabs {
            d.y = 9000.0;
        }
        assert!(!may_show(&below, on_screen), "and one far below");
        let mut edge = stroke(790.0);
        edge.width = 40.0;
        assert!(may_show(&edge, egui::Rect::from_min_max(egui::pos2(805.0, 0.0), egui::pos2(900.0, 600.0))), "its thick line reaches in");
        assert!(!may_show(&Stroke::new(Tool::Pen, Rgb::BLACK, 3.0), on_screen), "a stroke with no readings has nothing to draw");
    }

    /// Waits for the background write to finish, as the window would over a few frames.
    fn settle(pad: &mut Pad) {
        for _ in 0..400 {
            if pad.maintain().is_none() && !pad.writing() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the background write did not finish");
    }

    fn paths(file: &std::path::Path) -> usize {
        std::fs::read_to_string(file).unwrap_or_default().matches("<path").count()
    }

    /// How many strokes the journal holds, and the number of its first entry.
    fn entries(page: &std::path::Path) -> (usize, usize) {
        std::fs::read(page.join(JOURNAL)).ok().and_then(|b| journal::decode(&b)).map_or((0, 0), |j| (j.strokes.len(), j.base))
    }

    #[test]
    fn a_stroke_after_the_first_costs_a_line_of_the_journal_not_the_whole_file() {
        let t = TempDir::new("ink-journal");
        let page = t.path().to_path_buf();
        let mut pad = Pad::of_page(&page);
        pad.drawing.add(stroke(10.0));
        pad.save().expect("first save writes the file");
        let file = std::fs::read(page.join(FILE)).expect("the file");
        assert_eq!(paths(&page.join(FILE)), 1);
        pad.drawing.add(stroke(50.0));
        pad.save().expect("second");
        pad.drawing.add(stroke(90.0));
        pad.save().expect("third");

        assert_eq!(std::fs::read(page.join(FILE)).expect("the file"), file, "ink.svg was not touched");
        assert_eq!(entries(&page).0, 2, "the two later strokes are in the journal");
        let again = Pad::of_page(&page);
        assert_eq!(again.strokes(), 3, "and the page comes back with all three");
    }

    #[test]
    fn the_journal_is_folded_into_the_file_in_the_background_and_goes() {
        let t = TempDir::new("ink-fold");
        let page = t.path().to_path_buf();
        let mut pad = Pad::of_page(&page);
        for x in [10.0, 50.0, 90.0] {
            pad.drawing.add(stroke(x));
            pad.save().expect("save");
        }
        pad.flush();
        settle(&mut pad);
        assert!(!page.join(JOURNAL).exists(), "everything is in the file now");
        assert_eq!(paths(&page.join(FILE)), 3);
        assert_eq!(Pad::of_page(&page).strokes(), 3);

        // Later strokes start a new journal that carries on the numbering, and are not lost or doubled.
        pad.drawing.add(stroke(130.0));
        pad.save().expect("save");
        let again = Pad::of_page(&page);
        assert_eq!(again.strokes(), 4);
        assert_eq!(entries(&page), (1, 2), "one entry, numbered on from the two before");
    }

    #[test]
    fn a_journal_that_is_forty_strokes_long_is_folded_without_being_asked() {
        let t = TempDir::new("ink-forty");
        let page = t.path().to_path_buf();
        let mut pad = Pad::of_page(&page);
        for i in 0..42 {
            pad.drawing.add(stroke(i as f32 * 5.0));
            pad.save().expect("save");
            pad.maintain();
        }
        settle(&mut pad);
        assert_eq!(Pad::of_page(&page).strokes(), 42);
        assert!(paths(&page.join(FILE)) >= 41, "the file caught up: {}", paths(&page.join(FILE)));
    }

    #[test]
    fn taking_a_stroke_back_is_written_as_a_whole_and_is_not_lost_to_the_journal() {
        let t = TempDir::new("ink-rewrite");
        let page = t.path().to_path_buf();
        let mut pad = Pad::of_page(&page);
        for x in [10.0, 50.0, 90.0] {
            pad.drawing.add(stroke(x));
            pad.save().expect("save");
        }
        assert!(pad.undo());
        pad.save().expect("save after undo");
        settle(&mut pad);
        assert_eq!(Pad::of_page(&page).strokes(), 2, "the stroke taken back does not come back");

        // Something rubbed out from the middle, too.
        pad.drawing.strokes.remove(0);
        pad.save().expect("save after erasing");
        settle(&mut pad);
        let again = Pad::of_page(&page);
        assert_eq!(again.strokes(), 1);
        assert_eq!(again.drawing.strokes[0].dabs[0].x, 50.0);
    }

    #[test]
    fn a_half_written_last_line_and_entries_already_in_the_file_are_not_strokes() {
        let t = TempDir::new("ink-crash");
        let page = t.path().to_path_buf();
        let mut drawing = Drawing::new(SHEET_W, SHEET_H);
        drawing.add(stroke(10.0));
        drawing.add(stroke(50.0));
        std::fs::write(page.join(FILE), svg::write_folded(&drawing, svg::Flavour::Editable, 2)).expect("file");
        // Entries 0 and 1 are in the file already; 2 is new; the last record was cut off by a crash.
        let mut bytes = journal::header(0);
        for x in [10.0, 50.0, 90.0, 130.0] {
            bytes.extend(journal::encode(&stroke(x)).expect("record"));
        }
        bytes.truncate(bytes.len() - 7);
        std::fs::write(page.join(JOURNAL), &bytes).expect("journal");
        let pad = Pad::of_page(&page);
        assert_eq!(pad.strokes(), 3, "the two in the file, the one new, and not the torn record");
        // The torn end was cut off, so the next stroke is not appended after garbage.
        let mut pad = pad;
        pad.drawing.add(stroke(170.0));
        pad.save().expect("save");
        assert_eq!(Pad::of_page(&page).strokes(), 4);
    }

    #[test]
    fn an_ink_file_that_cannot_be_read_is_kept_not_written_over() {
        let t = TempDir::new("ink-keep");
        let page = t.path().to_path_buf();
        std::fs::write(page.join(FILE), "this is not a drawing").expect("write");
        let mut pad = Pad::of_page(&page);
        assert!(pad.is_empty());
        assert_eq!(std::fs::read_to_string(page.join(UNREADABLE)).expect("kept"), "this is not a drawing");
        pad.drawing.add(stroke(10.0));
        pad.save().expect("save");
        assert!(page.join(UNREADABLE).exists(), "still there after the next stroke");
    }

    #[test]
    fn shapes_made_once_and_moved_into_place_are_the_shapes_made_in_place() {
        let mut long = Stroke::new(Tool::Pen, Rgb(0xd0, 0x21, 0x21), 4.0);
        for i in 0..30 {
            long.add(Dab::new(i as f32 * 4.0, (i as f32 * 0.4).sin() * 9.0, 0.3 + 0.02 * i as f32));
        }
        let origin = egui::pos2(120.0, 340.0);
        let direct = stroke_shapes(origin, &long);
        let mut moved = stroke_shapes(egui::Pos2::ZERO, &long);
        for shape in &mut moved {
            shape.translate(origin.to_vec2());
        }
        assert_eq!(direct.len(), 3, "the ribbon and a disc at each end");
        assert_eq!(direct.len(), moved.len());
        let (egui::Shape::Mesh(a), egui::Shape::Mesh(b)) = (&direct[0], &moved[0]) else { panic!("the first shape is the ribbon") };
        assert_eq!(a.vertices.len(), b.vertices.len());
        for (x, y) in a.vertices.iter().zip(&b.vertices) {
            assert!((x.pos - y.pos).length() < 0.01, "{:?} {:?}", x.pos, y.pos);
        }
        assert_ne!(stroke_fingerprint(&long), stroke_fingerprint(&stroke(10.0)), "different strokes are told apart");
    }
}
