//! Opening, saving and changing the page that is open.

use super::*;

impl NoteCraftApp {
    pub(super) fn open_page(&mut self, dir: &Path) {
        if let Some(notebook) = notebook_for(dir) {
            self.remember_recent_notebook(&notebook);
        }
        self.nav.visit(dir);
        self.open_page_inner(dir);
    }

    /// Opens a page without counting it as a visit: what Back and Forward do.
    pub(super) fn open_page_inner(&mut self, dir: &Path) {
        self.save_now();
        self.code = crate::codeview::State::default();
        if let Some(Output::Page(page)) = self.run("page.load", &params(&[("path", &path_str(dir))])) {
            let (blocks, in_the_text) = notecraft_formats::boxes::read(page.format, &page.body);
            let older = notecraft_store::page::containers(dir).unwrap_or_default();
            let arranged = in_the_text.is_some() || !older.is_empty();
            let mut boxes = in_the_text.unwrap_or(older);
            // A never-arranged blank page still gets one empty writing box. A
            // page whose last box was deliberately deleted remains boxless so
            // its open paper can create a new box on the next click.
            if blocks.is_empty() && !arranged && boxes.is_empty() {
                boxes.push(notecraft_model::Container::new(0.0, 0.0, 360.0, 0));
            }
            // The page as it was opened is the first step of its history: undo goes back
            // this far and no further.
            let first = (page.body.clone(), blocks.clone(), boxes.clone());
            // The paper this page is drawn on and the colour behind it are the page's own.
            let appearance = notecraft_store::page::appearance(dir)
                .map(|(p, c)| (p.and_then(|p| ribbon::RuleLines::parse(&p)), c.and_then(|c| notecraft_model::NodeColor::parse(&c))))
                .unwrap_or((None, None));
            let attachments = notecraft_store::assets::list(dir);
            let page_body = page.body.clone();
            let writable = pageedit::round_trips(page.format, &page.body);
            // A page with no title of its own starts with a blank title area, not with
            // "Untitled page" sitting there as if it had been typed. `title` keeps the
            // display fallback, so losing focus on an empty box is correctly seen as "no
            // change" rather than a rename to the fallback word itself.
            let title_buf = if page.has_title { page.title.clone() } else { String::new() };
            self.editor = Some(Editor {
                dir: dir.to_path_buf(),
                format: page.format,
                title_buf,
                title: page.title,
                body: page.body,
                dirty_since: None,
                blocks,
                last_saved: None,
                attachments: attachments.clone(),
                caret: None,
                writable,
                page_dir: page.dir,
                boxes,
                arranged,
                columns: notecraft_store::page::table_columns(dir).unwrap_or_default(),
                justified: notecraft_store::page::justified(dir).unwrap_or_default(),
                across: None,
                ink_files: notecraft_store::page::ink_files(dir),
                paper: appearance.0,
                colour: appearance.1,
                sheet_a4: sheet_layout(dir).0,
                sheets: sheet_layout(dir).1,
                sheets_drawn: 0,
                sheets_to_save: false,
                history: crate::history::History::new(crate::history::Step::new(first.0, first.1, first.2, None, crate::history::Kind::Edit)),
                origin: egui::Pos2::ZERO,
            });
            if !self.icon_recovery_suppressed
                && let Some(name) = first_missing_attachment_in_source(&page_body, dir).or_else(|| first_legacy_attachment_in_source(&page_body))
            {
                let key = format!("{}::{name}", dir.display());
                log::warn!("attachment recovery scan found missing icon: page={:?} attachment={name:?}", dir);
                // Do not let page recovery replace the first-use privacy dialog. The normal
                // frame pass below will offer recovery after the user answers that dialog.
                if self.dialog.is_none() && self.icon_recovery_reported.insert(key) {
                    self.dialog = Some(Dialog::MissingAttachmentIcon { page: dir.to_path_buf(), name });
                }
            } else {
                log::debug!("attachment recovery scan found no missing icons: page={:?} attachments={}", dir, attachments.len());
            }
            if let Some(old) = self.pad.as_mut() {
                old.flush();
            }
            self.pad = Some(crate::inkpad::Pad::of_page(dir));
            self.absorb_old_sheets(dir);
            self.sync_ink_reference();
            self.selected = Some(dir.to_path_buf());
            self.cursor = 0;
            self.remember_last_page(dir);
            self.say("Opened");
        }
    }

