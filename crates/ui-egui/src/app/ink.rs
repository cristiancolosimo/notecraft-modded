//! The pen, its rack, and the drawing on the page.

use super::*;

impl NoteCraftApp {
    /// Puts whatever is on the clipboard into the page: a picture becomes a file beside
    /// the page and a line that shows it, anything else goes in as words.
    ///
    /// egui hands a window only `Paste(String)`, so a picture copied from a screen
    /// clipper never arrived and Ctrl+V did nothing at all. The clipboard is read here
    /// directly instead.
    pub(super) fn paste_here(&mut self) {
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
            self.say("Open a page first");
            return;
        };
        if let Some(picture) = self.clipboard.picture() {
            self.paste_picture(&dir, picture);
            return;
        }
        // Browsers, Word and other editors offer a formatted clipboard flavour as well
        // as plain text. Parse it through the same HTML-to-block path as an HTML page, so
        // links, emphasis, lists and tables survive a paste into NoteCraft.
        if let Some(html) = self.clipboard.html() {
            let new = pageedit::pasted_blocks(pageedit::pasted(&html));
            if !new.is_empty()
                && let Some(ed) = self.editor.as_mut().filter(|ed| ed.writable)
            {
                let at = ed.caret.as_ref().map_or(ed.blocks.len().saturating_sub(1), |c| c.block);
                if ed.blocks.is_empty() {
                    ed.blocks.push(notecraft_formats::markdown::Block::Paragraph(Vec::new()));
                }
                pageedit::paste_at(&mut ed.blocks, &mut ed.caret, at, new);
                ed.rebuild_body();
                Self::mark_dirty_from_blocks(ed);
                self.say("Pasted formatted content");
                return;
            }
        }
        match self.clipboard.text() {
            Some(text) => {
                let new = pageedit::blocks_to_paste(&text);
                match self.editor.as_mut() {
                    Some(ed) if !new.is_empty() && ed.writable => {
                        let at = ed.caret.as_ref().map_or(ed.blocks.len().saturating_sub(1), |c| c.block);
                        if ed.blocks.is_empty() {
                            ed.blocks.push(notecraft_formats::markdown::Block::Paragraph(Vec::new()));
                        }
                        pageedit::paste_at(&mut ed.blocks, &mut ed.caret, at, new);
                        ed.rebuild_body();
                        Self::mark_dirty_from_blocks(ed);
                    }
                    _ => self.insert_in_page(&text),
                }
                self.say("Pasted");
            }
            None => self.say("There is nothing on the clipboard to paste"),
        }
    }

    pub(super) fn paste_picture(&mut self, dir: &std::path::Path, picture: crate::clipboard::Picture) {
        let name = crate::clipboard::picture_name();
        let (w, h) = (picture.width, picture.height);
        match notecraft_store::assets::attach_bytes(dir, &name, &picture.png) {
            Ok(link) => {
                if let Some(ed) = self.editor.as_mut() {
                    let at = ed.caret.as_ref().map_or(ed.blocks.len(), |c| c.block + 1).min(ed.blocks.len());
                    ed.blocks.insert(
                        at,
                        notecraft_formats::markdown::Block::Image { alt: "Pasted picture".to_string(), src: link, width: None, height: None },
                    );
                    ed.rebuild_body();
                    Self::mark_dirty_from_blocks(ed);
                    ed.attachments = notecraft_store::assets::list(dir);
                }
                self.say(format!("Pasted a picture, {w} by {h}"));
            }
            Err(e) => self.complain(&format!("Cannot save the pasted picture: {e}")),
        }
    }

    /// Keeps the page's words honest about its ink: a page with ink names `ink.svg`, a
    /// page without it does not. Without this the source pane showed a page of text while
    /// the page itself was covered in handwriting, and another program opening the folder
    /// would never have known the drawing was there.
    pub(super) fn sync_ink_reference(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        // The folder is asked again: the pen has just written or deleted a file.
        ed.ink_files = notecraft_store::page::ink_files(&ed.dir);
        if Self::name_ink(ed) {
            Self::mark_dirty_from_blocks(ed);
        }
    }

    /// The page names every ink file in its folder, and names no ink file that is gone.
    ///
    /// The folder decides. A drawing is a file beside the page file; the line in the page
    /// that names it is how the page's words and what is drawn on it agree, and how
    /// another program opening the page sees the handwriting. Before this, the line could
    /// be lost - deleting the box that held it took it out - and nothing put it back until
    /// the pen next moved, so a page could say nothing about ink that was still there.
    ///
    /// Returns true when the page's text had to be changed.
    pub(super) fn name_ink(ed: &mut Editor) -> bool {
        use notecraft_formats::markdown::Block;
        let files = ed.ink_files.clone();
        let mut changed = false;
        // A line that names an ink file that is no longer in the folder goes.
        let before = ed.blocks.len();
        ed.blocks.retain(|b| match b {
            Block::Image { src, alt, .. } => !crate::inkpad::is_ink_line(src, alt) || files.iter().any(|f| f == src.trim()),
            _ => true,
        });
        changed |= ed.blocks.len() != before;
        // An ink file the page does not name is adopted: it is drawn, so it is named.
        for file in &files {
            if !ed.blocks.iter().any(|b| matches!(b, Block::Image { src, .. } if src.trim() == file)) {
                ed.blocks.push(Block::Image { alt: "Ink".to_string(), src: file.clone(), width: None, height: None });
                changed = true;
            }
        }
        if changed {
            ed.rebuild_body();
        }
        changed
    }

    /// Writes the rack and which tool is in hand into the settings, so the next run opens
    /// with the pens the owner arranged.
    pub(super) fn remember_rack(&mut self) {
        let line = notecraft_ink::rack::write(&self.rack);
        let _ = self.session.settings.set(RACK_KEY, &line);
        let _ = self.session.settings.set(NIB_KEY, &self.nib.to_string());
    }

    /// The pen as the page's ink layer wants it: what it is, and what the pointer is for.
    pub(super) fn pen(&self) -> crate::inkpad::Pen {
        let nib =
            self.rack.get(self.nib).or_else(|| self.rack.first()).copied().unwrap_or_else(|| notecraft_ink::rack::new_nib(notecraft_ink::Tool::Pen));
        crate::inkpad::Pen {
            pointer: self.pointer,
            tool: nib.tool,
            colour: nib.colour,
            // Keep the chosen nib width at the neutral setting, but let the top of the
            // control reach the same four-times full-width range as the pressure canvas.
            width: nib.width() * Self::sensitivity_width(self.sensitivity),
            gain: self.gain(),
        }
    }

    /// The pressure canvas uses a 4x full-width range at sensitivity 100. Keep 50 neutral
    /// so the rack's selected thickness remains the familiar baseline, and grow only the
    /// maximum width above it. Existing strokes retain the width stored when they were made.
    pub(super) fn sensitivity_width(sensitivity: u8) -> f32 {
        if sensitivity <= 50 { 1.0 } else { 1.0 + 3.0 * (f32::from(sensitivity - 50) / 50.0) }
    }

    /// The sensitivity as the curve a stroke is drawn with: the middle of the control is
    /// 1, which leaves the pen's readings exactly as they came.
    pub(super) fn gain(&self) -> f32 {
        let share = f32::from(self.sensitivity.min(100)) / 50.0;
        if share <= 1.0 {
            notecraft_ink::MIN_GAIN + (1.0 - notecraft_ink::MIN_GAIN) * share
        } else {
            1.0 + (notecraft_ink::MAX_GAIN - 1.0) * (share - 1.0)
        }
    }

    /// Takes the last stroke back out of the drawing on this page.
    pub(super) fn ink_redo(&mut self) {
        let Some(pad) = self.pad.as_mut() else { return };
        if !pad.redo() {
            self.say("There is no stroke to put back");
            return;
        }
        match pad.save() {
            Ok(()) => {}
            Err(e) => self.complain(&format!("Cannot write the drawing: {e}")),
        }
        self.sync_ink_reference();
    }

    pub(super) fn ink_undo(&mut self) {
        let Some(pad) = self.pad.as_mut() else {
            self.say("There is no drawing on this page");
            return;
        };
        if !pad.undo() {
            self.say("There is no stroke to take back");
            return;
        }
        let left = pad.strokes();
        match pad.save() {
            Ok(()) => self.say(format!("Took the last stroke back; {left} left")),
            Err(e) => self.complain(&format!("Cannot write the drawing: {e}")),
        }
        // The page stops naming ink it no longer has.
        self.sync_ink_reference();
    }

    /// Picks up a pen, which is all it takes: the whole page is the ink's, so there is no
    /// sheet to put anywhere first.
    pub(super) fn ink_insert(&mut self) {
        if self.editor.is_none() {
            self.say("Open a page first");
            return;
        }
        self.pointer = ribbon::Pointer::Draw;
        self.ribbon_tab = "Draw".to_string();
        self.say("Write anywhere on the page");
    }

    /// Writes the drawing again with no pen readings in it, for sending somewhere else.
    pub(super) fn ink_export_plain(&mut self) {
        let Some(pad) = self.pad.as_ref() else {
            self.say("There is no drawing on this page");
            return;
        };
        match pad.save_plain() {
            Ok(path) => {
                let shown = self.shown(&path);
                self.say(format!("Saved the drawing as a picture: {shown}"));
            }
            Err(e) => self.complain(&format!("Cannot write the picture: {e}")),
        }
    }

    /// Whether the caret is in a table, which is when the Table tab is offered.
    /// The Styles kind of the block the caret is in (or that a selected box is all of), for the ribbon to highlight; "" with neither.
    pub(super) fn caret_style(&self, ctx: &Context) -> &'static str {
        let Some(ed) = self.editor.as_ref() else { return "" };
        if let Some(block) = ed.caret.as_ref().and_then(|c| ed.blocks.get(c.block)) {
            return pageedit::block_kind(block);
        }
        // A selected box has everything in it selected: its style is the one they all share.
        let range = crate::containers::selected(ctx, &ed.dir).map(|i| notecraft_model::container::blocks_of(&ed.boxes, i)).unwrap_or_default();
        let mut kinds = range.filter_map(|i| ed.blocks.get(i)).map(pageedit::block_kind);
        match kinds.next() {
            Some(first) if kinds.all(|k| k == first) => first,
            _ => "",
        }
    }
}
