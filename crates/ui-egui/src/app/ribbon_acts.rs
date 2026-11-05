//! What the ribbon asks for.

use super::*;

impl NoteCraftApp {
    /// The ribbon under the title row: tabs and their groups.
    pub(super) fn ribbon_bar(&mut self, ui: &mut Ui) {
        let (tab, source, tokens) = (self.ribbon_tab.clone(), self.show_source, self.tokens);
        if tab == "Draw" && !self.stylus.sample().pen && !self.pressure_warning_presented {
            self.pressure_warning_presented = true;
            self.alert(
                PRESSURE_WARNING_KEY,
                "Pressure sensitivity unavailable",
                "The selected pen is not reporting Windows Ink pressure. Pressure sensitivity will be disabled. Enable Windows Ink and disable Mouse mode in the tablet driver's settings.",
            );
        }
        let view = ribbon::View {
            split_panes: self.split_panes,
            always_on_top: self.always_on_top,
            full_page: self.full_page,
            rules: self.rules,
            hide_title: self.hide_title,
            sheet_a4: self.editor.as_ref().is_some_and(|e| e.sheet_a4),
            zoom_percent: self.zoom_percent,
            pointer: self.pointer,
            rack: &self.rack,
            nib: self.nib,
            in_table: self.caret_in_table(),
            table_borders: self.table_borders,
            gridcraft: self.gridcraft.is_some(),
            page_colour: self.editor.as_ref().and_then(|e| e.colour),
            can_undo: self.editor.as_ref().is_some_and(|e| e.history.can_undo()),
            can_redo: self.editor.as_ref().is_some_and(|e| e.history.can_redo()),
            ink_redo: self.pad.as_ref().is_some_and(|p| p.can_redo()),
            sensitivity: self.sensitivity,
            pressure_available: self.stylus.sample().pen,
            has_page: self.editor.is_some(),
            style: self.caret_style(ui.ctx()),
        };
        let format = self.editor.as_ref().map(|e| e.format);
        let mut acts = Vec::new();
        if self.full_page && self.editor.is_some() {
            // Full page view: a strip of the pen tools stands in for the ribbon.
            egui::Panel::top("quick-draw")
                .exact_size(ribbon::QUICK_BAR_H)
                .frame(egui::Frame::new().fill(tokens.panel).inner_margin(egui::Margin::symmetric(4, 0)))
                .show(ui, |ui| {
                    acts = ribbon::quick_draw_bar(ui, &view, &tokens);
                });
            for a in acts {
                self.ribbon_act(a);
            }
            return;
        }
        if self.compact_ui && ribbon::resolve_tab(&tab, source, view.in_table) == "Home" {
            let acts = {
                let mut acts = Vec::new();
                egui::Panel::top("toolbar")
                    .exact_size(48.0)
                    .frame(egui::Frame::new().fill(tokens.panel).inner_margin(egui::Margin::symmetric(4, 0)))
                    .show(ui, |ui| {
                        acts = ribbon::compact_toolbar(ui, &view, &tokens);
                    });
                acts
            };
            for a in acts {
                self.ribbon_act(a);
            }
            return;
        }
        if self.ribbon_collapsed {
            return;
        }
        egui::Panel::top("ribbon")
            .exact_size(ribbon::GROUP_H + 8.0)
            .frame(egui::Frame::new().fill(tokens.panel).inner_margin(egui::Margin::symmetric(4, 0)))
            .show_separator_line(false)
            .show(ui, |ui| {
                acts = ribbon::show(ui, &tab, source, &view, format, &tokens);
            });
        let picked = self.editor.as_ref().and_then(|e| crate::containers::selected(ui.ctx(), &e.dir));
        for a in acts {
            if let Some(i) = picked
                && self.act_on_box(ui.ctx(), i, &a)
            {
                continue;
            }
            self.ribbon_act(a);
        }
    }