    /// A page written by an earlier version holds its drawing as a picture pointing at
    /// `assets/drawing-1.svg`. Nothing draws those now, so the page showed a broken
    /// picture. Their strokes are moved onto the page's own ink and the picture is taken
    /// out of the words; the old file is left in the folder untouched.
    pub(super) fn absorb_old_sheets(&mut self, dir: &Path) {
        use notecraft_formats::markdown::Block;
        let olds: Vec<String> = self
            .editor
            .as_ref()
            .map(|e| {
                e.blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::Image { src, .. } if crate::inkpad::is_old_sheet(src) => Some(src.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        if olds.is_empty() {
            return;
        }
        let mut moved = 0;
        if let Some(pad) = self.pad.as_mut() {
            for src in &olds {
                moved += pad.absorb(dir, src);
            }
            if pad.dirty && pad.save().is_err() {
                self.complain("Cannot write this page's ink");
                return;
            }
        }
        if let Some(ed) = self.editor.as_mut() {
            ed.blocks.retain(|b| !matches!(b, Block::Image { src, .. } if crate::inkpad::is_old_sheet(src)));
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
        }
        self.sync_ink_reference();
        // The old file has been read and its strokes are in `ink.svg`, so leaving it
        // behind only puts a dead 200 KB picture in the page's folder for the owner to
        // wonder about. It goes, now that what was in it is kept somewhere better.
        let mut left = 0;
        for src in &olds {
            let file = dir.join(src.trim().replace('/', std::path::MAIN_SEPARATOR_STR));
            if std::fs::remove_file(&file).is_err() {
                left += 1;
            }
        }
        if left > 0 {
            self.say(format!("Moved {moved} strokes onto the page; {left} old drawing files could not be removed"));
        } else {
            self.say(format!("Moved {moved} strokes from this page's old drawing onto the page itself"));
        }
    }

    pub(super) fn save_now(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        if ed.dirty_since.is_none() {
            return;
        }
        let (dir, body) = (ed.dir.clone(), ed.body.clone());
        match self.session.run("page.save", &params(&[("path", &path_str(&dir)), ("body", &body)])) {
            Ok(Output::Text(ts)) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.dirty_since = None;
                    ed.last_saved = Some(ts);
                }
                self.say("Saved");
            }
            Ok(_) => {}
            Err(e) => self.fail(&e),
        }
    }

    pub(super) fn repair_attachment_reference(&mut self, page: &Path, name: &str) -> Result<bool, String> {
        let Some(ed) = self.editor.as_mut() else { return Ok(false) };
        if ed.dir != page {
            return Err("the page changed while the recovery dialog was open".into());
        }
        let link = format!("assets/{name}");
        let html_href = format!("href=\"{link}\"");
        let markdown_href = format!("]({link})");
        let references = ed.body.matches(&html_href).count() + ed.body.matches(&markdown_href).count();
        log::debug!("attachment recovery reference scan: page={:?} attachment={name:?} references={references}", page);
        let old = match ed.format {
            PageFormat::Html => format!("<p><a href=\"{link}\">{name}</a></p>"),
            PageFormat::Cmgfm => format!("[{name}]({link})"),
        };
        if references == 0 && !ed.body.contains(&old) {
            let icon_link = attachment_icon_link(page, name);
            let replacement = replace_attachment_icon(ed.format, &snippets::attachment(ed.format, &link, name), name, &icon_link).trim().to_string();
            ed.body.push('\n');
            ed.body.push_str(&replacement);
            Self::mark_dirty(ed);
            log::debug!("attachment recovery appended missing reference: page={:?} attachment={name:?}", page);
            return Ok(true);
        }
        let icon_link = attachment_icon_link(page, name);
        let replacement = replace_attachment_icon(ed.format, &snippets::attachment(ed.format, &link, name), name, &icon_link).trim().to_string();
        let mut body = ed.body.replace(&old, &replacement);
        if references > 1 {
            let mut kept = false;
            body = body
                .lines()
                .filter(|line| {
                    let card = match ed.format {
                        PageFormat::Html => line.contains(&html_href) && line.contains("<img"),
                        PageFormat::Cmgfm => line.contains(&markdown_href) && line.contains("!["),
                    };
                    if card {
                        if kept {
                            return false;
                        }
                        kept = true;
                    }
                    true
                })
                .collect::<Vec<_>>()
                .join("\n");
            log::warn!("attachment recovery removed duplicate references: page={:?} attachment={name:?} references={references}", page);
        }
        // Migrate an older generated icon in place when the platform now has a native icon.
        #[cfg(target_os = "windows")]
        {
            body = body.replace(&format!("assets/{name}.svg"), &icon_link).replace(&format!("assets/{name}.png"), &icon_link);
        }
        if body == ed.body {
            return Ok(false);
        }
        ed.body = body;
        Self::mark_dirty(ed);
        Ok(true)
    }

    /// The page text changed: remember to save it, and read the blocks from it again.
    pub(super) fn mark_dirty(ed: &mut Editor) {
        ed.dirty_since = Some(Instant::now());
        let (blocks, in_the_text) = notecraft_formats::boxes::read(ed.format, &ed.body);
        ed.blocks = blocks;
        ed.arranged = in_the_text.is_some();
        ed.boxes = in_the_text.unwrap_or_default();
        ed.caret = None;
    }

    /// The blocks changed (the caret is in the page): remember to save, and keep the blocks
    /// as they are. Reading them back from the text would throw away an empty new block and
    /// move the caret out from under the user.
    pub(super) fn mark_dirty_from_blocks(ed: &mut Editor) {
        ed.dirty_since = Some(Instant::now());
    }

    pub(super) fn insert(&mut self, text: &str) {
        let cursor = self.cursor;
        if let Some(ed) = self.editor.as_mut() {
            let (body, next) = snippets::insert_at(&ed.body, cursor, text);
            ed.body = body;
            Self::mark_dirty(ed);
            self.cursor = next;
        }
    }

    /// A dropped file asks what to do with it, the way OneNote asks when you insert one.
    pub(super) fn attach_dropped(&mut self, ctx: &Context) {
        let files: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if files.is_empty() {
            return;
        }
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
            self.status = "Open a page first, then drop files onto it.".into();
            self.status_is_error = true;
            return;
        };
        self.dialog = Some(Dialog::FileOptions { dir, files });
    }

