//! Marks, direction, lists and the clipboard on the page's words.

use super::*;

impl NoteCraftApp {
    /// Puts marks around the selection in the block the caret is in.
    pub(super) fn apply_mark(&mut self, mark: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_mut() else {
            self.status = "Click in the page first, then use the ribbon".into();
            self.status_is_error = false;
            return;
        };
        let marks = match mark {
            "bold" => "**",
            "italic" => "*",
            "strike" => "~~",
            _ => "`",
        };
        let len = caret.text.chars().count();
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        // All of the line's words: marked on the spans, so words that are partly marked already come out
        // right. A part of them is wrapped in the text, where the caret's own selection lives.
        if len > 0 && lo == 0 && hi == len && pageedit::toggle_mark(&mut ed.blocks, caret.block..caret.block + 1, mark) {
            if let Some(b) = ed.blocks.get(caret.block) {
                caret.text = pageedit::caret_text(b);
                caret.selection = (0, caret.text.chars().count());
            }
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
            return;
        }
        let (text, _) = pageedit::wrap_selection(&caret.text, caret.selection, marks);
        caret.text = text;
        if let Some(b) = ed.blocks.get_mut(caret.block) {
            pageedit::set_block_text(b, &caret.text);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
    }

    /// The keys Windows users expect for paragraph direction. Alt+Shift remains available
    /// to Windows for switching the active input language.
    pub(super) fn direction_keys(&mut self, ctx: &Context) {
        use notecraft_model::Dir;
        let ctrl_shift = Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE };
        let pressed = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        if pressed(ctrl_shift, Key::R) {
            self.apply_direction(Some(Dir::Rtl));
        }
        if pressed(ctrl_shift, Key::L) {
            self.apply_direction(Some(Dir::Ltr));
        }
    }