    /// What a ribbon button does while a container is selected: selecting a box selects everything in
    /// it, so Cut, Copy, the marks and the styles take all of it. True when the act was one of those.
    pub(super) fn act_on_box(&mut self, ctx: &Context, at: usize, act: &ribbon::Act) -> bool {
        use notecraft_formats::markdown::Block;
        let wanted = matches!(
            act,
            ribbon::Act::Clipboard(_)
                | ribbon::Act::Mark(_)
                | ribbon::Act::Block(_)
                | ribbon::Act::ClearFormatting
                | ribbon::Act::Direction(_)
                | ribbon::Act::Indent(_)
                | ribbon::Act::Tag(_)
                | ribbon::Act::Justify(_)
        );
        let Some(ed) = self.editor.as_mut().filter(|_| wanted) else { return false };
        let range = notecraft_model::container::blocks_of(&ed.boxes, at);
        if range.end > ed.blocks.len() || range.is_empty() {
            return false;
        }
        let words = |b: &Block| !matches!(b, Block::Code { .. } | Block::MathBlock(_)) && pageedit::is_one_line_of_words(b);
        match act {
            ribbon::Act::Clipboard(which) => {
                crate::containers::ask_clipboard(ctx, *which == "cut");
                return true;
            }
            ribbon::Act::Mark(mark) => {
                if !pageedit::toggle_mark(&mut ed.blocks, range, mark) {
                    return true;
                }
            }
            ribbon::Act::Direction(dir) => {
                // Words only: a mark in a diagram or an equation would be part of its source.
                for i in range {
                    if ed.blocks.get(i).is_some_and(words) {
                        pageedit::set_direction(&mut ed.blocks, i, *dir);
                    }
                }
            }
            ribbon::Act::Indent(deeper) => {
                for i in range {
                    pageedit::indent(&mut ed.blocks, i, *deeper);
                }
            }
            ribbon::Act::Tag(tag) => {
                // All of them have the tag already: the same button takes it off every one.
                let has = |b: &Block| pageedit::tag_of(b).0 == Some(*tag);
                // A to-do line carries no other tag, so it is left out of every tag but its own.
                let can = |b: &Block| words(b) && (*tag == notecraft_model::Tag::ToDo || !matches!(b, Block::Item { task: Some(_), .. }));
                let all = range.clone().filter_map(|i| ed.blocks.get(i)).filter(|b| can(b)).all(has);
                for i in range {
                    if ed.blocks.get(i).is_some_and(|b| can(b) && (all || !has(b))) {
                        pageedit::set_tag(&mut ed.blocks, i, *tag);
                    }
                }
            }
            ribbon::Act::Justify(strength) => {
                let paragraphs: Vec<usize> = range.filter(|i| matches!(ed.blocks.get(*i), Some(Block::Paragraph(_)))).collect();
                if paragraphs.is_empty() {
                    return true;
                }
                let all = paragraphs.iter().all(|i| ed.justified.strength_of(*i) == Some(*strength));
                let want = (!all).then_some(*strength);
                let mut marked = ed.justified.clone();
                for i in paragraphs {
                    marked.set(i, want);
                }
                self.write_justified(marked, want);
                return true;
            }
            ribbon::Act::Block(kind) => {
                // All of them are that kind already: the same button turns them back into text.
                let all = range.clone().all(|i| ed.blocks.get(i).is_some_and(|b| pageedit::block_kind(b) == *kind));
                for i in range {
                    if all || ed.blocks.get(i).is_some_and(|b| pageedit::block_kind(b) != *kind) {
                        pageedit::set_kind(&mut ed.blocks, i, kind);
                    }
                }
            }
            _ => {
                for i in range {
                    if let Some(b) = ed.blocks.get_mut(i).filter(|b| words(b)) {
                        let text = pageedit::without_marks(&pageedit::block_text(b));
                        pageedit::set_block_text(b, &text);
                    }
                }
            }
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        true
    }

    /// Carries out one thing the ribbon asked for.
    pub(super) fn ribbon_act(&mut self, act: ribbon::Act) {
        match act {
            ribbon::Act::File => self.backstage = Some("Info"),
            ribbon::Act::PageVersions => {
                self.save_now();
                if let Some(page) = self.editor.as_ref().map(|e| e.dir.clone()) {
                    let p = params(&[("path", &path_str(&page))]);
                    if let Some(Output::Versions(versions)) = self.run("page.versions", &p) {
                        self.dialog = Some(Dialog::PageVersions { page, versions });
                    }
                }
            }
            ribbon::Act::RecycleBin => {
                if let Some(notebook) = self.editor.as_ref().and_then(|e| notebook_for(&e.dir)) {
                    let p = params(&[("notebook", &path_str(&notebook))]);
                    if let Some(Output::Recycled(items)) = self.run("recycle.list", &p) {
                        self.dialog = Some(Dialog::RecycleBin { notebook, items });
                    }
                }
            }
            ribbon::Act::Symbols => self.dialog = Some(Dialog::Symbols),
            ribbon::Act::Equation(tex) => self.insert_in_page(&format!("${tex}$")),
            ribbon::Act::ToggleRibbon => self.ribbon_collapsed = !self.ribbon_collapsed,
            ribbon::Act::Tab(name) => {
                self.ribbon_tab = name;
                // Picking a tab unfolds the ribbon.
                self.ribbon_collapsed = false;
                // A tab is a way back to the ribbon.
                self.full_page = false;
            }
            ribbon::Act::Mark(mark) => self.apply_mark(mark),
            ribbon::Act::Block(kind) => self.apply_block_kind(kind),
            ribbon::Act::Insert(what) => match what {
                "space" => {
                    let Some(ed) = self.editor.as_mut() else { return };
                    let at = ed.caret.as_ref().map(|c| c.block + 1).unwrap_or(ed.blocks.len()).min(ed.blocks.len());
                    ed.blocks.insert(at, notecraft_formats::Block::Paragraph(Vec::new()));
                    ed.rebuild_body();
                    ed.caret =
                        Some(pageedit::Caret { block: at, text: String::new(), focus: true, selection: (0, 0), place: None, cell: None, span: None });
                    Self::mark_dirty_from_blocks(ed);
                }
                "link" => self.insert_in_page("[text](https://)"),
                "equation" => self.insert_in_page("$E = mc^2$"),
                "date" | "time" | "datetime" => {
                    let now = notecraft_model::time::iso_now();
                    let stamp = match what {
                        "time" => notecraft_model::time::clock_from_iso(&now),
                        "datetime" => notecraft_model::time::long_from_iso(&now),
                        _ => notecraft_model::time::date_from_iso(&now),
                    };
                    self.insert_in_page(&stamp.unwrap_or(now));
                }
                other => self.insert_block(other),
            },
            ribbon::Act::AddPrintout => {
                let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
                    self.say("Open a page first");
                    return;
                };
                let kinds = ["pdf", "doc", "docx", "ppt", "pptx", "xls", "xlsx", "odt", "odp", "ods", "rtf"];
                let title = "Choose a PDF or Office document to print out";
                if let Some(file) = self.pick(Purpose::Printout(dir.clone()), title, false, &kinds, None) {
                    self.add_file(&dir, &file, FileChoice::Printout);
                }
            }
            ribbon::Act::AddFile | ribbon::Act::AddPicture => {
                let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
                    self.say("Open a page first");
                    return;
                };
                if self.media.is_some() && (matches!(act, ribbon::Act::AddPicture) || self.files.is_none()) {
                    self.dialog = Some(Dialog::PickMedia { dir, picker: crate::mediapicker::Picker::new(matches!(act, ribbon::Act::AddPicture)) });
                    return;
                }
                let picture = matches!(act, ribbon::Act::AddPicture);
                let title = if picture { "Choose a picture" } else { "Choose a file to attach" };
                let kinds: &[&str] = if picture { &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"] } else { &[] };
                if let Some(file) = self.pick(Purpose::AttachFile(dir.clone()), title, false, kinds, None) {
                    self.dialog = Some(Dialog::FileOptions { dir, files: vec![file] });
                }
            }
            ribbon::Act::ToggleSource => self.set_show_source(!self.show_source),
            ribbon::Act::ToggleFullPage => self.full_page = !self.full_page,
            ribbon::Act::OpenThemes => self.theme_window = !self.theme_window,
            ribbon::Act::RuleLines(r) => {
                // The paper is the page's own, as OneNote's rule lines are. The window's
                // own setting stays as what a page with none of its own is drawn on.
                self.rules = r;
                let _ = self.session.settings.set("rule_lines", r.id());
                if self.editor.is_some() {
                    self.set_page_appearance(Some(r), None);
                }
                self.say(match r {
                    ribbon::RuleLines::None => "No lines behind the page".to_string(),
                    other => format!("{} behind the page", other.label()),
                });
            }
            ribbon::Act::ToggleAlwaysOnTop => {
                self.always_on_top = !self.always_on_top;
                // Asked for on the next frame, where the context is at hand.
                self.window_level_pending = true;
                let _ = self.session.settings.set("always_on_top", if self.always_on_top { "true" } else { "false" });
                self.say(if self.always_on_top { "This window stays on top" } else { "This window behaves like the others" });
            }
            ribbon::Act::TogglePanes => {
                self.split_panes = !self.split_panes;
                self.organisers = true;
                let _ = self.session.settings.set("split_panes", if self.split_panes { "true" } else { "false" });
                self.say(if self.split_panes { "Notebooks and pages in their own columns" } else { "One tree of notebooks" });
            }
            ribbon::Act::ToggleTitle => {
                self.hide_title = !self.hide_title;
                let _ = self.session.settings.set("hide_title", if self.hide_title { "true" } else { "false" });
            }
            ribbon::Act::Zoom(step) => {
                self.zoom_percent = match step {
                    1 => (self.zoom_percent + 10).min(250),
                    -1 => self.zoom_percent.saturating_sub(10).max(50),
                    _ => 100,
                };
                self.fit_page_width = false;
            }
            ribbon::Act::ZoomTo(percent) => {
                self.zoom_percent = percent.clamp(50, 250);
                self.fit_page_width = false;
            }
            ribbon::Act::PageWidth => self.fit_page_width = true,
            ribbon::Act::NotBuilt(what) => {
                self.status = format!("{what} is not built yet. See the roadmap for what is coming.");
                self.status_is_error = false;
            }
            ribbon::Act::Tag(tag) => self.apply_tag(tag),
            ribbon::Act::Clipboard(which) => self.clipboard(which),
            ribbon::Act::Paste => self.paste_here(),
            ribbon::Act::Indent(deeper) => self.indent_block(deeper),
            ribbon::Act::ClearFormatting => self.clear_formatting(),
            ribbon::Act::Direction(d) => self.apply_direction(d),
            ribbon::Act::Justify(strength) => self.apply_justify(strength),
            ribbon::Act::PageDirection(d) => self.apply_page_direction(d),
            ribbon::Act::Embedding(d) => self.apply_embedding(d),
            ribbon::Act::InkPointer(p) => self.pointer = p,
            ribbon::Act::InkPen(i) => {
                self.nib = i.min(self.rack.len().saturating_sub(1));
                self.pointer = ribbon::Pointer::Draw;
                self.remember_rack();
            }
            ribbon::Act::InkRack(edit) => self.edit_rack(edit),
            ribbon::Act::Table(what) => self.edit_table(what),
            ribbon::Act::TableSelect(what) => self.select_in_table(what),
            ribbon::Act::TableBorders(on) => {
                self.table_borders = on;
                let _ = self.session.settings.set("table_borders", if on { "true" } else { "false" });
            }
            ribbon::Act::PageColour(c) => {
                self.set_page_appearance(None, Some(c));
                self.say(match c {
                    Some(c) => format!("The page is {}", c.label().to_lowercase()),
                    None => "The page is the theme's own colour".to_string(),
                });
            }
            ribbon::Act::Undo => self.undo(),
            ribbon::Act::Redo => self.redo(),
            ribbon::Act::ToGridCraft => self.hand_table_to_gridcraft(),
            ribbon::Act::FromGridCraft => self.read_table_from_gridcraft(),
            ribbon::Act::NewGridCraft => self.new_gridcraft(),
            ribbon::Act::InsertTable(rows, columns) => self.insert_table(rows, columns),
            ribbon::Act::InkSensitivity(v) => {
                self.sensitivity = v.min(100);
                let _ = self.session.settings.set(SENSITIVITY_KEY, &self.sensitivity.to_string());
            }
            ribbon::Act::InkUndo => self.ink_undo(),
            ribbon::Act::InkRedo => self.ink_redo(),
            ribbon::Act::PageSheets(on) => {
                let Some(ed) = self.editor.as_mut() else {
                    self.say("Open a page first");
                    return;
                };
                ed.sheet_a4 = on;
                ed.sheets = usize::from(on);
                let path = path_str(&ed.dir);
                self.run("page.sheet", &params(&[("path", path.as_str()), ("sheet", if on { "a4" } else { "none" })]));
            }
            ribbon::Act::InkInsert => self.ink_insert(),
            ribbon::Act::InkExportPlain => self.ink_export_plain(),
        }
    }

