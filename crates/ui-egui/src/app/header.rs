//! The band at the top: title, controls, menus, the search.

use super::*;

impl NoteCraftApp {
    pub(super) fn top_bar(&mut self, ui: &mut Ui) {
        self.title_band(ui, true);
        self.ribbon_bar(ui);
        // Below the ribbon, as OneNote can put it.
        if self.qat.below && !self.qat.hidden {
            let tokens = self.tokens;
            egui::Panel::top("quick-access").show(ui, |ui| {
                ui.horizontal(|ui| self.quick_access(ui, tokens));
            });
        }
    }

    /// Says that the window has no frame of the system's own, so this window draws its buttons
    /// and takes care of moving and resizing it. The program calls this when it asked for no frame.
    /// Saves the page being written, then syncs the notebooks folder in the background.
    pub(super) fn sync_now(&mut self, ctx: &Context) {
        if self.editor.as_ref().is_some_and(|e| e.dirty_since.is_some()) {
            self.save_now();
        }
        let root = self.session.root().to_path_buf();
        self.sync.start(&root, ctx);
    }

    /// Runs the sync timer, and takes in what a finished sync brought.
    pub(super) fn poll_sync(&mut self, ctx: &Context) {
        let was_idle = !self.sync.busy();
        if self.sync.form.enabled && was_idle && self.editor.as_ref().is_some_and(|e| e.dirty_since.is_some()) {
            // Never sync a page half way through a change: the next frame after it is saved will.
            return;
        }
        let root = self.session.root().to_path_buf();
        if let Some(done) = self.sync.poll(&root, ctx)
            && done.report.pulled
        {
            self.session.refresh();
            if let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) {
                self.open_path(&dir.to_string_lossy());
            }
        }
    }

    /// The switch in the header that keeps the on-screen keyboard from opening. Only on a tablet.
    pub(super) fn keyboard_switch(&mut self, ui: &mut Ui, tokens: Tokens) {
        if !self.pen_only {
            return;
        }
        let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 24.0), Sense::click());
        let painter = ui.painter_at(rect);
        if resp.hovered() || self.soft_keyboard_blocked {
            painter.rect_filled(rect, 4.0, tokens.row_hover);
        }
        let body = egui::Rect::from_center_size(rect.center(), vec2(22.0, 13.0));
        let colour = if self.soft_keyboard_blocked { tokens.muted } else { tokens.text };
        painter.rect_stroke(body, 2.0, egui::Stroke::new(1.3, colour), egui::StrokeKind::Inside);
        for row in 0..2 {
            for key in 0..5 {
                let at = body.min + vec2(4.0 + key as f32 * 3.6, 3.6 + row as f32 * 3.4);
                painter.circle_filled(at, 0.9, colour);
            }
        }
        painter.line_segment([body.min + vec2(6.0, 10.5), body.min + vec2(16.0, 10.5)], egui::Stroke::new(1.3, colour));
        if self.soft_keyboard_blocked {
            painter.line_segment([rect.left_bottom() + vec2(4.0, -3.0), rect.right_top() + vec2(-4.0, 3.0)], egui::Stroke::new(2.0, tokens.accent));
        }
        let label = if self.soft_keyboard_blocked { "On-screen keyboard: off" } else { "On-screen keyboard: on" };
        let (enabled, blocked) = (ui.is_enabled(), self.soft_keyboard_blocked);
        resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, blocked, label));
        if resp
            .on_hover_text(if blocked {
                "The on-screen keyboard is blocked. Click to allow it."
            } else {
                "Click to stop the on-screen keyboard from opening."
            })
            .clicked()
        {
            self.soft_keyboard_blocked = !self.soft_keyboard_blocked;
            let _ = self.session.settings.set("soft_keyboard_blocked", if self.soft_keyboard_blocked { "true" } else { "false" });
        }
    }

    /// The coloured band across the top of the window, as in Office: the Quick Access Toolbar,
    /// the page's title, search and the window's buttons in one row, and under it the tab
    /// names. The File screen shows the first row only.
    pub(super) fn title_band(&mut self, ui: &mut Ui, with_tabs: bool) {
        let t = self.tokens;
        let (tab, source, in_table) = (self.ribbon_tab.clone(), self.show_source, self.caret_in_table());
        let title = self.editor.as_ref().map(|e| e.title.trim().to_owned()).filter(|s| !s.is_empty());
        let mut acts = Vec::new();
        // The header is in the panel's own colours. The compact layout has no row of tabs (the toolbar
        // and the header's menu reach every tab); the classic one has them, as plain words.
        let (fill, line) = (t.panel, self.compact_ui);
        egui::Panel::top("band").frame(egui::Frame::new().fill(fill)).show_separator_line(line).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
            self.title_row(ui, title.as_deref());
            if with_tabs && !self.compact_ui {
                ribbon::tab_strip(ui, &tab, source, in_table, self.ribbon_collapsed, &t, &mut acts);
            }
        });
        for a in acts {
            self.ribbon_act(a);
        }
    }

    /// The band's first row. Anything on it that is not a control moves the window.
    pub(super) fn title_row(&mut self, ui: &mut Ui, title: Option<&str>) {
        let h_row: f32 = if self.compact_ui { 48.0 } else { 30.0 };
        let on = self.tokens;
        let ctx = ui.ctx().clone();
        let row = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), h_row));
        ctx.data_mut(|d| d.insert_temp(Id::new("header-bottom"), row.max.y));
        // Added first, so the controls drawn over it win the pointer: only bare band is a handle.
        let handle = ui.interact(row, ui.id().with("title-handle"), Sense::click_and_drag());
        if self.frameless {
            if handle.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if handle.double_clicked() {
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
        }
        // The choice between the system's title bar and this one, kept in the settings.
        handle.context_menu(|ui| {
            let layout = if self.compact_ui { "Use the classic ribbon" } else { "Use the compact toolbar" };
            if ui.button(layout).clicked() {
                self.compact_ui = !self.compact_ui;
                let _ = self.session.settings.set("layout", if self.compact_ui { "compact" } else { "classic" });
                ui.close();
            }
            let label = if self.frameless { "Use the system title bar" } else { "Use NoteCraft's own title bar" };
            if ui.button(label).clicked() {
                self.frameless = !self.frameless;
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!self.frameless));
                let _ = self.session.settings.set("window_frame", if self.frameless { "own" } else { "system" });
                ui.close();
            }
        });
        let text = title.map_or_else(|| "NoteCraft".to_owned(), |t| format!("{t} - NoteCraft"));
        // A page named in Arabic puts the band's words in the wrong order if the whole
        // string is handed to egui, so the band goes through the bidi painter like the page
        // does. The name decides its own direction here; there is no page to follow.
        let band = ui.painter_at(row);
        crate::bidi::line_at(ui, &band, row.center(), Align2::CENTER_CENTER, &text, 12.5, on.text, None);
        let mut left = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        left.add_space(8.0);
        // The logo is the way to the File screen, and back out of it.
        let logo =
            left.add(egui::Image::from_bytes("bytes://notecraft-logo", crate::LOGO_PNG).fit_to_exact_size(vec2(18.0, 18.0)).sense(Sense::click()));
        let (enabled, label) = (left.is_enabled(), "NoteCraft: File");
        logo.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        let logo = logo.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(crate::i18n::tr("File"));
        if logo.clicked() {
            self.backstage = if self.backstage.is_some() { None } else { Some("Info") };
        }
        left.add_space(6.0);
        if self.qat.hidden || !self.qat.below {
            self.quick_access(&mut left, on);
        }
        let mut right = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if self.frameless {
            self.window_buttons(&mut right, &on);
        }
        right.add_space(4.0);
        if self.compact_ui {
            self.more_menu(&mut right, &on);
        }
        right.add_space(4.0);
        self.theme_menu(&mut right, on);
        right.add_space(6.0);
        self.keyboard_switch(&mut right, on);
        right.add_space(10.0);
        let resp = right.add(egui::TextEdit::singleline(&mut self.search).hint_text(crate::i18n::tr("Search all pages")).desired_width(200.0));
        if self.focus_search {
            resp.request_focus();
            self.focus_search = false;
        }
        if resp.changed() && self.search != self.searched {
            self.searched = self.search.clone();
            self.hits = match self.session.run("search.query", &params(&[("q", &self.search), ("limit", "20")])) {
                Ok(Output::Hits(h)) => h,
                _ => Vec::new(),
            };
        }
        ui.advance_cursor_after_rect(row);
    }

    /// The menu at the end of the header: the File screen and the ribbon's other tabs, which the
    /// one-row toolbar leaves out.
    pub(super) fn more_menu(&mut self, ui: &mut Ui, on: &Tokens) {
        let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
        let painter = ui.painter_at(rect);
        if resp.hovered() {
            painter.rect_filled(rect, 4.0, on.row_hover);
        }
        for dx in [-6.0, 0.0, 6.0] {
            painter.circle_filled(rect.center() + vec2(dx, 0.0), 1.8, on.text);
        }
        let resp = resp.on_hover_text(crate::i18n::tr("File, and the other ribbon tabs"));
        let mut acts = Vec::new();
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(150.0);
            if ui.button(crate::i18n::tr("File")).clicked() {
                acts.push(ribbon::Act::File);
                ui.close();
            }
            for name in ["Home", "Insert", "Draw", "History", "View"] {
                if ui.button(name).clicked() {
                    acts.push(ribbon::Act::Tab(name.to_string()));
                    ui.close();
                }
            }
        });
        for a in acts {
            self.ribbon_act(a);
        }
    }

    /// Minimise, maximise or restore, and close: the buttons the system would have drawn.
    pub(super) fn window_buttons(&self, ui: &mut Ui, on: &Tokens) {
        let ctx = ui.ctx().clone();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        for kind in ["Close", "Maximize", "Minimize"] {
            let (rect, resp) = ui.allocate_exact_size(vec2(46.0, 30.0), Sense::click());
            let painter = ui.painter_at(rect);
            if resp.hovered() {
                let fill = if kind == "Close" { Color32::from_rgb(196, 43, 28) } else { on.row_hover };
                painter.rect_filled(rect, 0.0, fill);
            }
            let c = rect.center();
            let stroke = egui::Stroke::new(1.0, on.text);
            match kind {
                "Close" => {
                    painter.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], stroke);
                    painter.line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], stroke);
                }
                "Maximize" if maximized => {
                    painter.rect_stroke(egui::Rect::from_min_size(c + vec2(-5.0, -2.0), vec2(7.0, 7.0)), 0.0, stroke, egui::StrokeKind::Inside);
                    painter.line_segment([c + vec2(-2.0, -5.0), c + vec2(5.0, -5.0)], stroke);
                    painter.line_segment([c + vec2(5.0, -5.0), c + vec2(5.0, 2.0)], stroke);
                }
                "Maximize" => {
                    painter.rect_stroke(egui::Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, stroke, egui::StrokeKind::Inside);
                }
                _ => {
                    painter.line_segment([c + vec2(-5.0, 4.0), c + vec2(5.0, 4.0)], stroke);
                }
            }
            let label = if kind == "Maximize" && maximized { "Restore" } else { kind };
            let enabled = ui.is_enabled();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
            if resp.on_hover_text(label).clicked() {
                ctx.send_viewport_cmd(match kind {
                    "Close" => egui::ViewportCommand::Close,
                    "Maximize" => egui::ViewportCommand::Maximized(!maximized),
                    _ => egui::ViewportCommand::Minimized(true),
                });
            }
        }
    }

    /// With no frame of the system's own, the edges and corners of the window are ours: the
    /// pointer shows the resize arrow there, and a press starts the resize.
    pub(super) fn frame_edges(&self, ctx: &Context) {
        if !self.frameless || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let r = ctx.content_rect();
        // A line round the window, so its edge shows against whatever is behind it.
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("window-edge"))).rect_stroke(
            r,
            0.0,
            egui::Stroke::new(1.0, self.tokens.band),
            egui::StrokeKind::Inside,
        );
        let Some(pos) = ctx.pointer_latest_pos() else { return };
        const E: f32 = 5.0;
        let (west, east, north, south) = (pos.x - r.min.x < E, r.max.x - pos.x < E, pos.y - r.min.y < E, r.max.y - pos.y < E);
        use egui::{CursorIcon as C, viewport::ResizeDirection as D};
        let (dir, cursor) = match (west, east, north, south) {
            (true, _, true, _) => (D::NorthWest, C::ResizeNorthWest),
            (_, true, true, _) => (D::NorthEast, C::ResizeNorthEast),
            (true, _, _, true) => (D::SouthWest, C::ResizeSouthWest),
            (_, true, _, true) => (D::SouthEast, C::ResizeSouthEast),
            (true, ..) => (D::West, C::ResizeWest),
            (_, true, ..) => (D::East, C::ResizeEast),
            (_, _, true, _) => (D::North, C::ResizeNorth),
            (_, _, _, true) => (D::South, C::ResizeSouth),
            _ => return,
        };
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }

    /// The theme switcher: 14 swatches (light and dark, seven accents each) and a switch to
    /// follow the system setting.
    pub(super) fn theme_menu(&mut self, ui: &mut Ui, tokens: Tokens) {
        let current = self.theme();
        let mut pick: Option<(Mode, Accent)> = None;
        let mut follow = self.mode_pref == ModePref::System;
        let follow_before = follow;
        // A sun or a moon in the theme's accent colour, instead of a line of text.
        let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 24.0), Sense::click());
        let painter = ui.painter_at(rect);
        if resp.hovered() {
            painter.rect_filled(rect, 4.0, tokens.row_hover);
        }
        let icon_rect = egui::Rect::from_center_size(rect.center(), vec2(19.0, 19.0));
        if current.mode.is_dark() {
            icons::paint_moon(&painter, icon_rect, tokens.accent, if resp.hovered() { tokens.row_hover } else { tokens.panel });
        } else {
            icons::paint(&painter, icon_rect, Icon::Sun, tokens.accent);
        }
        let label = format!("Theme: {}", current.label());
        let enabled = ui.is_enabled();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        let resp = resp.on_hover_text(format!("{} theme. Click to change it.", current.label()));
        egui::Popup::menu(&resp).show(|ui| {
            theme_grid(ui, current, &mut pick, &mut follow);
        });
        // The same chooser, as a window, for the ribbon's Theme button.
        if self.theme_window {
            let mut open = true;
            egui::Window::new("Theme").collapsible(false).resizable(false).open(&mut open).anchor(Align2::RIGHT_TOP, vec2(-12.0, 120.0)).show(
                ui.ctx(),
                |ui| {
                    theme_grid(ui, current, &mut pick, &mut follow);
                },
            );
            if !open || pick.is_some() {
                self.theme_window = false;
            }
        }
        if let Some((mode, accent)) = pick {
            // A swatch names a mode, so clicking one means that mode, even while the app was
            // following the system. Otherwise picking "Dark Default" on a light desktop did
            // nothing visible. The switch below is the way back to following the system.
            self.set_theme(Theme { mode, accent });
        } else if follow != follow_before {
            if follow {
                self.follow_system_theme(current.accent);
            } else {
                self.set_theme(current);
            }
        }
    }

    pub(super) fn search_popup(&mut self, ctx: &Context) {
        if self.search.trim().is_empty() {
            return;
        }
        let mut chosen: Option<PathBuf> = None;
        egui::Area::new(Id::new("search-results")).order(egui::Order::Foreground).anchor(Align2::RIGHT_TOP, vec2(-8.0, 44.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(420.0);
                if self.hits.is_empty() {
                    ui.weak(crate::i18n::tr("No pages match."));
                }
                for h in &self.hits {
                    let r = ui.vertical(|ui| {
                        ui.strong(&h.title);
                        ui.label(egui::RichText::new(&h.snippet).small().color(self.tokens.muted));
                    });
                    if ui.interact(r.response.rect, Id::new(("hit", &h.path)), Sense::click()).clicked() {
                        chosen = Some(h.path.clone());
                    }
                    ui.separator();
                }
            });
        });
        if let Some(p) = chosen {
            let mut parent = p.parent();
            while let Some(d) = parent {
                if d == self.session.root() {
                    break;
                }
                self.open.insert(d.to_path_buf());
                parent = d.parent();
            }
            self.open_page(&p);
            self.search.clear();
            self.searched.clear();
            self.hits.clear();
        }
    }
}
