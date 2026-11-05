//! The window for a new page, and what a new page is given.

use super::*;

impl NoteCraftApp {
    /// The window for a new page, to a drawn design: its title, whether it is one endless sheet or A4 sheets, and its
    /// file format. Gives (create, cancel).
    pub(super) fn new_page_modal(&mut self, ctx: &Context, parent: &Path, title: &mut String, format: &mut Option<PageFormat>) -> (bool, bool) {
        use crate::svgicon::{Prim, paint};
        use crate::viewtab::text_on_baseline;
        const CLOSE: [Prim; 1] = [Prim::Path("M6 6l12 12M18 6L6 18")];
        const DOWN: [Prim; 1] = [Prim::Path("M6 9l6 6 6-6")];
        let t = self.tokens;
        let ink = Color32::from_rgb(26, 16, 48);
        let line = crate::theme::mix(t.faint, t.muted, 0.7);
        let field_fill = crate::theme::mix(t.editor, Color32::BLACK, 0.2);
        let (mut create, mut cancel) = (false, false);
        // The rest of the window steps back.
        modal_backdrop(ctx, "new-page-backdrop");
        let tall = (ctx.viewport_rect().height() - ctx.data(|d| d.get_temp::<f32>(Id::new("header-bottom"))).unwrap_or(0.0) - 32.0).max(200.0);
        let section = Self::node_at(self.session.tree(), parent).map(|n| (n.info.title.clone(), n.info.color));
        let above = Self::parent_title(self.session.tree(), parent);
        let same = notecraft_store::ops::effective_format(Some(parent), None, &self.session.settings).0;
        egui::Area::new("new-page-modal".into()).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift)).show(
            ctx,
            |ui| {
                let mut content = |ui: &mut Ui| {
                    let (card, _) = ui.allocate_exact_size(vec2(480.0, 622.0), Sense::hover());
                    let painter = ui.painter().clone();
                    painter.add(egui::Shadow { offset: [0, 7], blur: 24, spread: 0, color: Color32::from_black_alpha(115) }.as_shape(card, 16.0));
                    painter.rect_filled(card, 16.0, crate::theme::mix(t.panel, t.text, 0.06));
                    let x = card.min.x + 24.0;
                    let top = card.min.y;
                    text_on_baseline(&painter, x, top + 50.0, false, "New page", 20.0, t.text);
                    // Where it will be.
                    let bar_colour = section
                        .as_ref()
                        .and_then(|s| s.1)
                        .map(|c| {
                            let (r, g, b) = c.rgb(self.effective_mode.is_dark());
                            Color32::from_rgb(r, g, b)
                        })
                        .unwrap_or(t.section);
                    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(x, top + 56.0), vec2(4.0, 12.0)), 2.0, bar_colour);
                    let place = match (&section, &above) {
                        (Some((s, _)), Some(a)) => format!("In {s} \u{b7} {a}"),
                        (Some((s, _)), None) => format!("In {s}"),
                        _ => String::new(),
                    };
                    text_on_baseline(&painter, x + 10.0, top + 69.0, false, &place, 12.0, t.muted);
                    // Close.
                    let close = egui::Rect::from_min_size(egui::pos2(card.max.x - 24.0 - 20.0, top + 28.0), vec2(20.0, 20.0));
                    let press = ui.interact(close.expand(8.0), ui.id().with("np-close"), Sense::click());
                    if press.hovered() {
                        painter.rect_filled(close.expand(6.0), 8.0, t.row_hover);
                    }
                    paint(&painter, close.min, 20.0, &CLOSE, t.text);
                    let enabled = ui.is_enabled();
                    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Close"));
                    if press.on_hover_text(crate::i18n::tr("Close (Esc)")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        cancel = true;
                    }
                    // Title.
                    text_on_baseline(&painter, x, top + 108.0, false, "Title", 14.0, t.text);
                    let field = egui::Rect::from_min_size(egui::pos2(x, top + 116.0), vec2(432.0, 44.0));
                    painter.rect_filled(field, 10.0, field_fill);
                    let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(field.shrink2(vec2(13.0, 10.0))));
                    let edit = inner.add(
                        egui::TextEdit::singleline(title)
                            .frame(egui::Frame::NONE)
                            .desired_width(f32::INFINITY)
                            .font(egui::FontId::proportional(15.0))
                            .text_color(t.text),
                    );
                    edit.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Title"));
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    if !edit.has_focus() && !ui.memory(|m| m.focused().is_some()) {
                        // Once: asking again every frame restarts the on-screen keyboard every frame.
                        edit.request_focus();
                    }
                    if edit.has_focus() {
                        painter.rect_stroke(field.expand(1.0), 11.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Outside);
                    }
                    painter.rect_stroke(
                        field,
                        10.0,
                        egui::Stroke::new(1.0, if edit.has_focus() { t.accent } else { line }),
                        egui::StrokeKind::Inside,
                    );
                    text_on_baseline(&painter, x, top + 181.0, false, "You can leave it empty and rename the page later.", 12.0, t.muted);
                    // Type.
                    text_on_baseline(&painter, x, top + 220.0, false, "Page type", 14.0, t.text);
                    for (n, (a4, name, what)) in [
                        (false, "Infinite scroll", "A single sheet that grows as you write."),
                        (true, "A4 sheets", "Separate pages, ready to print."),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let option = egui::Rect::from_min_size(egui::pos2(x + 222.0 * n as f32, top + 230.0), vec2(210.0, 188.0));
                        let on = self.new_page_a4 == a4;
                        let resp = ui.interact(option, ui.id().with(("np-type", n)), Sense::click());
                        painter.rect_filled(option, 12.0, if on { crate::theme::mix(t.panel, t.accent, 0.12) } else { t.panel });
                        let edge = if on {
                            t.accent
                        } else if resp.hovered() {
                            line
                        } else {
                            t.faint
                        };
                        painter.rect_stroke(option, 12.0, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
                        let thumb = egui::Rect::from_min_size(option.min + vec2(13.0, 13.0), vec2(184.0, 96.0));
                        painter.rect_filled(thumb, 8.0, field_fill);
                        if a4 {
                            for k in 0..2 {
                                let sheet = egui::Rect::from_min_size(thumb.min + vec2(32.0 + 50.0 * k as f32, 17.0), vec2(44.0, 62.0));
                                painter.rect_filled(sheet, 3.0, crate::theme::mix(t.panel, t.muted, 0.2));
                            }
                        } else {
                            let sheet = egui::Rect::from_min_size(thumb.min + vec2(60.0, 12.0), vec2(64.0, 84.0));
                            painter.rect_filled(sheet, 4.0, crate::theme::mix(t.panel, t.accent, 0.25));
                        }
                        text_on_baseline(&painter, option.min.x + 13.0, option.min.y + 138.0, false, name, 14.0, t.text);
                        backstage::wrapped(&painter, option.min.x + 13.0, option.min.y + 158.0, what, 12.0, t.muted, 184.0);
                        let enabled = ui.is_enabled();
                        resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, on, name));
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            self.new_page_a4 = a4;
                        }
                    }
                    // Format.
                    text_on_baseline(&painter, x, top + 456.0, false, "File format", 14.0, t.text);
                    let pick = egui::Rect::from_min_size(egui::pos2(x, top + 464.0), vec2(432.0, 44.0));
                    let resp = ui.interact(pick, ui.id().with("np-format"), Sense::click());
                    painter.rect_filled(pick, 10.0, field_fill);
                    painter.rect_stroke(pick, 10.0, egui::Stroke::new(1.0, if resp.hovered() { t.muted } else { line }), egui::StrokeKind::Inside);
                    let shown = match format {
                        Some(f) => f.label().to_string(),
                        None => "Same as the section".to_string(),
                    };
                    let w = text_on_baseline(&painter, pick.min.x + 13.0, pick.min.y + 29.0, false, &shown, 14.0, t.text);
                    if format.is_none() {
                        text_on_baseline(
                            &painter,
                            pick.min.x + 13.0 + w + 6.0,
                            pick.min.y + 29.0,
                            false,
                            &format!("\u{b7} {}", same.label()),
                            14.0,
                            t.muted,
                        );
                    }
                    paint(&painter, egui::pos2(pick.max.x - 29.0, pick.min.y + 14.0), 16.0, &DOWN, t.text);
                    let (enabled, said) = (ui.is_enabled(), format!("File format: {shown}"));
                    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, enabled, said.clone()));
                    egui::Popup::menu(&resp.on_hover_cursor(egui::CursorIcon::PointingHand)).show(|ui| {
                        ui.set_min_width(200.0);
                        if ui.selectable_label(format.is_none(), crate::i18n::tr("Same as the section")).clicked() {
                            *format = None;
                            ui.close();
                        }
                        for f in PageFormat::ALL {
                            if ui.selectable_label(*format == Some(f), f.label()).clicked() {
                                *format = Some(f);
                                ui.close();
                            }
                        }
                    });
                    text_on_baseline(
                        &painter,
                        x,
                        top + 529.0,
                        false,
                        "If the section does not say, the notebook's or the settings' format applies.",
                        12.0,
                        t.muted,
                    );
                    // The foot.
                    text_on_baseline(&painter, x, top + 583.0, false, "Enter to create", 12.0, t.muted);
                    let words = painter.layout_no_wrap(crate::i18n::tr("Create page").into(), egui::FontId::proportional(14.0), ink).size().x;
                    let go = egui::Rect::from_min_size(egui::pos2(card.max.x - 24.0 - (words + 36.0), top + 554.0), vec2(words + 36.0, 44.0));
                    let press = ui.interact(go, ui.id().with("np-create"), Sense::click());
                    painter.rect_filled(go, 10.0, if press.hovered() { crate::theme::mix(t.accent, Color32::WHITE, 0.12) } else { t.accent });
                    text_on_baseline(&painter, go.min.x + 18.0, go.min.y + 30.0, false, "Create page", 14.0, ink);
                    let enabled = ui.is_enabled();
                    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Create page"));
                    if press.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() || enter {
                        create = true;
                    }
                    let cw = painter.layout_no_wrap(crate::i18n::tr("Cancel").into(), egui::FontId::proportional(14.0), t.text).size().x;
                    let off = egui::Rect::from_min_size(egui::pos2(go.min.x - 26.0 - cw, top + 554.0), vec2(cw, 44.0));
                    let press = ui.interact(off.expand2(vec2(10.0, 0.0)), ui.id().with("np-cancel"), Sense::click());
                    text_on_baseline(&painter, off.min.x, off.min.y + 30.0, false, "Cancel", 14.0, if press.hovered() { t.accent } else { t.text });
                    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Cancel"));
                    if press.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        cancel = true;
                    }
                };
                // A screen shorter than the window scrolls it.
                if tall >= 622.0 {
                    content(ui);
                } else {
                    egui::ScrollArea::vertical().max_height(tall).show(ui, content);
                }
            },
        );
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            cancel = true;
        }
        (create && !cancel, cancel)
    }

    /// Lays a page just made out as A4 sheets when that is what new pages are asked to be.
    pub(super) fn give_sheet(&mut self, dir: &Path) {
        if self.new_page_a4 {
            self.run("page.sheet", &params(&[("path", &path_str(dir)), ("sheet", "a4")]));
        }
    }
}