    /// Carries out one of the choices of the file dialog.
    pub(super) fn add_file(&mut self, dir: &Path, file: &Path, how: FileChoice) {
        let Some(format) = self.editor.as_ref().map(|e| e.format) else { return };
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match how {
            FileChoice::Copy => {
                log::debug!("file attach requested: name={name:?} page={:?}", dir);
                match self.run("page.attach", &params(&[("path", &path_str(dir)), ("source", &path_str(file))])) {
                    Some(Output::Text(link)) => {
                        log::debug!("file attach copied: name={name:?} link={link:?}");
                        let text = snippets::attachment(format, &link, &name);
                        self.insert(&text);
                        self.say(format!("{name} is now a file in this page's folder"));
                    }
                    Some(_) => self.complain("The file was not attached: the attach command returned an unexpected result."),
                    None => {
                        log::error!("file attach failed: name={name:?} page={:?} status={:?}", dir, self.status);
                        self.complain(&format!("Could not attach {name} to this page."));
                    }
                }
            }
            FileChoice::Link => {
                let text = snippets::file_link(format, &path_str(file), &name);
                self.insert(&text);
                self.say(format!("Linked {name} where it is; the file was not copied"));
            }
            FileChoice::Printout => self.insert_printout(dir, file, format),
        }
        if let Some(ed) = self.editor.as_mut() {
            ed.attachments = notecraft_store::assets::list(&ed.dir);
        }
    }

