//! The page itself: the sheet, its words and ink, the status line.

use super::*;

impl NoteCraftApp {
    pub(super) fn editor_ui(&mut self, ui: &mut Ui) {
        // A sheet was added by scrolling: the count goes into the page.
        if let Some((dir, n)) = self.editor.as_mut().and_then(|e| std::mem::take(&mut e.sheets_to_save).then(|| (e.dir.clone(), e.sheets))) {
            self.run("page.sheet", &params(&[("path", &path_str(&dir)), ("sheet", &format!("a4:{n}"))]));
            self.say(format!("Sheets: {n}"));
        }
        if !self.icon_recovery_suppressed
            && self.dialog.is_none()
            && let Some(ed) = self.editor.as_ref()
            && let Some(name) = first_missing_attachment_in_source(&ed.body, &ed.dir).or_else(|| first_legacy_attachment_in_source(&ed.body))
        {
            let key = format!("{}::{name}", ed.dir.display());
            if self.icon_recovery_reported.insert(key) {
                log::warn!("attachment icon is missing: page={:?} attachment={name:?}", ed.dir);
                self.dialog = Some(Dialog::MissingAttachmentIcon { page: ed.dir.clone(), name });
            }
        }
        let editor_rect = ui.max_rect();
        // A pinch has no pointer hovering, so its own centre counts as being over the page.
        let wheel_zoom = ui.input(|i| {
            let over =
                i.multi_touch().is_some_and(|m| editor_rect.contains(m.center_pos)) || i.pointer.hover_pos().is_some_and(|p| editor_rect.contains(p));
            if over { i.zoom_delta() } else { 1.0 }
        });
        if wheel_zoom.is_finite() && (wheel_zoom - 1.0).abs() > 0.001 {
            // The percent is a whole number, but a pinch moves it a fraction a frame: the fraction is kept,
            // or a slow pinch would round away to nothing.
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                let kept = if self.zoom_exact.round() as u32 == self.zoom_percent { self.zoom_exact } else { self.zoom_percent as f32 };
                self.zoom_exact = (kept * wheel_zoom).clamp(50.0, 250.0);
                self.zoom_percent = self.zoom_exact.round() as u32;
            }
            self.fit_page_width = false;
        }
        if self.fit_page_width {
            let width = ui.available_width();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let percent = (width / 780.0 * 100.0).round().clamp(50.0, 250.0) as u32;
            self.zoom_percent = percent;
            self.fit_page_width = false;
        }
        #[allow(clippy::cast_precision_loss)]
        let zoom_scale = self.zoom_percent as f32 / 100.0;
        if self.zoom_percent != self.zoom_seen {
            // Zoom around the point that was pinched, or pointed at, or the middle of the page:
            // what is under it stays under it, instead of the page growing from its top left corner.
            #[allow(clippy::cast_precision_loss)]
            let old = (self.zoom_seen as f32 / 100.0).max(0.01);
            self.zoom_seen = self.zoom_percent;
            if let (Some(offset), Some(view)) = (
                ui.ctx().data(|d| d.get_temp::<egui::Vec2>(Id::new("preview-offset"))),
                ui.ctx().data(|d| d.get_temp::<egui::Rect>(Id::new("preview-view"))),
            ) {
                let focus = ui
                    .input(|i| i.multi_touch().map(|m| m.center_pos).or(i.pointer.hover_pos()))
                    .filter(|p| view.contains(*p))
                    .unwrap_or(view.center());
                let (f, width) = (focus - view.min, view.width());
                let margin = |z: f32| ((width - 780.0 * z) / 2.0).max(8.0);
                let across = (offset.x + f.x - margin(old)) / old;
                let x = (margin(zoom_scale) + zoom_scale * across - f.x).max(0.0);
                let y = ((offset.y + f.y) * (zoom_scale / old) - f.y).max(0.0);
                // For a few frames: the page is larger now, and the scroll area only lets the offset reach as far
                // as the page it has measured, which is the old one for the first of them.
                ui.ctx().data_mut(|d| d.insert_temp(Id::new("preview-set-offset"), (egui::vec2(x, y), 4_u8)));
            }
        }
        let root = self.session.root().to_path_buf();
        let tokens = self.tokens;
        let mut rename: Option<String> = None;
        let cursor = self.cursor;
        let mut changed = false;
        let show_source = self.show_source;
        let full_page = self.full_page;
        let (hide_title, rules) = (self.hide_title, self.paper_now());
        // The wash behind this page, if it carries a colour: a pale tint of it over the
        // page's own background, so the words keep their contrast whatever is chosen.
        let page_colour = self.editor.as_ref().and_then(|e| e.colour).map(|c| {
            let dark = self.theme().mode == crate::theme::Mode::Dark;
            let (r, g, b) = c.rgb(dark);
            crate::theme::mix(self.tokens.editor, egui::Color32::from_rgb(r, g, b), 0.16)
        });
        // What the title area is showing, so the page's words do not say it again.
        let shown_title = self.editor.as_ref().map(|e| e.title.clone()).unwrap_or_default();
        let table_borders = self.table_borders;
        let (mut toggle_source, mut toggle_full_page, mut focus_search) = (false, false, false);
        let mut from_blocks = false;
        let mut boxes_moved = false;
        let mut cols_moved = false;
        // A file other than the page written in the code view, waiting to go to disk.
        let mut wrote_file: Option<(String, String)> = None;
        let mut file_action: Option<crate::codeview::FileAction> = None;
        let mut code = std::mem::take(&mut self.code);
        let mut save_cols: Option<(String, String)> = None;
        // A link clicked in the page this frame, followed once the page's borrow is over.
        let mut follow: Option<String> = None;
        // The drawing on this page is lent to the block that holds it, and written back
        // once the page's borrow is over.
        let (pen, pressure) = (self.pen(), self.stylus.sample().pressure);
        let (pen_only, stylus_in_contact) = (self.pen_only, self.stylus.pen_contact());
        let mut pad = self.pad.take();
        let mut ink_changed = false;
        let mut page_rect = None;
        // What the page's right-click menu asked for, run once the page's borrow is over.
        let mut page_acts: Vec<ribbon::Act> = Vec::new();
        {
            let Some(ed) = self.editor.as_mut() else { return };
            let crumbs = ed.dir.strip_prefix(&root).map(|p| p.to_string_lossy().replace('\\', "  /  ")).unwrap_or_default();
            // One id for the title, whether it is drawn as words or as a text box.
            let title_id = egui::Id::new("page-title-box");
            // Full page view gives the room to the page: no path, no hint, no strip of icons.
            if !full_page {
                ui.label(egui::RichText::new(crumbs).small().color(tokens.muted));
                ui.horizontal(|ui| {
                    // The title is not here any more. It is drawn on the sheet itself, at the
                    // top of the page, because the page's name is part of the page: that is
                    // what OneNote does and what the owner asked for on 2026-10-19. What is
                    // left in this strip is the things that are about the file rather than in
                    // it, and they stay out of the page.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let state = if ed.dirty_since.is_some() {
                            "Editing...".to_string()
                        } else {
                            ed.last_saved.clone().map(|t| format!("Saved {t}")).unwrap_or_else(|| "Saved".into())
                        };
                        ui.label(egui::RichText::new(crate::i18n::tr_dyn(&state).into_owned()).small().color(tokens.muted));
                        ui.label(egui::RichText::new(ed.format.label()).small().color(tokens.muted));
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::tr("Drop a file onto the page to add it")).small().color(tokens.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // The page controls, in OneNote's order: full page, search, source.
                        let full = icon_toggle(
                            ui,
                            if full_page { Icon::ExitFullPage } else { Icon::FullPage },
                            full_page,
                            &tokens,
                            "Full page",
                            if full_page { "Show the notebooks again" } else { "Give the page the whole window" },
                        );
                        if full.clicked() {
                            toggle_full_page = true;
                        }
                        if icon_toggle(ui, Icon::Search, false, &tokens, "Search", "Search all pages").clicked() {
                            focus_search = true;
                        }
                        if icon_toggle(
                            ui,
                            Icon::Code,
                            show_source,
                            &tokens,
                            "Page source",
                            if show_source { "Hide the page source" } else { "Show the page source" },
                        )
                        .clicked()
                        {
                            toggle_source = true;
                        }
                    });
                });
                ui.separator();
            }
            let height = ui.available_height();
            // The page itself. Without the source beside it the page is a readable column,
            // the way a page looks in OneNote or Word, and a click puts the caret in it.
            let mut page = |ui: &mut Ui,
                            ed: &mut Editor,
                            wide: bool,
                            _changed: &mut bool,
                            from_blocks: &mut bool,
                            _toggle: &mut bool,
                            boxes_moved: &mut bool,
                            cols_moved: &mut bool,
                            follow: &mut Option<String>,
                            rename: &mut Option<String>| {
                let scroll = egui::containers::scroll_area::ScrollSource {
                    drag: egui::containers::scroll_area::DragScroll::Never,
                    ..egui::containers::scroll_area::ScrollSource::default()
                };
                // The visible area: the scroll area below gives its content no limit in either
                // direction, so the column is centred and sized by this instead.
                let view = vec2(ui.available_width(), height);
                let mut scroller = egui::ScrollArea::both().id_salt("preview").scroll_source(scroll).max_height(height);
                // A zoom this frame asked for the page to stay where it was under the pointer.
                if let Some((offset, frames)) = ui.ctx().data_mut(|d| d.remove_temp::<(egui::Vec2, u8)>(Id::new("preview-set-offset"))) {
                    scroller = scroller.scroll_offset(offset);
                    if frames > 1 {
                        ui.ctx().data_mut(|d| d.insert_temp(Id::new("preview-set-offset"), (offset, frames - 1)));
                        ui.ctx().request_repaint();
                    }
                }
                let shown = scroller.show(ui, |ui| {
                    // Lines or squares behind the page, the way OneNote rules one. The
                    // spacing comes from the paper chosen; NoteCraft's own two papers draw
                    // every fifth line stronger, which the others do not.
                    // The colour behind the words, and then the lines on it. Both are
                    // the page's own when it carries them, as OneNote's page colour and
                    // rule lines are the page's.
                    // The page has no end down or to the right, so the paper has none either: it is
                    // drawn over whatever part of the sheet is on the screen, numbered from the
                    // sheet's own corner so it stays put as the page scrolls.
                    if wide && let Some(wash) = page_colour {
                        ui.painter().rect_filled(ui.clip_rect(), 0.0, wash);
                    }
                    if wide && rules.on() {
                        let corner = ui.max_rect().min;
                        let sheet = egui::Rect::from_min_max(corner, egui::pos2(f32::INFINITY, f32::INFINITY));
                        crate::ribbon::paint_paper(ui.painter(), sheet, rules, zoom_scale, tokens.rule_line, tokens.rule_line_strong);
                    }
                    // Which way lines fall when they say nothing themselves: the page's own
                    // direction, if the writer gave it one.
                    let page_dir = ed.page_dir;
                    let available_width = view.x;
                    let page_column = 780.0 * zoom_scale;
                    let margin = if wide { ((available_width - page_column) / 2.0).max(8.0) } else { 0.0 };
                    egui::Frame::NONE.inner_margin(egui::Margin { left: margin as i8, right: margin as i8, top: 4, bottom: 8 }).show(ui, |ui| {
                        let column = if available_width >= page_column { page_column } else { (available_width - 16.0).max(1.0) };
                        ui.set_min_width(column);
                        ui.set_width(column);
                        // The page's own name, at the top of the sheet. Before the sheet's
                        // origin is taken below, so the ink layer keeps its own coordinates
                        // and a drawing stays beside the words it was made next to.
                        if !hide_title && !full_page {
                            page_title(ui, ed, &tokens, title_id, rename, zoom_scale);
                        }
                        if wide && ed.writable {
                            // The caret lives in the page: every block is drawn the way it
                            // will be read, and the one being written in is a text box in
                            // that block's own type. The markup is never shown here. The
                            // blocks sit in containers, which is where they are placed.
                            let top = ui.cursor().min;
                            // A press lets go of whatever was selected: the drag that may
                            // follow it builds a new one. Done here, once a frame, rather
                            // than inside the blocks, where a container drawn later would
                            // clear what an earlier one had just set.
                            if ui.input(|i| i.pointer.any_pressed()) {
                                ed.across = None;
                            }
                            let mut moved = crate::containers::show(
                                ui,
                                &mut ed.blocks,
                                &mut ed.boxes,
                                &mut ed.caret,
                                &ed.dir,
                                &tokens,
                                page_dir,
                                table_borders,
                                &mut ed.columns,
                                &ed.justified,
                                &mut ed.across,
                                if hide_title { "" } else { &shown_title },
                                zoom_scale,
                            );
                            page_acts.append(&mut moved.acts);
                            if let Some((html, text)) = moved.copy.take()
                                && self.clipboard.set_html(&html, &text).is_err()
                            {
                                ui.ctx().copy_text(text);
                            }
                            // The ink of a page sits over its words, anywhere on it, the
                            // way a pen on paper does. It goes on after them so a press
                            // with a pen in hand reaches the ink and not the caret.
                            {
                                // The sheet, not the words: ink goes anywhere on the page,
                                // and the page is a sheet of paper whether or not anything
                                // is written that far down it. Measuring the ink's area
                                // from where the text happens to end meant the pen stopped
                                // at the last line, which is not what a sheet of paper
                                // does. The room is claimed so the page can be scrolled
                                // into it and so what is drawn there is never cut off.
                                let used = ui.cursor().min.y - top.y;
                                // The canvas grows with the ink, down and to the right, without
                                // a limit: the page is scrolled both ways to reach it.
                                let far = pad.as_ref().map(|p| p.far()).unwrap_or((0.0, 0.0));
                                let size = crate::inkpad::canvas(
                                    (far.0 * zoom_scale, far.1 * zoom_scale),
                                    used,
                                    view,
                                    ui.max_rect().width(),
                                    ed.sheet_a4,
                                    ed.sheets,
                                );
                                let want = size.y;
                                if want > used {
                                    ui.allocate_space(vec2(size.x, want - used));
                                }
                                let body = egui::Rect::from_min_size(egui::pos2(ui.max_rect().min.x, top.y), size);
                                page_rect = Some(body);
                                if ed.sheet_a4 {
                                    // Each sheet is outlined, so its sides show as well as the gap between it and the
                                    // next; the gap is a tinted band, light enough that words running across it stay
                                    // readable.
                                    let (sheet, pitch) = crate::inkpad::a4_sheet(body.width());
                                    ed.sheets_drawn = (body.height() / pitch).round().max(1.0) as usize;
                                    let painter = ui.painter_at(body.expand(2.0));
                                    let edge = egui::Stroke::new(2.0, tokens.muted);
                                    let mut y = body.min.y;
                                    while y < body.max.y - 1.0 {
                                        let sheet_rect = egui::Rect::from_min_size(egui::pos2(body.min.x, y), vec2(body.width(), sheet));
                                        painter.rect_stroke(sheet_rect.shrink(1.0), 0.0, edge, egui::StrokeKind::Inside);
                                        let gap = egui::Rect::from_min_max(
                                            egui::pos2(body.min.x, sheet_rect.max.y),
                                            egui::pos2(body.max.x, (y + pitch).min(body.max.y)),
                                        );
                                        painter.rect_filled(gap, 0.0, tokens.text.gamma_multiply(0.12));
                                        y += pitch;
                                    }
                                }
                                if let Some(pad) = pad.as_mut() {
                                    ink_changed |=
                                        crate::inkpad::show(ui, body, pad, pen, pressure, !pen_only || stylus_in_contact, &tokens, zoom_scale);
                                    if crate::inkpad::take_switch_to_pick(ui.ctx()) {
                                        page_acts.push(ribbon::Act::InkPointer(ribbon::Pointer::Pick));
                                    }
                                }
                                // A sheet that is empty and was only added by scrolling can be taken away again: its corner has a button.
                                let needed = crate::inkpad::a4_sheets_needed((far.0 * zoom_scale, far.1 * zoom_scale), used, ui.max_rect().width());
                                if ed.sheet_a4 && ed.sheets_drawn > needed.max(1) {
                                    let (_, pitch) = crate::inkpad::a4_sheet(body.width());
                                    let top_of_last = body.min.y + pitch * (ed.sheets_drawn as f32 - 1.0);
                                    let corner = egui::Rect::from_min_size(egui::pos2(body.max.x - 44.0, top_of_last + 12.0), vec2(32.0, 32.0));
                                    let press = ui.interact(corner, ui.id().with("delete-sheet"), Sense::click());
                                    let painter = ui.painter();
                                    painter.circle_filled(corner.center(), 16.0, if press.hovered() { tokens.row_hover } else { tokens.panel });
                                    painter.circle_stroke(corner.center(), 16.0, egui::Stroke::new(1.0, tokens.muted));
                                    crate::svgicon::paint(
                                        painter,
                                        corner.min + vec2(6.0, 6.0),
                                        20.0,
                                        &[crate::svgicon::Prim::Path("M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3")],
                                        tokens.text,
                                    );
                                    let enabled = ui.is_enabled();
                                    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Delete this sheet"));
                                    if press
                                        .on_hover_text(crate::i18n::tr("Delete this sheet"))
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        ed.sheets = ed.sheets_drawn - 1;
                                        ed.sheets_to_save = true;
                                    }
                                }
                                // The page's own right-click menu. No widget is laid
                                // over the page for it: a widget that senses clicks would
                                // take the primary click away from the words, the ink and
                                // the caret, and one that does not sense clicks cannot
                                // carry a context menu at all. So the secondary click is
                                // watched for directly and the menu drawn where it fell.
                                crate::pagemenu::show(ui, body, &mut page_acts, &tokens);
                            }
                            if moved.changed.blocks {
                                ed.rebuild_body();
                                *from_blocks = true;
                            }
                            if moved.boxes {
                                *boxes_moved = true;
                            }
                            if moved.changed.columns {
                                *cols_moved = true;
                            }
                            ed.origin = moved.origin;
                            if moved.changed.link.is_some() {
                                *follow = moved.changed.link;
                            }
                        } else if wide {
                            // A page with markup the block model cannot hold: writing it back
                            // would drop that markup, so it is read here and changed in its
                            // source. See docs/gaps.md.
                            if let Some(href) = preview::show_zoomed(ui, &ed.blocks, &ed.dir, &tokens, page_dir, &ed.justified, zoom_scale) {
                                *follow = Some(href);
                            }
                            ui.add_space(14.0);
                            ui.label(
                                egui::RichText::new(crate::i18n::tr(
                                    "This page holds markup NoteCraft does not edit yet, so it is shown as it reads.",
                                ))
                                .small()
                                .color(tokens.muted),
                            );
                        } else if let Some(href) = preview::show_zoomed(ui, &ed.blocks, &ed.dir, &tokens, page_dir, &ed.justified, zoom_scale) {
                            *follow = Some(href);
                        }
                    });
                });
                // Scrolling on past the foot of the last A4 sheet makes the next one.
                if ed.sheet_a4 {
                    let at_foot = shown.state.offset.y + shown.inner_rect.height() >= shown.content_size.y - 2.0;
                    let down = ui.input(|i| i.raw.events.iter().any(|e| matches!(e, egui::Event::MouseWheel { delta, .. } if delta.y < 0.0)))
                        && ui.rect_contains_pointer(shown.inner_rect);
                    let now = ui.input(|i| i.time);
                    let last = ui.ctx().data(|d| d.get_temp::<f64>(Id::new("sheet-added-at"))).unwrap_or(-1.0);
                    if at_foot && down && now - last > 0.45 {
                        ed.sheets = ed.sheets_drawn.max(1) + 1;
                        ed.sheets_to_save = true;
                        ui.ctx().data_mut(|d| d.insert_temp(Id::new("sheet-added-at"), now));
                    }
                }
                ui.ctx().data_mut(|d| {
                    d.insert_temp(Id::new("preview-offset"), shown.state.offset);
                    d.insert_temp(Id::new("preview-view"), shown.inner_rect);
                });
            };
            if show_source {
                // The code view: the page's files on the left, the one picked on the
                // right. The page itself is not drawn while it is open, so there is only
                // ever one editable copy of the page on screen.
                let named = pageedit::links_in(&ed.blocks);
                let entries = crate::codeview::files(&ed.dir, ed.format.page_file(), &named);
                let out = crate::codeview::show(ui, &mut code, &entries, &ed.dir, &tokens, height);
                wrote_file = out.wrote;
                file_action = out.file_action;
            } else {
                page(ui, ed, true, &mut changed, &mut from_blocks, &mut toggle_source, &mut boxes_moved, &mut cols_moved, &mut follow, &mut rename);
            }
            if changed {
                Self::mark_dirty(ed);
            } else if from_blocks {
                Self::mark_dirty_from_blocks(ed);
            }
            // Before the step is remembered, so the page in the history is one that
            // names its ink: a box deleted with the ink line in it is repaired at once
            // rather than at the next open.
            if (changed || from_blocks) && Self::name_ink(ed) {
                from_blocks = true;
            }
            if changed || from_blocks || boxes_moved {
                Self::remember_step(ed);
            }
            // A container was dragged or resized: the arrangement is the page's own, so it
            // is written into the page's file rather than kept in the window.
            if boxes_moved {
                ed.arranged = true;
                ed.rebuild_body();
                Self::mark_dirty_from_blocks(ed);
            }
            // A column was dragged wider or narrower. Widths are layout, like the boxes,
            // so they go into the page's own file beside them.
            if cols_moved {
                let path = path_str(&ed.dir);
                let line = notecraft_model::tablecols::write(&ed.columns);
                save_cols = Some((path, line));
            }
        }
        self.cursor = cursor;
        if let Some((path, line)) = save_cols {
            self.run("page.columns", &params(&[("path", &path), ("widths", &line)]));
        }
        self.code = code;
        if let Some((name, text)) = wrote_file {
            self.write_page_file(&name, &text);
        }
        if let Some(action) = file_action {
            match action {
                crate::codeview::FileAction::CopyPath(path) => {
                    ui.ctx().copy_text(path);
                    self.say("Full path copied");
                }
                crate::codeview::FileAction::OpenFolder(path) => match open_folder(&path) {
                    Ok(()) => self.say("Opened the containing folder"),
                    Err(e) => self.complain(&format!("Could not open the containing folder: {e}")),
                },
                crate::codeview::FileAction::OpenShell(path) => {
                    if self.open_attachment_warning_suppressed {
                        match open_file_for_viewer(&path) {
                            Ok(()) => self.say("Opened using the associated application"),
                            Err(e) => self.complain(&format!("Could not open the file: {e}")),
                        }
                    } else {
                        self.dialog = Some(Dialog::OpenAttachmentWarning { path });
                    }
                }
                crate::codeview::FileAction::Delete(name) => {
                    log::debug!("source view delete action received: file={name:?}");
                    let page = self.editor.as_ref().map(|e| e.dir.clone());
                    if let Some(page) = page {
                        self.dialog = Some(Dialog::DeleteFile { page, name });
                    }
                }
                crate::codeview::FileAction::AddFile => {
                    if let Some(dir) = self.editor.as_ref().map(|ed| ed.dir.clone())
                        && let Some(file) = self.pick(Purpose::AttachFile(dir.clone()), "Add file to this page", false, &[], None)
                    {
                        self.dialog = Some(Dialog::FileOptions { dir, files: vec![file] });
                    }
                }
            }
        }
        if toggle_source {
            self.set_show_source(!show_source);
        }
        if toggle_full_page {
            self.full_page = !full_page;
        }
        if focus_search {
            self.focus_search = true;
        }
        // The drawing comes back from the page, and a stroke laid down this frame is
        // written at once: ink is its own file, so it does not wait for the page to save.
        self.pad = pad;
        if page_rect.is_some() {
            self.page_rect = page_rect;
        }
        for act in page_acts {
            self.ribbon_act(act);
        }
        if ink_changed {
            self.last_ink_change = Some(Instant::now());
        }
        if ink_changed && let Some(pad) = self.pad.as_mut() {
            let strokes = pad.strokes();
            match pad.save() {
                Ok(()) => self.say(format!("Drawing saved, {strokes} strokes")),
                Err(e) => self.complain(&format!("Cannot write the drawing: {e}")),
            }
            self.sync_ink_reference();
        }
        if let Some(href) = follow {
            self.follow_link(ui.ctx(), &href);
        }
        if let Some(t) = rename
            && let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone())
            && self.run("node.title", &params(&[("path", &path_str(&dir)), ("title", &t)])).is_some()
        {
            if let Some(ed) = self.editor.as_mut() {
                ed.title = t;
            }
            self.say("Title saved");
        }
    }

    pub(super) fn central(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            if self.editor.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.22);
                    ui.add(egui::Image::from_bytes("bytes://notecraft-logo", crate::LOGO_PNG).fit_to_exact_size(vec2(128.0, 128.0)));
                    ui.add_space(8.0);
                    ui.heading(crate::i18n::tr("NoteCraft"));
                    ui.label(crate::i18n::tr("Notes as plain folders. Open a page, or create a notebook to start."));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("Notebooks folder: {}", self.shown(self.session.root()))).small().color(self.tokens.muted));
                });
                return;
            }
            self.editor_ui(ui);
        });
    }

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let color = if self.status_is_error { self.tokens.warn } else { self.tokens.muted };
                ui.label(egui::RichText::new(crate::i18n::tr_dyn(&self.status).into_owned()).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let nodes: usize = self.session.tree().iter().map(|n| n.count()).sum();
                    ui.label(
                        egui::RichText::new(
                            crate::i18n::tr_dyn(&format!(
                                "{} notebooks, {nodes} items  |  {}",
                                self.session.tree().len(),
                                self.shown(self.session.root())
                            ))
                            .into_owned(),
                        )
                        .small()
                        .color(self.tokens.muted),
                    );
                    ui.separator();
                    let (current, source) = self.session.settings.global_format();
                    let mut chosen = current;
                    egui::ComboBox::from_id_salt("default-format").selected_text(current.label()).show_ui(ui, |ui| {
                        for f in PageFormat::ALL {
                            ui.selectable_value(&mut chosen, f, f.label());
                        }
                    });
                    ui.label(egui::RichText::new(format!("({})", crate::i18n::tr(source.label()))).small().color(self.tokens.muted));
                    ui.label(egui::RichText::new(crate::i18n::tr("New pages:")).small().color(self.tokens.muted));
                    if chosen != current && self.run("settings.default_format.set", &params(&[("format", chosen.id())])).is_some() {
                        self.say(format!("New pages use {} unless the notebook or section says otherwise", chosen.label()));
                    }
                });
            });
        });
    }
}
