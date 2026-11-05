//! The notebooks and pages columns: the tree, its rows, the actions on it, and a page carried from one place to another.

use super::*;

impl NoteCraftApp {
    pub(super) fn apply(&mut self, act: Act) {
        match act {
            Act::Toggle(p) => {
                if !self.open.remove(&p) {
                    self.open.insert(p);
                }
            }
            Act::Select(p) => {
                let is_page = notecraft_store::tree::read_node(&p).map(|i| i.kind == NodeKind::Page).unwrap_or(false);
                if is_page {
                    self.open_page(&p);
                } else {
                    if !self.open.remove(&p) {
                        self.open.insert(p.clone());
                    }
                    // Selecting a container must also move the editor. Leaving the old
                    // page open here made the navigator and page column describe one
                    // location while the editor showed another.
                    if let Some(page) = first_page(self.session.tree(), &p) {
                        self.open_page(&page);
                    } else {
                        self.selected = Some(p);
                    }
                }
            }
            Act::Add(parent, parent_kind) => {
                let kind = parent_kind.may_contain().first().copied().unwrap_or(NodeKind::Page);
                let kind = if parent_kind == NodeKind::Notebook || parent_kind == NodeKind::SectionGroup { NodeKind::Section } else { kind };
                self.dialog = Some(Dialog::Add { parent, parent_kind, title: String::new(), kind, format: None });
            }
            Act::NewNotebook => self.dialog = Some(Dialog::NewNotebook { name: String::new(), format: None }),
            Act::CloseNotebook(path) => {
                self.save_now();
                let was_showing = self.editor.as_ref().is_some_and(|ed| ed.dir.starts_with(&path));
                if matches!(self.run("notebook.close", &params(&[("path", &path_str(&path))])), Some(Output::Text(ref result)) if result == "closed")
                {
                    self.open.retain(|p| !p.starts_with(&path));
                    if self.selected.as_ref().is_some_and(|p| p.starts_with(&path)) {
                        self.selected = self.session.tree().first().map(|n| n.path.clone());
                    }
                    if was_showing {
                        self.editor = None;
                        self.pad = None;
                        self.nav = NavHistory::default();
                    }
                    self.say("Notebook closed; its files are still on disk");
                }
            }
            Act::RevealRoot => {
                let root = self.session.root().to_path_buf();
                self.reveal(&root);
            }
            Act::ChooseRoot => self.choose_notebooks_folder(),
            Act::AddKind(parent, parent_kind, kind) => {
                self.dialog = Some(Dialog::Add { parent, parent_kind, title: String::new(), kind, format: None });
            }
            Act::AddPageAt(section, order) => {
                let path = path_str(&section);
                let order = order.to_string();
                // No title: the page is created genuinely untitled, like a new OneNote page.
                // "Untitled page" is only ever the display fallback, never real content.
                if let Some(Output::Path(dir)) = self.run("node.create", &params(&[("parent", &path), ("kind", "page"), ("order", &order)])) {
                    self.give_sheet(&dir);
                    self.open_page(&dir);
                    self.say("New page");
                }
            }
            Act::Rename(path, title) => self.dialog = Some(Dialog::Rename { path, title }),
            Act::Delete(path, title, count) => self.dialog = Some(Dialog::Delete { path, title, count }),
            Act::Color(path, color) => {
                let value = color.map_or("none", NodeColor::id);
                if self.run("node.color", &params(&[("path", &path_str(&path)), ("color", value)])).is_some() {
                    self.say(match color {
                        Some(c) => format!("Colour set to {}", c.label().to_lowercase()),
                        None => "Colour removed".to_string(),
                    });
                }
            }
            Act::Place(path, parent, before) => {
                self.save_now();
                let open_here = self.editor.as_ref().map(|e| e.dir.clone());
                let (from, to) = (path_str(&path), path_str(&parent));
                let before = before.map(|b| path_str(&b)).unwrap_or_default();
                if let Some(Output::Path(now)) = self.run("node.move", &params(&[("path", &from), ("parent", &to), ("before", &before)])) {
                    self.session.refresh();
                    if self.selected.as_deref().is_some_and(|s| s.starts_with(&path)) {
                        self.selected = self.selected.take().map(|s| now.join(s.strip_prefix(&path).unwrap_or(Path::new(""))));
                    }
                    if let Some(dir) = open_here.filter(|d| d.starts_with(&path)) {
                        let inside = dir.strip_prefix(&path).unwrap_or(Path::new("")).to_path_buf();
                        self.open_page_inner(&now.join(inside));
                    }
                    self.open.insert(parent);
                }
            }
            Act::CopyLink(path) => self.copy_link_to(&path),
            Act::Reveal(path) => match open_folder(&path) {
                Ok(()) => self.say(format!("Opened {}", self.shown(&path))),
                Err(e) => {
                    self.status = format!("The folder could not be opened: {e}");
                    self.status_is_error = true;
                }
            },
        }
    }

    pub(super) fn collect_branches(&self) -> Vec<PathBuf> {
        let mut v = Vec::new();
        for n in self.session.tree() {
            n.branches(&mut v);
        }
        v
    }

    pub(super) fn draw_node(&self, ui: &mut Ui, node: &Node, depth: usize, acts: &mut Vec<Act>) {
        self.draw_row(ui, node, depth, acts, true);
    }