    pub(super) fn insert_printout(&mut self, dir: &Path, file: &Path, format: PageFormat) {
        let rendered = match printout::render(file) {
            Ok(rendered) => rendered,
            Err(error) => {
                self.complain(&error);
                return;
            }
        };
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let Some(Output::Text(link)) = self.run("page.attach", &params(&[("path", &path_str(dir)), ("source", &path_str(file))])) else {
            self.complain(&format!("Could not attach {name} before rendering its pages"));
            return;
        };
        let Some(base) = link.strip_prefix("assets/").and_then(|link| Path::new(link).file_name()).map(|name| name.to_string_lossy().into_owned())
        else {
            self.complain("The attachment returned an invalid page link");
            return;
        };
        let assets = dir.join("assets");
        let mut generated = Vec::with_capacity(rendered.pages().len());
        for (index, page) in rendered.pages().iter().enumerate() {
            let target = assets.join(format!("{base}.printout-{:03}.png", index + 1));
            if let Err(error) = std::fs::copy(page, &target) {
                for path in &generated {
                    let _ = std::fs::remove_file(path);
                }
                self.complain(&format!("Could not store printout page {}: {error}", index + 1));
                return;
            }
            generated.push(target);
        }
        let mut text = snippets::attachment(format, &link, &name);
        for (index, _) in generated.iter().enumerate() {
            let image = format!("assets/{base}.printout-{:03}.png", index + 1);
            text.push_str(&snippets::printout_page(format, &image, index + 1));
        }
        self.insert(&text);
        self.say(format!("Inserted {name} and {} printout page(s)", generated.len()));
    }

    /// Removes what has been in a notebook's recycle bin longer than the setting allows. Looked at once when the
    /// window starts and then every hour, so a window left open for days still empties its bins.
    pub(super) fn sweep_recycle_bins(&mut self) {
        if self.bins_swept.is_some_and(|at| at.elapsed() < Duration::from_secs(3600)) {
            return;
        }
        self.bins_swept = Some(Instant::now());
        if self.recycle_days == 0 {
            return;
        }
        let days = self.recycle_days.to_string();
        let notebooks: Vec<PathBuf> = self.session.tree().iter().map(|n| n.path.clone()).collect();
        let mut gone = 0;
        for nb in notebooks {
            if let Some(Output::Count(n)) = self.run("recycle.empty", &params(&[("notebook", &path_str(&nb)), ("older_than_days", &days)])) {
                gone += n;
            }
        }
        if gone > 0 {
            log::info!("recycle bins: {gone} items older than {days} days removed");
        }
    }

    /// Opens the page that was open when the window last closed, and the branches above it,
    /// the way OneNote comes back where you left it. Runs once, on the first frame.
    pub(super) fn restore_last_page(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        let Some(saved) = self.session.settings.get("last_page") else { return };
        let path = self.session.root().join(saved.trim().replace('/', std::path::MAIN_SEPARATOR_STR));
        if !path.is_dir() {
            return;
        }
        // Open every container above it, so the page is visible in the tree.
        let mut at = path.parent();
        while let Some(dir) = at {
            if !dir.starts_with(self.session.root()) || dir == self.session.root() {
                break;
            }
            self.open.insert(dir.to_path_buf());
            at = dir.parent();
        }
        self.open_page(&path);
    }

    /// Remembers which page is open, for the next time the window starts.
    pub(super) fn remember_last_page(&mut self, dir: &Path) {
        let Ok(rel) = dir.strip_prefix(self.session.root()) else { return };
        let value = rel.to_string_lossy().replace('\\', "/");
        let _ = self.session.settings.set("last_page", &value);
    }

    /// Writes a file of the open page that was typed in in the code view.
    ///
    /// Only the files the code view lets anybody type in reach here: a drawing's SVG. The
    /// page file's body is not one of them; it is the editor's own text and is saved the
    /// way it always was.
    pub(super) fn write_page_file(&mut self, name: &str, text: &str) {
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else { return };
        let is_page_file = self.editor.as_ref().is_some_and(|e| e.format.page_file() == name);
        let path = dir.join(name.replace('/', std::path::MAIN_SEPARATOR_STR));
        if !path.starts_with(&dir) {
            self.complain("That file is not in this page's folder");
            return;
        }
        if let Err(e) = notecraft_store::page::write_atomic(&path, text) {
            self.complain(&format!("Cannot write {name}: {e}"));
            return;
        }
        if is_page_file {
            // The page itself was typed in. Nothing the window remembered about it is
            // true any more, and the autosave must not write its old copy over this one;
            // the page is read again when the view closes.
            if let Some(ed) = self.editor.as_mut() {
                ed.dirty_since = None;
                ed.body = notecraft_store::page::load_page(&dir).map(|p| p.body).unwrap_or_else(|_| ed.body.clone());
            }
            self.code.page_edited = true;
            return;
        }
        // A drawing that was typed in is read again, so what the pen does next is done to
        // the file as it now stands rather than to the one the window remembered.
        if notecraft_ink::svg::is_ink(text) {
            if let Some(old) = self.pad.as_mut() {
                old.flush();
            }
            self.pad = Some(crate::inkpad::Pad::of_page(&dir));
        }
        self.sync_ink_reference();
    }