    /// Adds, removes, moves or changes a tool on the rack, and remembers the result. The
    /// rules about what may not happen - the last tool cannot be taken off, a tool cannot
    /// be moved off either end - live in `notecraft_ink::rack`, so they hold wherever the
    /// rack is edited from.
    pub(super) fn edit_rack(&mut self, edit: ribbon::RackEdit) {
        use notecraft_ink::rack;
        match edit {
            ribbon::RackEdit::Add(tool) => {
                if self.rack.len() >= rack::MAX_NIBS {
                    self.say("There is no room on the rack for another tool");
                    return;
                }
                self.rack.push(rack::new_nib(tool));
                self.nib = self.rack.len() - 1;
                self.pointer = ribbon::Pointer::Draw;
                self.say(format!("Added a {}", tool.label().to_lowercase()));
            }
            ribbon::RackEdit::Remove(i) => {
                if !rack::remove(&mut self.rack, i) {
                    self.say("The last tool stays on the rack");
                    return;
                }
                if self.nib >= self.rack.len() {
                    self.nib = self.rack.len() - 1;
                } else if self.nib > i {
                    self.nib -= 1;
                }
                self.say("Took it off the rack");
            }
            ribbon::RackEdit::Move(i, left) => {
                let to = rack::shift(&mut self.rack, i, left);
                if self.nib == i {
                    self.nib = to;
                } else if self.nib == to {
                    self.nib = i;
                }
            }
            ribbon::RackEdit::Set(i, nib) => {
                if let Some(slot) = self.rack.get_mut(i) {
                    *slot = nib;
                    self.nib = i;
                    self.pointer = ribbon::Pointer::Draw;
                }
            }
        }
        self.remember_rack();
    }
}