    /// Which way the line the caret is in reads. `None` lets its own words decide.
    pub(super) fn apply_direction(&mut self, dir: Option<notecraft_model::Dir>) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.status = "Click in the page first, then use the ribbon".into();
            self.status_is_error = false;
            return;
        };
        if !pageedit::set_direction(&mut ed.blocks, at, dir) {
            self.say("A direction goes on a paragraph of text");
            return;
        }
        ed.rebuild_body();
        if let Some(b) = ed.blocks.get(at) {
            let text = pageedit::caret_text(b);
            if let Some(c) = ed.caret.as_mut() {
                c.text = text;
            }
        }
        Self::mark_dirty_from_blocks(ed);
        self.say(match dir {
            Some(d) if d.is_rtl() => "This paragraph reads right to left",
            Some(_) => "This paragraph reads left to right",
            None => "This paragraph reads whichever way its words do",
        });
    }

    /// Fill the column with the paragraph the caret is in, or stop filling it.
    ///
    /// Which paragraphs are justified is the page's own, like the arrangement of its boxes
    /// and the width of its table columns, so it goes into the page's file and not into the
    /// window. The words are not touched: the stretching happens when the paragraph is
    /// drawn, so the file another program reads is the sentences the writer typed.
    pub(super) fn apply_justify(&mut self, strength: notecraft_model::justify::Strength) {
        let Some(ed) = self.editor.as_ref() else {
            self.say("Open a page first");
            return;
        };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.status = "Click in the page first, then use the ribbon".into();
            self.status_is_error = false;
            return;
        };
        if !matches!(ed.blocks.get(at), Some(notecraft_formats::Block::Paragraph(_))) {
            self.say("Justification goes on a paragraph of text");
            return;
        }
        // Asking for the strength a paragraph already has turns justification off, which
        // is how a menu of three gives a way back to a ragged edge without a fourth entry.
        let want = (ed.justified.strength_of(at) != Some(strength)).then_some(strength);
        let mut marked = ed.justified.clone();
        marked.set(at, want);
        self.write_justified(marked, want);
    }

    /// Writes which paragraphs are justified into the page's own file, and says what `want` now is.
    pub(super) fn write_justified(&mut self, marked: notecraft_model::justify::Justified, want: Option<notecraft_model::justify::Strength>) {
        let Some(ed) = self.editor.as_ref() else { return };
        let path = path_str(&ed.dir);
        let line = notecraft_model::justify::write(&marked);
        self.save_now();
        if self.run("page.justify", &params(&[("path", &path), ("blocks", &line)])).is_some() {
            if let Some(ed) = self.editor.as_mut() {
                ed.justified = marked;
            }
            self.say(match want {
                Some(notecraft_model::justify::Strength::Low) => "This paragraph fills the column, stretched lightly",
                Some(notecraft_model::justify::Strength::Medium) => "This paragraph fills the column",
                Some(notecraft_model::justify::Strength::High) => "This paragraph fills the column, stretched hard",
                None => "This paragraph is ragged again",
            });
        }
    }

    /// Which way the whole page reads. It is written into the page's own file.
    pub(super) fn apply_page_direction(&mut self, dir: Option<notecraft_model::Dir>) {
        let Some(ed) = self.editor.as_ref() else {
            self.say("Open a page first");
            return;
        };
        let path = path_str(&ed.dir);
        let want = dir.map(|d| d.id()).unwrap_or("auto");
        self.save_now();
        if self.run("page.direction", &params(&[("path", &path), ("dir", want)])).is_some() {
            if let Some(ed) = self.editor.as_mut() {
                ed.page_dir = dir;
            }
            self.say(match dir {
                Some(d) if d.is_rtl() => "This page reads right to left",
                Some(_) => "This page reads left to right",
                None => "Every line of this page decides for itself",
            });
        }
    }

    /// Which way the selection, or what is typed next, reads: an isolated run.
    pub(super) fn apply_embedding(&mut self, dir: notecraft_model::Dir) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_mut() else {
            self.status = "Click in the page first, then use the ribbon".into();
            self.status_is_error = false;
            return;
        };
        let (text, selection) = pageedit::isolate(&caret.text, caret.selection, dir);
        caret.text = text;
        caret.selection = selection;
        caret.place = Some(selection.1);
        if let Some(b) = ed.blocks.get_mut(caret.block) {
            pageedit::set_block_text(b, &caret.text);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        self.say(if dir.is_rtl() {
            "This run reads right to left. A mixed line is still drawn in one order; see the roadmap"
        } else {
            "This run reads left to right. A mixed line is still drawn in one order; see the roadmap"
        });
    }

    /// Makes the block the caret is in a heading, a list item, a quote or ordinary text.
    pub(super) fn apply_block_kind(&mut self, kind: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_ref() else {
            self.status = "Click in the page first, then use the ribbon".into();
            self.status_is_error = false;
            return;
        };
        let at = caret.block;
        if !pageedit::set_kind(&mut ed.blocks, at, kind) {
            return;
        }
        ed.rebuild_body();
        if let Some(b) = ed.blocks.get(at) {
            let text = pageedit::caret_text(b);
            if let Some(c) = ed.caret.as_mut() {
                c.text = text;
            }
        }
        Self::mark_dirty_from_blocks(ed);
    }

    /// Puts a tag on the line the caret is in, or takes it off again.
    pub(super) fn apply_tag(&mut self, tag: notecraft_model::Tag) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.status = "Click in the page first, then use the ribbon".into();
            self.status_is_error = false;
            return;
        };
        if !pageedit::set_tag(&mut ed.blocks, at, tag) {
            self.say("A tag goes on a line of text");
            return;
        }
        ed.rebuild_body();
        if let Some(b) = ed.blocks.get(at) {
            let text = pageedit::caret_text(b);
            if let Some(c) = ed.caret.as_mut() {
                c.text = text;
            }
        }
        Self::mark_dirty_from_blocks(ed);
        self.say(format!("{} tag", tag.label()));
    }

    /// Cut or copy what is selected in the block the caret is in.
    /// Ctrl+C, Ctrl+X, Ctrl+V and Ctrl+A for the words on a page.
    ///
    /// egui's own text box answers these for a paragraph it is drawing, and it is left to
    /// do so. Nothing answered them anywhere else, and that is a lot of places: not in a
    /// right-to-left paragraph, which NoteCraft draws itself; not for a selection that
    /// spans paragraphs, which no text box owns; and not when a box is picked or nothing
    /// has the keyboard at all. So Ctrl+C did nothing in Arabic, Ctrl+A selected nothing,
    /// and a box that had been copied could not be pasted into an empty one. The owner
    /// reported all three.
    ///
    /// The keys are read here and sent to the same `clipboard` and `paste_here` the ribbon
    /// buttons use, so there is one path and not two. Nothing is done when egui's text box
    /// is holding the keyboard, or both would act and the words would be pasted twice.
    pub(super) fn clipboard_keys(&mut self, ctx: &egui::Context) {
        let (copy, cut, paste, all) = ctx.input(|i| {
            let hit = |want: egui::Key| {
                // `key_pressed` is the platform-normalized view.  Some native
                // backends do not retain the raw key event in `events` after a
                // focused text widget has considered it, while the normalized
                // edge remains available for the rest of the frame.  Keep the
                // raw-event check as a fallback for the headless harness and
                // older egui backends.
                (i.key_pressed(want) && (i.modifiers.command || i.modifiers.ctrl))
                    || i.events.iter().any(|e| {
                        matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == want && (modifiers.command || modifiers.ctrl))
                    })
            };
            (hit(egui::Key::C), hit(egui::Key::X), hit(egui::Key::V), hit(egui::Key::A))
        });
        if !(copy || cut || paste || all) {
            return;
        }
        let ours = self.words_are_ours();
        if copy && ours {
            self.clipboard("copy");
        }
        if cut && ours {
            self.clipboard("cut");
        }
        // A screenshot has no text for egui's text widget to consume. Route every Ctrl+V
        // through the same paste path; it reads the image clipboard exactly once. Probing
        // with has_picture() first was racy with VirtualBox clipboard synchronisation and
        // could make the actual image read miss the clipboard handoff.
        if paste {
            let picture = self.clipboard.picture();
            if let (Some(picture), Some(dir)) = (picture, self.editor.as_ref().map(|e| e.dir.clone())) {
                self.paste_picture(&dir, picture);
            } else if ours || self.editor.as_ref().is_some_and(|e| e.caret.is_none()) {
                self.paste_here();
            }
        }
        if all && ours {
            self.select_all_in_block();
        }
    }

    /// True when the words the caret is in are drawn by NoteCraft rather than by an egui
    /// text box, so the keys above are ours to answer.
    pub(super) fn words_are_ours(&self) -> bool {
        let Some(ed) = self.editor.as_ref() else { return false };
        // A selection across paragraphs belongs to no text box at all.
        if ed.across.is_some_and(|a| !a.inside_one_block()) {
            return true;
        }
        let Some(caret) = ed.caret.as_ref() else { return false };
        // The same question `show_blocks` asks when it decides which painter draws the
        // block: a line with right-to-left in it is ours, and code and maths never are.
        pageedit::needs_our_caret(&caret.text)
            && !matches!(
                ed.blocks.get(caret.block),
                Some(notecraft_formats::markdown::Block::Code { .. }) | Some(notecraft_formats::markdown::Block::MathBlock(_))
            )
    }

    /// Ctrl+A: take the whole of the paragraph the caret is in.
    ///
    /// The paragraph and not the page. That is what a text box does, and it is the step
    /// OneNote takes first as well; pressing again there widens to the box and then to the
    /// page, which is not built here and is written down in `work\expected`.
    pub(super) fn select_all_in_block(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_mut() else { return };
        let end = caret.text.chars().count();
        caret.selection = (end, 0);
        caret.place = None;
        self.say("Paragraph selected");
    }

    pub(super) fn clipboard(&mut self, which: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        // A selection that runs across blocks is not any one block's, so it is taken from
        // the blocks themselves. Cut takes the words out and joins what is left.
        if let Some(open) = ed.across.filter(|a| !a.inside_one_block()) {
            let picked = pageedit::span_text(&ed.blocks, open);
            if picked.is_empty() {
                self.say("Select some words first");
                return;
            }
            let copied = pageedit::blocks_to_paste(&picked);
            self.pending_copy = Some(notecraft_formats::render::blocks_to_text(&copied));
            self.pending_copy_html = Some(notecraft_formats::render::blocks_to_html(&copied));
            if which == "cut" {
                let (block, at) = pageedit::delete_span(&mut ed.blocks, open);
                let text = ed.blocks.get(block).map(pageedit::caret_text).unwrap_or_default();
                ed.caret = Some(pageedit::Caret { block, text, focus: true, selection: (at, at), place: Some(at), cell: None, span: None });
                ed.across = None;
                ed.rebuild_body();
                Self::mark_dirty_from_blocks(ed);
            }
            self.say(if which == "cut" { "Cut" } else { "Copied" });
            return;
        }
        let Some(caret) = ed.caret.as_mut() else {
            self.status = "Click in the page first".into();
            self.status_is_error = false;
            return;
        };
        let (a, b) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        let picked: String = caret.text.chars().skip(a).take(b.saturating_sub(a)).collect();
        if picked.is_empty() {
            self.status = "Select some words first".into();
            self.status_is_error = false;
            return;
        }
        let copied = pageedit::blocks_to_paste(&picked);
        self.pending_copy = Some(notecraft_formats::render::blocks_to_text(&copied));
        self.pending_copy_html = Some(notecraft_formats::render::blocks_to_html(&copied));
        if which == "cut" {
            let (text, at) = pageedit::insert_at_caret(&caret.text, caret.selection, "");
            caret.text = text;
            caret.selection = (at, at);
            caret.place = Some(at);
            caret.focus = true;
            if let Some(b) = ed.blocks.get_mut(caret.block) {
                pageedit::set_block_text(b, &caret.text);
            }
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
        }
        self.say(if which == "cut" { "Cut" } else { "Copied" });
    }

    /// More or less indent on the list item the caret is in.
    pub(super) fn indent_block(&mut self, deeper: bool) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.status = "Click in the page first".into();
            self.status_is_error = false;
            return;
        };
        if !pageedit::indent(&mut ed.blocks, at, deeper) {
            self.say("Only a list item can be indented");
            return;
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
    }

    /// Takes the marks off the selection: bold, italic, strikethrough, code.
    pub(super) fn clear_formatting(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_mut() else { return };
        caret.text = pageedit::without_marks(&caret.text);
        caret.selection = (0, caret.text.chars().count());
        if let Some(b) = ed.blocks.get_mut(caret.block) {
            pageedit::set_block_text(b, &caret.text);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
    }

    /// Puts a whole block (a table, a diagram) after the caret.
    pub(super) fn insert_block(&mut self, what: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        let at = ed.caret.as_ref().map(|c| c.block + 1).unwrap_or(ed.blocks.len());
        if !pageedit::insert_block(&mut ed.blocks, at, what) {
            self.say("That is not something a page can hold");
            return;
        }
        ed.rebuild_body();
        ed.caret = ed.blocks.get(at).map(|b| pageedit::Caret {
            block: at,
            text: pageedit::caret_text(b),
            focus: true,
            selection: (0, 0),
            place: None,
            cell: None,
            span: None,
        });
        Self::mark_dirty_from_blocks(ed);
    }

    /// Puts a snippet where the caret is, or at the end of the page.
    pub(super) fn insert_in_page(&mut self, text: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        match ed.caret.as_mut() {
            Some(caret) => {
                // Where the caret is, not at the end of the block.
                let (new_text, at) = pageedit::insert_at_caret(&caret.text, caret.selection, text.trim_matches('\n'));
                caret.text = new_text;
                caret.selection = (at, at);
                caret.place = Some(at);
                caret.focus = true;
                if let Some(b) = ed.blocks.get_mut(caret.block) {
                    pageedit::set_block_text(b, &caret.text);
                }
                ed.rebuild_body();
                Self::mark_dirty_from_blocks(ed);
            }
            None => {
                let cursor = ed.body.chars().count();
                self.cursor = cursor;
                self.insert(text);
            }
        }
    }
}