    /// Gives the open page its own paper, its own colour, or both, and writes them into
    /// the page's file. `None` for either leaves that one as it is.
    pub(super) fn set_page_appearance(&mut self, paper: Option<ribbon::RuleLines>, colour: Option<Option<notecraft_model::NodeColor>>) {
        let Some(ed) = self.editor.as_mut() else {
            self.say("Open a page first");
            return;
        };
        if let Some(p) = paper {
            ed.paper = Some(p);
        }
        if let Some(c) = colour {
            ed.colour = c;
        }
        let (path, paper, colour) = (path_str(&ed.dir), ed.paper, ed.colour);
        let pairs: Vec<(&str, &str)> = vec![
            ("path", path.as_str()),
            ("paper", paper.map(ribbon::RuleLines::id).unwrap_or("none")),
            ("color", colour.map(notecraft_model::NodeColor::id).unwrap_or("none")),
        ];
        self.run("page.paper", &params(&pairs));
    }

    /// Takes the page back to how it was before the last change, or forward again.
    ///
    /// The whole page is put back: its text, its blocks, the boxes they sit in and where
    /// the caret was. Ink is not part of this; a stroke is taken back with Undo Stroke on
    /// the Draw tab, because the drawing is its own file beside the page.
    pub(super) fn step_back(&mut self, back: bool) {
        let mut nothing = false;
        if let Some(ed) = self.editor.as_mut() {
            if !ed.writable {
                nothing = true;
            } else {
                let step = if back { ed.history.undo().cloned() } else { ed.history.redo().cloned() };
                match step {
                    Some(step) => {
                        ed.body = step.body;
                        ed.blocks = step.blocks;
                        ed.boxes = step.boxes;
                        ed.caret = step.caret.map(|c| pageedit::Caret { focus: true, ..c });
                        ed.dirty_since = Some(Instant::now());
                        // The restored text says whether the page was arranged.
                        ed.arranged = notecraft_formats::boxes::has_boxes(ed.format, &ed.body);
                    }
                    None => nothing = true,
                }
            }
        }
        if nothing {
            self.say(if back { "Nothing to undo" } else { "Nothing to redo" });
        }
    }

    /// Remembers the page as it stands now, so the change that just happened can be taken
    /// back. Called once a frame, after whatever changed has been applied.
    ///
    /// Letters going into the block the caret was already in fold into the step before
    /// them, so Ctrl+Z takes back a run of writing rather than one character.
    pub(super) fn remember_step(ed: &mut Editor) {
        let same = ed.history.here().is_some_and(|s| s.body == ed.body && s.blocks == ed.blocks && s.boxes == ed.boxes);
        // The body alone changed: the page was written out and its text now says where the boxes are.
        // That is the same step, with its text brought up to date, not another one to undo.
        if !same && ed.history.here().is_some_and(|s| s.blocks == ed.blocks && s.boxes == ed.boxes) {
            ed.history.refresh_body(ed.body.clone());
            return;
        }
        if same {
            // Nothing really changed, or the page was just put back by undo itself.
            return;
        }
        let here = ed.caret.as_ref().map(|c| c.block);
        let was = ed.history.here().and_then(|s| s.caret.as_ref().map(|c| c.block));
        let shape = ed.history.here().is_some_and(|s| s.blocks.len() == ed.blocks.len());
        let moved = ed.history.here().is_some_and(|s| s.blocks == ed.blocks && s.boxes != ed.boxes);
        let kind = match (shape, here, was) {
            _ if moved => crate::history::Kind::Move,
            (true, Some(a), Some(b)) if a == b => crate::history::Kind::Typing(a),
            _ => crate::history::Kind::Edit,
        };
        let step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), kind);
        ed.history.record(step);
    }

    /// The paper the open page is drawn on: its own, or the window's choice when it has
    /// none of its own.
    pub(super) fn paper_now(&self) -> ribbon::RuleLines {
        self.editor.as_ref().and_then(|e| e.paper).unwrap_or(self.rules)
    }
}