    /// One row of the tree. With `pages` false the pages are left out, which is what the
    /// organisers column shows: notebooks, section groups and sections only.
    pub(super) fn draw_row(&self, ui: &mut Ui, node: &Node, depth: usize, acts: &mut Vec<Act>, pages: bool) {
        use crate::svgicon::{Prim, paint};
        const DOWN: [Prim; 1] = [Prim::Path("M6 9l6 6 6-6")];
        const RIGHT: [Prim; 1] = [Prim::Path("M9 6l6 6-6 6")];
        const FOLDER: [Prim; 1] = [Prim::Path("M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z")];
        const PAGE: [Prim; 2] = [Prim::Path("M14 3H6a2 2 0 00-2 2v14a2 2 0 002 2h12a2 2 0 002-2V9z"), Prim::Path("M14 3v6h6")];
        let has_kids = if pages { !node.children.is_empty() } else { node.children.iter().any(|c| c.info.kind != NodeKind::Page) };
        let open = self.open.contains(&node.path);
        let is_page = node.info.kind == NodeKind::Page;
        let selected = self.selected.as_deref() == Some(node.path.as_path());
        // A section stays lit while one of its pages is the one open.
        let lit = selected || (node.info.kind == NodeKind::Section && self.selected.as_deref().is_some_and(|s| s.starts_with(&node.path)));
        let width = ui.available_width();
        // The rows of a tree stand 40 apart and those of pages 42, a page's strip between two of them being 4.
        let height = if is_page { 38.0 } else { 40.0 };
        // A notebook after another one stands a little apart from it.
        if node.info.kind == NodeKind::Notebook && depth == 0 && ui.cursor().min.y > ui.min_rect().min.y + 1.0 {
            ui.add_space(4.0);
        }
        let drag_on = is_page && ui.ctx().data(|d| d.get_temp::<bool>(Id::new("nav-drag-on"))).unwrap_or(false);
        // With a mouse a page is carried by dragging it. On a touch screen a drag is the list scrolling, so there a
        // page is picked up by holding a finger on it for a moment.
        let touch = ui.input(|i| i.has_touch_screen());
        let (rect, resp) = ui.allocate_exact_size(vec2(width, height), if drag_on && !touch { Sense::click_and_drag() } else { Sense::click() });
        let painter = ui.painter_at(rect);
        let t = &self.tokens;
        // The rows on screen, for a page being dragged to find where it would land.
        ui.ctx().data_mut(|d| {
            d.get_temp_mut_or_default::<Vec<NavRow>>(Id::new("nav-rows")).push((node.path.clone(), rect, is_page, node.info.title.clone()))
        });
        if drag_on && resp.drag_started() {
            ui.ctx().data_mut(|d| d.insert_temp(Id::new("nav-drag"), Some(node.path.clone())));
        }
        if drag_on && touch && resp.is_pointer_button_down_on() {
            let (now, since, from, at) = ui.input(|i| (i.time, i.pointer.press_start_time(), i.pointer.press_origin(), i.pointer.latest_pos()));
            if let (Some(since), Some(from), Some(at)) = (since, from, at)
                && at.distance(from) < 10.0
            {
                if now - since >= 0.4 {
                    ui.ctx().data_mut(|d| d.insert_temp(Id::new("nav-drag"), Some(node.path.clone())));
                } else {
                    ui.ctx().request_repaint_after(Duration::from_millis(60));
                }
            }
        }
        let being_dragged = ui.ctx().data(|d| d.get_temp::<Option<PathBuf>>(Id::new("nav-drag"))).flatten().is_some_and(|p| p == node.path);
        if lit {
            painter.rect_filled(rect, 8.0, t.row_selected);
        } else if resp.hovered() {
            painter.rect_filled(rect, 8.0, t.row_hover);
        }
        let cy = rect.center().y;
        let d = depth as f32;
        // The colour the user gave this notebook or section, kept readable on the panel.
        let color = match node.info.color {
            Some(c) => {
                let (r, g, b) = c.rgb(self.effective_mode.is_dark());
                crate::theme::readable_on(egui::Color32::from_rgb(r, g, b), t.panel, 3.0, self.effective_mode.is_dark())
            }
            None => match node.info.kind {
                NodeKind::Notebook => t.notebook,
                NodeKind::SectionGroup => t.group,
                NodeKind::Section => t.section,
                NodeKind::Page => t.page,
            },
        };
        // Where the chevron sits, and so where it is pressed.
        let chevron_x = if is_page { rect.min.x + 1.0 } else { rect.min.x + 8.0 + 16.0 * d };
        let chevron_rect = egui::Rect::from_min_size(egui::pos2(chevron_x - 4.0, rect.min.y), vec2(if is_page { 10.0 } else { 20.0 }, height));
        if has_kids {
            let size = if is_page { 10.0 } else { 16.0 };
            let at = egui::pos2(chevron_x, cy - size / 2.0);
            paint(&painter, at, size, if open { &DOWN } else { &RIGHT }, if is_page { t.muted } else { t.text });
        }
        let text_x;
        let text_color;
        let mut text_size = 14.0;
        match node.info.kind {
            NodeKind::Notebook => {
                let chip = egui::Rect::from_min_size(egui::pos2(chevron_x + 24.0, cy - 7.0), vec2(10.0, 14.0));
                painter.rect_filled(chip, 2.0, color);
                text_x = chevron_x + 42.0;
                text_color = if lit { t.accent } else { t.text };
            }
            NodeKind::SectionGroup => {
                paint(&painter, egui::pos2(chevron_x + 24.0, cy - 8.0), 16.0, &FOLDER, t.muted);
                text_x = chevron_x + 48.0;
                text_color = if lit { t.accent } else { t.text };
            }
            NodeKind::Section => {
                let bar = egui::Rect::from_min_size(egui::pos2(rect.min.x + 8.0 + 24.0 * d, cy - 7.0), vec2(4.0, 14.0));
                painter.rect_filled(bar, 2.0, color);
                text_x = bar.min.x + 12.0;
                text_color = if lit { t.accent } else { t.muted };
            }
            NodeKind::Page => {
                let x = rect.min.x + 10.0 + 16.0 * d;
                paint(&painter, egui::pos2(x, cy - 8.0), 16.0, &PAGE, if lit { t.accent } else { t.muted });
                text_x = x + 26.0;
                text_color = if lit { t.accent } else { t.text };
                text_size = 14.0;
            }
        }
        // Pages reserve room for their format label and add-page affordance. Organiser rows
        // do not, so notebook, group and section names should run almost to the pane border.
        let count =
            (lit && node.info.kind == NodeKind::Section).then(|| node.children.iter().filter(|c| c.info.kind == NodeKind::Page).count().to_string());
        let trailing = if is_page {
            52.0
        } else if count.is_some() {
            40.0
        } else {
            10.0
        };
        let text_clip = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x - trailing, rect.max.y));
        // The tree is on screen all the time, so this is the most visible place the words
        // of an Arabic name would come out backwards. Each row's name decides its own
        // direction; the row keeps its place and its clip either way.
        let clipped = painter.with_clip_rect(text_clip);
        crate::bidi::line_at(ui, &clipped, egui::pos2(text_x, cy), Align2::LEFT_CENTER, &node.info.title, text_size, text_color, None);
        if let Some(n) = count {
            painter.text(egui::pos2(rect.max.x - 12.0, cy), Align2::RIGHT_CENTER, n, egui::FontId::proportional(12.0), t.accent);
        }
        let plus_rect = egui::Rect::from_min_size(egui::pos2(rect.max.x - 24.0, rect.min.y + 2.0), vec2(20.0, height - 4.0));
        if is_page && let Some(f) = node.info.format {
            painter.text(egui::pos2(rect.max.x - 30.0, cy), Align2::RIGHT_CENTER, f.short(), egui::FontId::proportional(10.5), t.muted);
        }
        if is_page && resp.hovered() {
            painter.text(plus_rect.center(), Align2::CENTER_CENTER, "+", egui::FontId::proportional(16.0), t.strong);
        }
        if being_dragged {
            painter.rect_filled(rect, 8.0, t.panel.gamma_multiply(0.6));
        }
        let enabled = ui.is_enabled();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &node.info.title));
        let resp = resp.on_hover_text(self.shown(&node.path));
        if resp.clicked() && !being_dragged {
            let pos = resp.interact_pointer_pos().unwrap_or_default();
            if has_kids && chevron_rect.expand2(vec2(2.0, 0.0)).contains(pos) {
                acts.push(Act::Toggle(node.path.clone()));
            } else if plus_rect.contains(pos) {
                acts.push(Act::Add(node.path.clone(), node.info.kind));
            } else {
                acts.push(Act::Select(node.path.clone()));
            }
        }
        resp.context_menu(|ui| {
            // What this node may hold, named in full rather than left to the dialog: a
            // notebook and a group take sections and groups, a section takes pages, and a
            // page takes sub-pages.
            for kind in node.info.kind.may_contain() {
                let label = match kind {
                    NodeKind::SectionGroup => "New section group...",
                    NodeKind::Section => "New section...",
                    NodeKind::Page if node.info.kind == NodeKind::Page => "New sub-page...",
                    NodeKind::Page => "New page...",
                    NodeKind::Notebook => "New notebook...",
                };
                let icon = match kind {
                    NodeKind::SectionGroup => Icon::SectionGroup,
                    NodeKind::Section => Icon::Section,
                    NodeKind::Page => Icon::Page,
                    NodeKind::Notebook => Icon::Notebook,
                };
                if floating::menu_row(ui, Some(floating::Glyph::Icon(icon)), label, t).clicked() {
                    acts.push(Act::AddKind(node.path.clone(), node.info.kind, *kind));
                    ui.close();
                }
            }
            floating::menu_separator(ui, t);
            if floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Page)), crate::i18n::tr("Rename..."), t).clicked() {
                acts.push(Act::Rename(node.path.clone(), node.info.title.clone()));
                ui.close();
            }
            if node.info.kind == NodeKind::Notebook
                && floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Notebook)), crate::i18n::tr("Close notebook"), t).clicked()
            {
                acts.push(Act::CloseNotebook(node.path.clone()));
                ui.close();
            }
            if has_kids
                && floating::menu_row(
                    ui,
                    Some(floating::Glyph::Icon(if open { Icon::Minus } else { Icon::Plus })),
                    if open { "Collapse" } else { "Expand" },
                    t,
                )
                .clicked()
            {
                acts.push(Act::Toggle(node.path.clone()));
                ui.close();
            }
            // Pages take the colour of their section, the way OneNote colours tabs.
            if node.info.kind != NodeKind::Page {
                ui.menu_button("Colour", |ui| {
                    let dark = self.effective_mode.is_dark();
                    egui::Grid::new("node-colours").spacing([4.0, 4.0]).show(ui, |ui| {
                        for (i, c) in NodeColor::ALL.into_iter().enumerate() {
                            if color_swatch(ui, c, dark, node.info.color == Some(c)).clicked() {
                                acts.push(Act::Color(node.path.clone(), Some(c)));
                                ui.close();
                            }
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.add_space(2.0);
                    if ui.button(crate::i18n::tr("No colour")).clicked() {
                        acts.push(Act::Color(node.path.clone(), None));
                        ui.close();
                    }
                });
            }
            if floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Link)), crate::i18n::tr("Copy link to page"), t)
                .on_hover_text(crate::i18n::tr("A link to paste into another page"))
                .clicked()
            {
                acts.push(Act::CopyLink(node.path.clone()));
                ui.close();
            }
            if floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Folder)), crate::i18n::tr("Open folder"), t)
                .on_hover_text(self.shown(&node.path))
                .clicked()
            {
                acts.push(Act::Reveal(node.path.clone()));
                ui.close();
            }
            floating::menu_separator(ui, t);
            if floating::menu_row(ui, Some(floating::Glyph::Mark(crate::ribbon_icons::Mark::Delete)), crate::i18n::tr("Delete..."), t).clicked() {
                acts.push(Act::Delete(node.path.clone(), node.info.title.clone(), node.count()));
                ui.close();
            }
        });
        if has_kids && open {
            for c in node.children.iter().filter(|c| pages || c.info.kind != NodeKind::Page) {
                self.draw_row(ui, c, depth + 1, acts, pages);
            }
        }
    }

    /// The node at `path` in the tree.
    pub(super) fn node_at<'a>(nodes: &'a [Node], path: &Path) -> Option<&'a Node> {
        nodes.iter().find_map(|n| if n.path == path { Some(n) } else { Self::node_at(&n.children, path) })
    }

    /// The name of the notebook or group that holds `path`.
    pub(super) fn parent_title(nodes: &[Node], path: &Path) -> Option<String> {
        for n in nodes {
            if n.children.iter().any(|c| c.path == path) {
                return Some(n.info.title.clone());
            }
            if let Some(found) = Self::parent_title(&n.children, path) {
                return Some(found);
            }
        }
        None
    }

    /// The first section anywhere in the tree, for a window that has nothing selected.
    pub(super) fn first_section_of(nodes: &[Node]) -> Option<&Node> {
        for n in nodes {
            if n.info.kind == NodeKind::Section {
                return Some(n);
            }
            if let Some(found) = Self::first_section_of(&n.children) {
                return Some(found);
            }
        }
        None
    }

    /// The section whose pages the pages column shows: the selected node if it is one, else
    /// the section the selected page sits in, else the first section there is.
    pub(super) fn current_section<'a>(&self, tree: &'a [Node]) -> Option<&'a Node> {
        let selected = self.selected.clone();
        fn walk<'a>(nodes: &'a [Node], want: &Option<PathBuf>, best: &mut Option<&'a Node>, inside: Option<&'a Node>) {
            for n in nodes {
                let section = if n.info.kind == NodeKind::Section { Some(n) } else { inside };
                if Some(&n.path) == want.as_ref() {
                    *best = match n.info.kind {
                        NodeKind::Section => Some(n),
                        NodeKind::Page => section,
                        _ => *best,
                    };
                }
                walk(&n.children, want, best, section);
            }
        }
        let mut best = None;
        walk(tree, &selected, &mut best, None);
        best.or_else(|| Self::first_section_of(tree))
    }

    /// The empty space under the rows of a column, with a menu of its own: right-clicking
    /// where there is no row is how a new notebook or a new page is started in OneNote.
    pub(super) fn empty_space_menu(&self, ui: &mut Ui, section: Option<&Path>, acts: &mut Vec<Act>) {
        let left = ui.available_size();
        if left.y < 2.0 {
            return;
        }
        let (rect, resp) = ui.allocate_exact_size(left, Sense::click());
        let _ = rect;
        let mut reveal = false;
        let mut choose = false;
        resp.context_menu(|ui| {
            match section {
                // Under the pages: a page at the end of this section.
                Some(path) => {
                    if ui.button(crate::i18n::tr("New page...")).clicked() {
                        acts.push(Act::AddKind(path.to_path_buf(), NodeKind::Section, NodeKind::Page));
                        ui.close();
                    }
                }
                // Under the notebooks: a notebook, and the ways into the folder itself.
                None => {
                    if ui.button(crate::i18n::tr("New notebook...")).clicked() {
                        acts.push(Act::NewNotebook);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(crate::i18n::tr("Open a folder...")).clicked() {
                        choose = true;
                        ui.close();
                    }
                    if ui.button(crate::i18n::tr("Show the notebooks folder")).clicked() {
                        reveal = true;
                        ui.close();
                    }
                }
            }
        });
        if reveal {
            acts.push(Act::RevealRoot);
        }
        if choose {
            acts.push(Act::ChooseRoot);
        }
    }

    /// The gap between two pages in the pages column. Hovering it draws the line OneNote
    /// draws there, and clicking it makes a page in that place rather than at the end.
    ///
    /// `before` and `after` are the orders of the pages on either side; the new page takes
    /// the number in between, which is why siblings are numbered ten apart.
    pub(super) fn page_gap(&self, ui: &mut Ui, section: &Path, before: Option<u32>, after: Option<u32>, acts: &mut Vec<Act>) {
        /// How big the round "+" is. The gap itself is shorter than this on purpose: it is
        /// a thin strip between two pages, and the button stands out of it.
        const PLUS: f32 = 8.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0), Sense::click());
        let hot = resp.hovered();
        if hot {
            // The gap is 7 points tall and the circle is 13 across, so the circle has to
            // be drawn outside the strip it belongs to. `painter_at` cannot: it narrows
            // the clip it is given to whatever the ui already has, which is the strip, and
            // the circle came out a lozenge, flat at the top and the bottom. Painting on
            // the layer starts from the whole window instead.
            let room = (PLUS + 1.0 - rect.height() / 2.0).max(2.0);
            // On the foreground, so the row below, which is drawn later, does not cover it.
            let painter =
                ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, ui.id().with("page-gap"))).with_clip_rect(rect.expand(room));
            let y = rect.center().y;
            let t = &self.tokens;
            painter.line_segment([egui::pos2(rect.min.x + 21.0, y), egui::pos2(rect.max.x - 6.0, y)], egui::Stroke::new(1.5, t.accent));
            let c = egui::pos2(rect.min.x + 10.0, y);
            painter.circle_filled(c, PLUS, t.accent);
            // Draw the cross as two equal strokes around the circle's center. A font glyph's
            // baseline and line box can make a visually off-center plus even with centered text.
            let cross = egui::Stroke::new(2.0, t.panel);
            painter.line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], cross);
            painter.line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], cross);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let enabled = ui.is_enabled();
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Add a page here"));
        let resp = resp.on_hover_text(crate::i18n::tr("A page here"));
        if resp.clicked() {
            // Halfway between the two, or ten past the last one.
            let order = match (before, after) {
                (Some(b), Some(a)) if a > b + 1 => b + (a - b) / 2,
                (Some(b), Some(_)) => b.saturating_add(1),
                (Some(b), None) => b.saturating_add(10),
                (None, Some(a)) => a / 2,
                (None, None) => 10,
            };
            acts.push(Act::AddPageAt(section.to_path_buf(), order));
        }
    }

    /// The three-bar button, at the top left of the side panel.
    pub(super) fn bars_button_at(ui: &mut Ui) -> egui::Response {
        // Drawn rather than typed: the font's own three-bar glyph is heavy and tight.
        let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
        let painter = ui.painter_at(rect);
        if resp.hovered() {
            painter.rect_filled(rect, 3.0, ui.visuals().widgets.hovered.bg_fill);
        }
        let colour = ui.visuals().widgets.inactive.fg_stroke.color;
        icons::paint(&painter, egui::Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), Icon::Bars, colour);
        let enabled = ui.is_enabled();
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, BARS));
        resp.on_hover_text(crate::i18n::tr("Show or hide the notebooks column"))
    }

    /// The head of the notebooks column: its name and the two buttons that close and open every level.
    /// With `bars` the three-bar button that hides the column comes first. Gives (bars, expand, collapse).
    pub(super) fn nav_header(&self, ui: &mut Ui, bars: bool) -> (bool, bool, bool) {
        use crate::svgicon::{Prim, paint};
        const COLLAPSE: [Prim; 1] = [Prim::Path("M7 20l5-5 5 5M7 4l5 5 5-5")];
        const EXPAND: [Prim; 1] = [Prim::Path("M7 15l5 5 5-5M7 9l5-5 5 5")];
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::hover());
        let painter = ui.painter().clone();
        let t = &self.tokens;
        let mut left = rect.min.x + 10.0;
        let mut toggle = false;
        if bars {
            let at = egui::Rect::from_min_size(egui::pos2(rect.min.x, rect.min.y + 17.0), vec2(22.0, 20.0));
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(at));
            toggle = Self::bars_button_at(&mut inner).clicked();
            left = rect.min.x + 30.0;
        }
        crate::viewtab::text_on_baseline(&painter, left, rect.min.y + 36.5, false, "NOTEBOOKS", 11.0, t.muted);
        let press = |ui: &mut Ui, x: f32, icon: &[Prim], name: &str, tip: &str| -> bool {
            let hit = egui::Rect::from_center_size(egui::pos2(x, rect.min.y + 30.0), vec2(26.0, 26.0));
            let resp = ui.interact(hit, ui.id().with(("nav-header", name)), Sense::click());
            if resp.hovered() {
                painter.rect_filled(hit, 6.0, t.row_hover);
            }
            paint(&painter, egui::pos2(x - 9.0, rect.min.y + 21.0), 18.0, icon, t.text);
            let (enabled, said) = (ui.is_enabled(), name.to_string());
            resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, said.clone()));
            resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
        };
        let expand = press(ui, rect.max.x - 19.0, &EXPAND, "Expand all", "Open every level");
        let collapse = press(ui, rect.max.x - 55.0, &COLLAPSE, "Collapse all", "Close every level");
        (toggle, expand, collapse)
    }

    /// The dashed button under the notebooks. True when pressed.
    pub(super) fn nav_new_notebook(&self, ui: &mut Ui) -> bool {
        use crate::svgicon::{Prim, paint};
        const PLUS: [Prim; 1] = [Prim::Path("M12 5v14M5 12h14")];
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
        let painter = ui.painter().clone();
        let t = &self.tokens;
        if resp.hovered() {
            painter.rect_filled(rect, 8.0, t.row_hover);
        }
        let outline = egui::Stroke::new(1.0, crate::theme::mix(t.faint, t.muted, 0.7));
        let r = rect.shrink(0.5);
        let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
        painter.extend(egui::Shape::dashed_line(&corners, outline, 4.0, 3.0));
        let label = "New notebook";
        let words = painter.layout_no_wrap(crate::i18n::tr(label).into(), egui::FontId::proportional(14.0), t.text).size().x;
        let x = rect.center().x - (18.0 + 8.0 + words) / 2.0;
        paint(&painter, egui::pos2(x, rect.center().y - 9.0), 18.0, &PLUS, t.text);
        crate::viewtab::text_on_baseline(&painter, x + 26.0, rect.min.y + 29.0, false, label, 14.0, t.text);
        let (enabled, said) = (ui.is_enabled(), label.to_string());
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, said.clone()));
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    }

    /// The side panel as two columns: the organisers (notebooks, section groups and
    /// sections) and then the pages of whichever section is chosen, the way OneNote splits
    /// them. The three-bar button in the top bar hides the first column.
    pub(super) fn organiser_panes(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        // A finger drags the lists to scroll them, except while a page is being carried: then it carries.
        let dragging = ui.ctx().data(|d| d.get_temp::<Option<PathBuf>>(Id::new("nav-drag"))).flatten().is_some();
        let scroll = || egui::containers::scroll_area::ScrollSource {
            drag: if dragging { egui::containers::scroll_area::DragScroll::Never } else { egui::containers::scroll_area::DragScroll::OnTouch },
            ..egui::containers::scroll_area::ScrollSource::default()
        };
        let mut toggle = false;
        let ctx = ui.ctx().clone();
        ctx.data_mut(|d| d.insert_temp(Id::new("nav-rows"), Vec::<NavRow>::new()));
        ctx.data_mut(|d| d.insert_temp(Id::new("nav-drag-on"), false));
        if self.organisers {
            egui::Panel::left("organisers").resizable(true).default_size(260.0).size_range(160.0..=520.0).show(ui, |ui| {
                // The three-bar button sits at the top left of the notebooks, where
                // OneNote keeps it, and hides this column.
                let (bars, expand, collapse) = self.nav_header(ui, true);
                toggle |= bars;
                if expand {
                    self.open = self.collect_branches().into_iter().collect();
                }
                if collapse {
                    self.open.clear();
                }
                let room = (ui.available_height() - 72.0).max(60.0);
                egui::ScrollArea::vertical().id_salt("organisers").auto_shrink([false, false]).max_height(room).scroll_source(scroll()).show(
                    ui,
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        if self.session.tree().is_empty() {
                            ui.weak(crate::i18n::tr("No notebooks yet. Use \"New notebook\"."));
                        }
                        for n in self.session.tree() {
                            self.draw_row(ui, n, 0, acts, false);
                        }
                        self.empty_space_menu(ui, None, acts);
                    },
                );
                ui.add_space(12.0);
                if self.nav_new_notebook(ui) {
                    acts.push(Act::NewNotebook);
                }
            });
        }
        let section = self.current_section(self.session.tree()).map(|n| (n.path.clone(), n.info.title.clone(), n.children.clone()));
        egui::Panel::left("pages").resizable(true).default_size(230.0).size_range(150.0..=520.0).show(ui, |ui| {
            ctx.data_mut(|d| d.insert_temp(Id::new("nav-pages-rect"), ui.max_rect()));
            ui.add_space(4.0);
            match &section {
                Some((path, title, pages)) => {
                    let count = pages.iter().filter(|p| p.info.kind == NodeKind::Page).count();
                    let above = Self::parent_title(self.session.tree(), path);
                    let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::hover());
                    let painter = ui.painter().clone();
                    let mut x = head.min.x + 10.0;
                    // With the notebooks hidden the button lives here instead, so it is
                    // always at the top left of the panel.
                    if !self.organisers {
                        let at = egui::Rect::from_min_size(egui::pos2(head.min.x, head.min.y + 17.0), vec2(22.0, 20.0));
                        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(at));
                        toggle |= Self::bars_button_at(&mut inner).clicked();
                        x = head.min.x + 30.0;
                    }
                    let clip = painter.with_clip_rect(egui::Rect::from_min_max(head.min, egui::pos2(head.max.x - 40.0, head.max.y)));
                    crate::viewtab::text_on_baseline(&clip, x, head.min.y + 31.0, false, title, 15.0, self.tokens.text);
                    let pages_word = crate::i18n::tr_dyn(&format!("{count} page{}", if count == 1 { "" } else { "s" })).into_owned();
                    let sub = format!("{}{pages_word}", above.map(|a| format!("{a} \u{b7} ")).unwrap_or_default());
                    crate::viewtab::text_on_baseline(&clip, x, head.min.y + 47.0, false, &sub, 12.0, self.tokens.muted);
                    // Sorting: by hand, which is the order a page can be dragged in, or by what the pages say.
                    {
                        use crate::svgicon::{Prim, paint};
                        const SORT: [Prim; 1] = [Prim::Path("M7 4v16M4 17l3 3 3-3M14 6h6M14 12h4M14 18h2")];
                        let hit = egui::Rect::from_center_size(egui::pos2(head.max.x - 19.0, head.min.y + 30.0), vec2(26.0, 26.0));
                        let resp = ui.interact(hit, ui.id().with("pages-sort"), Sense::click());
                        if resp.hovered() {
                            painter.rect_filled(hit, 6.0, self.tokens.row_hover);
                        }
                        paint(&painter, egui::pos2(hit.center().x - 9.0, hit.center().y - 9.0), 18.0, &SORT, self.tokens.text);
                        let enabled = ui.is_enabled();
                        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Sort pages"));
                        let resp = resp.on_hover_text(crate::i18n::tr("Sort pages")).on_hover_cursor(egui::CursorIcon::PointingHand);
                        let mut chosen = None;
                        egui::Popup::menu(&resp).show(|ui| {
                            ui.set_min_width(170.0);
                            for (value, label) in
                                [("manual", "By hand"), ("name", "By name"), ("created", "Newest first"), ("modified", "Last changed")]
                            {
                                if ui.selectable_label(self.pages_sort == value, label).clicked() {
                                    chosen = Some(value);
                                    ui.close();
                                }
                            }
                        });
                        if let Some(v) = chosen {
                            self.pages_sort = v.to_string();
                            let _ = self.session.settings.set("pages_sort", v);
                        }
                    }
                    // The button for a page, as wide as the column.
                    let (btn, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                    if resp.hovered() {
                        painter.rect_filled(btn, 8.0, self.tokens.row_hover);
                    }
                    painter.rect_stroke(
                        btn,
                        8.0,
                        egui::Stroke::new(1.0, crate::theme::mix(self.tokens.faint, self.tokens.muted, 0.7)),
                        egui::StrokeKind::Inside,
                    );
                    let label = "New page";
                    let words = painter.layout_no_wrap(crate::i18n::tr(label).into(), egui::FontId::proportional(14.0), self.tokens.text).size().x;
                    let lx = btn.center().x - (18.0 + 10.0 + words) / 2.0;
                    crate::svgicon::paint(
                        &painter,
                        egui::pos2(lx, btn.center().y - 9.0),
                        18.0,
                        &[crate::svgicon::Prim::Path("M12 5v14M5 12h14")],
                        self.tokens.text,
                    );
                    crate::viewtab::text_on_baseline(&painter, lx + 28.0, btn.min.y + 26.0, false, label, 14.0, self.tokens.text);
                    let enabled = ui.is_enabled();
                    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "New page"));
                    if resp.on_hover_text(crate::i18n::tr("A new page in this section")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        acts.push(Act::Add(path.clone(), NodeKind::Section));
                    }
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().id_salt("pages").auto_shrink([false, false]).scroll_source(scroll()).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let mut list: Vec<&Node> = pages.iter().filter(|p| p.info.kind == NodeKind::Page).collect();
                        Self::sort_pages(&mut list, &self.pages_sort);
                        // Only a list in the order of the hand can be reordered by it.
                        ctx.data_mut(|d| d.insert_temp(Id::new("nav-drag-on"), self.pages_sort == "manual"));
                        if list.is_empty() {
                            ui.weak(crate::i18n::tr("No pages in this section yet."));
                        }
                        // A line between every pair, and above the first and below the
                        // last: hovering one offers a page in that place.
                        let mut last: Option<u32> = None;
                        for p in &list {
                            self.page_gap(ui, path, last, Some(p.info.order), acts);
                            self.draw_row(ui, p, 0, acts, true);
                            last = Some(p.info.order);
                        }
                        self.page_gap(ui, path, last, None, acts);
                        self.empty_space_menu(ui, Some(path), acts);
                    });
                }
                None => {
                    ui.horizontal(|ui| {
                        if !self.organisers {
                            toggle |= Self::bars_button_at(ui).clicked();
                        }
                        ui.weak(crate::i18n::tr("Choose a section to see its pages."));
                    });
                }
            }
        });
        if toggle {
            self.organisers = !self.organisers;
        }
        let current = section.as_ref().map(|(p, _, _)| p.clone());
        self.nav_drag(&ctx, current.as_deref(), acts);
    }

    /// The order the pages of a section are listed in.
    pub(super) fn sort_pages(list: &mut Vec<&Node>, how: &str) {
        let times = |n: &Node| {
            let meta = notecraft_store::tree::page_file(&n.path).and_then(|(_, f)| std::fs::metadata(f).ok());
            (meta.as_ref().and_then(|m| m.created().ok()), meta.as_ref().and_then(|m| m.modified().ok()))
        };
        match how {
            "name" => list.sort_by_key(|n| n.info.title.to_lowercase()),
            "created" => list.sort_by_key(|n| std::cmp::Reverse(times(n).0)),
            "modified" => list.sort_by_key(|n| std::cmp::Reverse(times(n).1)),
            _ => {}
        }
    }

    /// A page being dragged: the copy that follows the pointer, where it would land, and the move itself when it is let go.
    pub(super) fn nav_drag(&mut self, ctx: &Context, section: Option<&Path>, acts: &mut Vec<Act>) {
        let drag_id = Id::new("nav-drag");
        let Some(source) = ctx.data(|d| d.get_temp::<Option<PathBuf>>(drag_id)).flatten() else { return };
        let released = ctx.input(|i| i.pointer.any_released());
        let cancelled = ctx.input(|i| i.key_pressed(Key::Escape));
        let finish = |ctx: &Context| ctx.data_mut(|d| d.insert_temp(drag_id, None::<PathBuf>));
        let Some(pointer) = ctx.pointer_latest_pos() else {
            finish(ctx);
            return;
        };
        let rows = ctx.data(|d| d.get_temp::<Vec<NavRow>>(Id::new("nav-rows"))).unwrap_or_default();
        let pages_rect = ctx.data(|d| d.get_temp::<egui::Rect>(Id::new("nav-pages-rect")));
        let t = self.tokens;
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("nav-drag-layer")));
        // Where it would land: into a section pointed at, else among the pages of this one.
        let mut drop: Option<(PathBuf, Option<PathBuf>)> = None;
        if let Some((path, rect, _, _)) = rows.iter().find(|r| !r.2 && r.1.contains(pointer)) {
            painter.rect_stroke(*rect, 8.0, egui::Stroke::new(2.0, t.accent), egui::StrokeKind::Inside);
            drop = Some((path.clone(), None));
        } else if pages_rect.is_some_and(|r| r.contains(pointer)) {
            let pages: Vec<&NavRow> = rows.iter().filter(|r| r.2 && !r.0.starts_with(&source)).collect();
            let line = |y: f32, x0: f32, x1: f32| {
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(x0 + 6.0, y - 1.0), egui::pos2(x1 - 6.0, y + 1.0)), 1.0, t.accent);
                painter.circle_filled(egui::pos2(x0 + 2.0, y), 5.0, t.panel);
                painter.circle_stroke(egui::pos2(x0 + 2.0, y), 4.0, egui::Stroke::new(2.0, t.accent));
            };
            match pages.iter().find(|r| r.1.center().y > pointer.y) {
                Some((path, rect, _, _)) => {
                    line(rect.min.y, rect.min.x, rect.max.x);
                    if let Some(parent) = path.parent() {
                        drop = Some((parent.to_path_buf(), Some(path.clone())));
                    }
                }
                None => {
                    if let (Some(section), Some(r)) = (section, pages_rect) {
                        let (y, x0, x1) =
                            pages.last().map_or((r.min.y + 160.0, r.min.x + 8.0, r.max.x - 8.0), |l| (l.1.max.y + 2.0, l.1.min.x, l.1.max.x));
                        line(y, x0, x1);
                        drop = Some((section.to_path_buf(), None));
                    }
                }
            }
        }
        // The copy that follows the pointer.
        let title = rows.iter().find(|r| r.0 == source).map(|r| r.3.clone()).unwrap_or_default();
        let ghost = egui::Rect::from_min_size(pointer + vec2(-20.0, -20.0), vec2(250.0, 40.0));
        painter.add(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: egui::Color32::from_black_alpha(110) }.as_shape(ghost, 8.0));
        painter.rect_filled(ghost, 8.0, crate::theme::mix(t.panel, t.text, 0.1));
        crate::svgicon::paint(
            &painter,
            ghost.min + vec2(10.0, 12.0),
            16.0,
            &[crate::svgicon::Prim::Path("M14 3H6a2 2 0 00-2 2v14a2 2 0 002 2h12a2 2 0 002-2V9z"), crate::svgicon::Prim::Path("M14 3v6h6")],
            t.muted,
        );
        crate::viewtab::text_on_baseline(&painter.with_clip_rect(ghost), ghost.min.x + 36.0, ghost.min.y + 25.0, false, &title, 14.0, t.text);
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        ctx.request_repaint();
        if released || cancelled {
            finish(ctx);
            if released
                && !cancelled
                && let Some((parent, before)) = drop
            {
                acts.push(Act::Place(source, parent, before));
            }
        }
    }

    pub(super) fn side_panel(&mut self, ui: &mut Ui) {
        // Full page view: the notebooks step aside until the user asks for them again.
        if self.full_page && self.editor.is_some() {
            return;
        }
        let mut acts = Vec::new();
        if self.split_panes {
            self.organiser_panes(ui, &mut acts);
            for a in acts {
                self.apply(a);
            }
            return;
        }
        let mut split_now = false;
        egui::Panel::left("tree").resizable(true).default_size(280.0).size_range(200.0..=700.0).show(ui, |ui| {
            // In the one-tree layout the same button splits the panel and hides the
            // notebooks, which is what pressing it in OneNote does.
            let (bars, expand, collapse) = self.nav_header(ui, true);
            split_now = bars;
            if expand {
                self.open = self.collect_branches().into_iter().collect();
            }
            if collapse {
                self.open.clear();
            }
            // A drag in a pane must not scroll it: in the page a drag moves a container,
            // and in the tree it is a click that slipped. The wheel and the bar still work.
            let scroll = egui::containers::scroll_area::ScrollSource {
                drag: egui::containers::scroll_area::DragScroll::OnTouch,
                ..egui::containers::scroll_area::ScrollSource::default()
            };
            let room = (ui.available_height() - 72.0).max(60.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(room).scroll_source(scroll).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if self.session.tree().is_empty() {
                    ui.weak(crate::i18n::tr("No notebooks yet. Use \"New notebook\"."));
                }
                for n in self.session.tree() {
                    self.draw_node(ui, n, 0, &mut acts);
                }
                self.empty_space_menu(ui, None, &mut acts);
            });
            ui.add_space(12.0);
            if self.nav_new_notebook(ui) {
                self.dialog = Some(Dialog::NewNotebook { name: String::new(), format: None });
            }
        });
        if split_now {
            self.split_panes = true;
            self.organisers = false;
            let _ = self.session.settings.set("split_panes", "true");
        }
        for a in acts {
            self.apply(a);
        }
    }
}
