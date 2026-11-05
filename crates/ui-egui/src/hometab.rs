//! The Home tab, laid out to a drawn design with the same measures and group builder as the View and Insert
//! tabs ([`crate::viewtab`]): Undo, Clipboard, Text, Text direction, Styles, Tags.
//!
//! Small controls are rows of an icon and a label (34 high), tiles are 36 square, and the styles are a
//! framed line of the styles a writer reaches for with an arrow that opens the whole list.

use crate::ribbon::{Act, STYLES, View, leak};
use crate::svgicon::{Prim, paint};
use crate::theme::Tokens;
use crate::viewtab::{CONTENT_TOP, Face, GAP, Ink, LINE_TO, Spec, group, text_on_baseline, text_on_baseline_in, width_of};
use egui::{Color32, FontId, Rect, Response, Sense, Ui, pos2, vec2};
use notecraft_model::Tag;

const UNDO: [Prim; 2] = [Prim::Path("M9 14L4 9l5-5"), Prim::Path("M4 9h11a5 5 0 010 10h-3")];
const REDO: [Prim; 2] = [Prim::Path("M15 14l5-5-5-5"), Prim::Path("M20 9H9a5 5 0 000 10h3")];
const PASTE: [Prim; 2] = [Prim::Rect(5.0, 4.0, 14.0, 17.0, 2.0), Prim::Path("M9 4V3h6v1M9 11h6M9 15h4")];
const CUT: [Prim; 3] = [Prim::Circle(6.0, 18.0, 3.0), Prim::Circle(18.0, 18.0, 3.0), Prim::Path("M8 16L18 4M16 16L6 4")];
const COPY: [Prim; 2] = [Prim::Rect(8.0, 8.0, 12.0, 12.0, 2.0), Prim::Path("M16 8V5a1 1 0 00-1-1H5a1 1 0 00-1 1v10a1 1 0 001 1h3")];
const CODE: [Prim; 1] = [Prim::Path("M8 7l-5 5 5 5M16 7l5 5-5 5")];
const LINK: [Prim; 2] = [Prim::Path("M10 14a4 4 0 005.7 0l3-3a4 4 0 00-5.7-5.7l-1 1"), Prim::Path("M14 10a4 4 0 00-5.7 0l-3 3a4 4 0 005.7 5.7l1-1")];
const CLEAR: [Prim; 1] = [Prim::Path("M6 5h12M12 5l-4 14M15 14l5 5M20 14l-5 5")];
const BULLETS: [Prim; 1] = [Prim::Path("M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01")];
const NUMBERS: [Prim; 1] = [Prim::Path("M10 6h10M10 12h10M10 18h10M4 5h1.5v4M3.5 15.5a1.5 1.5 0 013 .5L4 19h2.5")];
const OUTDENT: [Prim; 1] = [Prim::Path("M21 6H11M21 12h-8M21 18H11M7 9l-3 3 3 3")];
const INDENT: [Prim; 1] = [Prim::Path("M21 6H11M21 12h-8M21 18H11M4 9l3 3-3 3")];
const QUOTE: [Prim; 1] = [Prim::Path("M5 6v12M10 8h10M10 12h10M10 16h6")];
const PILCROW: [Prim; 1] = [Prim::Path("M13 4v16M17 4v16M19 4H9.5a4.5 4.5 0 000 9H13")];
const PAGE_LTR: [Prim; 1] = [Prim::Path("M4 6h10M4 12h10M4 18h10M18 9l3 3-3 3")];
const PAGE_RTL: [Prim; 1] = [Prim::Path("M10 6h10M10 12h10M10 18h10M6 9l-3 3 3 3")];
const PAGE_AUTO: [Prim; 1] = [Prim::Path("M4 6h10M4 12h10M4 18h7M18 15v6M15 18h6")];
const PARA_LTR: [Prim; 1] = [Prim::Path("M10 4v12M13 4v12M15 4H8a3.5 3.5 0 000 7h2M16 20l3-2-3-2")];
const PARA_RTL: [Prim; 1] = [Prim::Path("M12 4v12M15 4v12M17 4h-7a3.5 3.5 0 000 7h2M7 16l-3 2 3 2")];
const CHEVRON: [Prim; 1] = [Prim::Path("M6 9l6 6 6-6")];
const TODO: [Prim; 2] = [Prim::Rect(4.0, 4.0, 16.0, 16.0, 3.0), Prim::Path("M8.5 12l2.5 2.5 4.5-5")];
const STAR: [Prim; 1] = [Prim::Path("M12 3l2.8 5.8 6.2.9-4.5 4.4 1 6.2L12 17.4 6.5 20.3l1-6.2L3 9.7l6.2-.9z")];

/// A small control: an icon of 18 and a label of 13 in a row 34 high. Gives its response.
fn row_item(ui: &mut Ui, ink: &Ink, rect: Rect, id: &str, name: &str, tip: &str, icon: &[Prim], label: &str, live: bool) -> Response {
    let painter = ui.painter().clone();
    let resp = ui.interact(rect, ui.id().with(("home-row", id)), if live { Sense::click() } else { Sense::hover() });
    if live && resp.hovered() {
        painter.rect_filled(rect, 8.0, ink.hover);
    }
    let colour = if live { ink.text } else { ink.disabled };
    paint(&painter, pos2(rect.min.x + 10.0, rect.min.y + 8.0), 18.0, icon, colour);
    text_on_baseline(&painter, rect.min.x + 36.0, rect.min.y + 24.5, false, label, 13.0, colour);
    let (enabled, said) = (ui.is_enabled() && live, name.to_string());
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, said.clone()));
    resp.on_hover_text(tip)
}

/// A tile: 36 square, lit when `on`, with whatever `draw` puts in it.
fn tile(ui: &mut Ui, ink: &Ink, rect: Rect, id: &str, name: &str, tip: &str, on: bool, draw: impl FnOnce(&egui::Painter, Rect, Color32)) -> Response {
    let painter = ui.painter().clone();
    let resp = ui.interact(rect, ui.id().with(("home-tile", id)), Sense::click());
    if on {
        painter.rect_filled(rect, 8.0, ink.on_fill);
    } else if resp.hovered() {
        painter.rect_filled(rect, 8.0, ink.hover);
    }
    draw(&painter, rect, if on { ink.on_text } else { ink.text });
    let (enabled, said) = (ui.is_enabled(), name.to_string());
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, said.clone()));
    resp.on_hover_text(tip)
}

/// A tile with an icon of 18 in the middle of it.
fn icon_tile(ui: &mut Ui, ink: &Ink, rect: Rect, id: &str, name: &str, tip: &str, icon: &[Prim]) -> Response {
    tile(ui, ink, rect, id, name, tip, false, |p, r, c| paint(p, pos2(r.center().x - 9.0, r.center().y - 9.0), 18.0, icon, c))
}

/// A letter in a tile, 16 points: bold (drawn twice, a little apart), italic, or struck through.
fn letter_tile(ui: &mut Ui, ink: &Ink, rect: Rect, id: &str, name: &str, tip: &str, letter: &str, style: u8) -> Response {
    tile(ui, ink, rect, id, name, tip, false, |p, r, c| {
        let mut format = egui::TextFormat { font_id: FontId::proportional(16.0), color: c, ..Default::default() };
        match style {
            1 => format.italics = true,
            2 => format.strikethrough = egui::Stroke::new(1.2, c),
            _ => {}
        }
        let mut job = egui::text::LayoutJob::default();
        job.append(letter, 0.0, format);
        let galley = p.layout_job(job);
        let at = pos2(r.center().x - galley.size().x / 2.0, r.center().y - galley.size().y / 2.0);
        p.galley(at, galley.clone(), c);
        if style == 0 {
            p.galley(at + vec2(0.7, 0.0), galley, c);
        }
    })
}

/// The Home tab.
pub fn home_tab(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    let origin = ui.cursor().min;
    let panel_top = origin.y - CONTENT_TOP;
    let ink = Ink::new(t);
    let painter = ui.painter().clone();
    let mut x = origin.x + 6.0;

    // Undo and redo, one under the other.
    let row_w = |labels: &[&str]| labels.iter().map(|l| 10.0 + 18.0 + 8.0 + width_of(&painter, l, 13.0) + 10.0).fold(0.0_f32, f32::max);
    x = group(ui, &ink, panel_top, x, "Undo", true, &[Spec::Custom(row_w(&["Undo", "Redo"]))], |ui, _, rect, _| {
        let top = panel_top + 10.0;
        let a = Rect::from_min_size(pos2(rect.min.x, top), vec2(rect.width(), 34.0));
        let b = Rect::from_min_size(pos2(rect.min.x, top + 34.0), vec2(rect.width(), 34.0));
        if row_item(ui, &ink, a, "undo", "Undo", "Undo (Ctrl+Z) takes back the last change", &UNDO, "Undo", view.can_undo).clicked() && view.can_undo
        {
            acts.push(Act::Undo);
        }
        if row_item(ui, &ink, b, "redo", "Redo", "Redo (Ctrl+Shift+Z) puts it back", &REDO, "Redo", view.can_redo).clicked() && view.can_redo {
            acts.push(Act::Redo);
        }
    });

    // The clipboard: Paste big, Cut and Copy beside it.
    x = group(
        ui,
        &ink,
        panel_top,
        x,
        "Clipboard",
        true,
        &[
            Spec::Button(Face { name: "Paste", lines: &["Paste"], icon: &PASTE, on: false, caret: false }, "paste"),
            Spec::Custom(row_w(&["Cut", "Copy"])),
        ],
        |ui, i, rect, resp| match i {
            0 if resp.is_some_and(|r| r.clicked()) => acts.push(Act::Paste),
            1 => {
                let top = panel_top + 10.0;
                let a = Rect::from_min_size(pos2(rect.min.x, top), vec2(rect.width(), 34.0));
                let b = Rect::from_min_size(pos2(rect.min.x, top + 34.0), vec2(rect.width(), 34.0));
                if row_item(ui, &ink, a, "cut", "Cut", "Cut (Ctrl+X) the selection", &CUT, "Cut", true).clicked() {
                    acts.push(Act::Clipboard("cut"));
                }
                if row_item(ui, &ink, b, "copy", "Copy", "Copy (Ctrl+C) the selection", &COPY, "Copy", true).clicked() {
                    acts.push(Act::Clipboard("copy"));
                }
            }
            _ => {}
        },
    );

    // Text: six tiles on each of two rows.
    x = group(ui, &ink, panel_top, x, "Text", true, &[Spec::Custom(6.0 * 36.0 + 5.0 * GAP)], |ui, _, rect, _| {
        let tile_at =
            |col: usize, row: usize| Rect::from_min_size(pos2(rect.min.x + 38.0 * col as f32, panel_top + 7.0 + 38.0 * row as f32), vec2(36.0, 36.0));
        if letter_tile(ui, &ink, tile_at(0, 0), "bold", "Bold", "Bold (Ctrl+B)", "B", 0).clicked() {
            acts.push(Act::Mark("bold"));
        }
        if letter_tile(ui, &ink, tile_at(1, 0), "italic", "Italic", "Italic (Ctrl+I)", "I", 1).clicked() {
            acts.push(Act::Mark("italic"));
        }
        if letter_tile(ui, &ink, tile_at(2, 0), "strike", "Strikethrough", "Strike through", "S", 2).clicked() {
            acts.push(Act::Mark("strike"));
        }
        if icon_tile(ui, &ink, tile_at(3, 0), "code", "Code", "Code in a line (Ctrl+E)", &CODE).clicked() {
            acts.push(Act::Mark("code"));
        }
        if icon_tile(ui, &ink, tile_at(4, 0), "link", "Link", "Link (Ctrl+K)", &LINK).clicked() {
            acts.push(Act::Insert("link"));
        }
        if icon_tile(ui, &ink, tile_at(5, 0), "clear", "Clear formatting", "Take the marks off", &CLEAR).clicked() {
            acts.push(Act::ClearFormatting);
        }
        if icon_tile(ui, &ink, tile_at(0, 1), "bullets", "Bullets", "A bullet list", &BULLETS).clicked() {
            acts.push(Act::Block("bullet"));
        }
        if icon_tile(ui, &ink, tile_at(1, 1), "numbering", "Numbering", "A numbered list", &NUMBERS).clicked() {
            acts.push(Act::Block("number"));
        }
        if icon_tile(ui, &ink, tile_at(2, 1), "outdent", "Decrease indentation", "Decrease indentation (Shift+Tab)", &OUTDENT).clicked() {
            acts.push(Act::Indent(false));
        }
        if icon_tile(ui, &ink, tile_at(3, 1), "indent", "Increase indentation", "Increase indentation (Tab)", &INDENT).clicked() {
            acts.push(Act::Indent(true));
        }
        if icon_tile(ui, &ink, tile_at(4, 1), "quote", "Quote", "Quote this paragraph", &QUOTE).clicked() {
            acts.push(Act::Block("quote"));
        }
        let align = icon_tile(ui, &ink, tile_at(5, 1), "align", "Alignment and direction", "Alignment, and which way this paragraph reads", &PILCROW);
        crate::ribbon::align_popup(&align, t, acts);
    });

    // Text direction: the page's and the paragraph's, in rows with their names.
    let more_label = "More\u{2026}";
    let more_w = width_of(&painter, more_label, 13.0) + 16.0;
    x = group(ui, &ink, panel_top, x, "Text direction", true, &[Spec::Custom(76.0 + 2.0 * 38.0 + more_w)], |ui, _, rect, _| {
        let tile_at = |col: usize, row: usize| {
            Rect::from_min_size(pos2(rect.min.x + 76.0 + 38.0 * col as f32, panel_top + 7.0 + 38.0 * row as f32), vec2(36.0, 36.0))
        };
        text_on_baseline(ui.painter(), rect.min.x, panel_top + 32.0, false, "Page", 12.0, ink.muted);
        text_on_baseline(ui.painter(), rect.min.x, panel_top + 70.0, false, "Paragraph", 12.0, ink.muted);
        let dir = |d| Act::PageDirection(d);
        if icon_tile(ui, &ink, tile_at(0, 0), "page-ltr", "Page left to right", "The whole page reads left to right", &PAGE_LTR).clicked() {
            acts.push(dir(Some(notecraft_model::Dir::Ltr)));
        }
        if icon_tile(ui, &ink, tile_at(1, 0), "page-rtl", "Page right to left", "The whole page reads right to left (Arabic, Hebrew)", &PAGE_RTL)
            .clicked()
        {
            acts.push(dir(Some(notecraft_model::Dir::Rtl)));
        }
        if icon_tile(ui, &ink, tile_at(2, 0), "page-auto", "Page direction from the words", "Every paragraph decides for itself", &PAGE_AUTO)
            .clicked()
        {
            acts.push(dir(None));
        }
        if icon_tile(ui, &ink, tile_at(0, 1), "para-ltr", "Paragraph left to right", "This paragraph reads left to right", &PARA_LTR).clicked() {
            acts.push(Act::Direction(Some(notecraft_model::Dir::Ltr)));
        }
        if icon_tile(ui, &ink, tile_at(1, 1), "para-rtl", "Paragraph right to left", "This paragraph reads right to left (Arabic, Hebrew)", &PARA_RTL)
            .clicked()
        {
            acts.push(Act::Direction(Some(notecraft_model::Dir::Rtl)));
        }
        let more = Rect::from_min_size(pos2(tile_at(2, 1).min.x, tile_at(2, 1).min.y), vec2(more_w, 36.0));
        let resp = ui.interact(more, ui.id().with("home-more-direction"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(more, 8.0, ink.hover);
        }
        text_on_baseline(ui.painter(), more.min.x + 8.0, panel_top + 70.5, false, more_label, 13.0, ink.text);
        let enabled = ui.is_enabled();
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Text run direction"));
        let resp = resp.on_hover_text(crate::i18n::tr("Text run direction, and a paragraph that reads from its words"));
        crate::ribbon::direction_more_popup(&resp, t, acts);
    });

    // Styles: a framed line of five, and an arrow that opens them all.
    x = group(ui, &ink, panel_top, x, "Styles", true, &[Spec::Custom(504.0)], |ui, _, rect, _| styles(ui, &ink, t, view, panel_top, rect, acts));

    // Tags: two rows, with their shortcuts, and an arrow for the rest.
    group(ui, &ink, panel_top, x, "Tags", false, &[Spec::Custom(224.0)], |ui, _, rect, _| tags(ui, &ink, t, panel_top, rect, acts));
    let _ = (&mut x, LINE_TO);
    ui.allocate_rect(Rect::from_min_max(origin, pos2(x + 240.0, panel_top + CONTENT_TOP + LINE_TO)), Sense::hover());
}

/// The styles box: Heading 1, 2 and 3, Normal and Code in their own looks, 94 apart.
fn styles(ui: &mut Ui, ink: &Ink, t: &Tokens, view: &View<'_>, panel_top: f32, rect: Rect, acts: &mut Vec<Act>) {
    let painter = ui.painter().clone();
    let frame = Rect::from_min_size(pos2(rect.min.x, panel_top + 7.0), vec2(504.0, 74.0));
    painter.rect_stroke(frame, 10.0, egui::Stroke::new(1.0, ink.line), egui::StrokeKind::Inside);
    let middle = frame.center().y;
    // (label, kind, size, a heavier look)
    let looks: [(&str, &str, f32); 5] =
        [("Heading 1", "h1", 17.0), ("Heading 2", "h2", 15.5), ("Heading 3", "h3", 14.5), ("Normal", "p", 14.0), ("Code", "codeblock", 13.0)];
    for (i, (label, kind, size)) in looks.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(frame.min.x + 13.0 + 94.0 * i as f32 - 6.0, frame.min.y + 8.0), vec2(94.0, 58.0));
        let resp = ui.interact(cell, ui.id().with(("home-style", *kind)), Sense::click());
        let on = view.style == *kind;
        if on {
            painter.rect_filled(cell.shrink2(vec2(0.0, 4.0)), 8.0, ink.on_fill);
        } else if resp.hovered() {
            painter.rect_filled(cell.shrink2(vec2(0.0, 4.0)), 8.0, ink.hover);
        }
        let colour = if on { ink.on_text } else { ink.text };
        let x = cell.min.x + 6.0;
        if *kind == "codeblock" {
            let w = width_of(&painter, label, *size) + 12.0;
            painter.rect_filled(Rect::from_min_size(pos2(x, middle - 10.0), vec2(w, 20.0)), 4.0, ink.chip);
            text_on_baseline_in(&painter, x + 6.0, middle + 9.0, false, label, FontId::monospace(*size), colour);
        } else {
            text_on_baseline(&painter, x, middle + 0.6 * size, false, label, *size, colour);
        }
        let (enabled, said) = (ui.is_enabled(), (*label).to_string());
        resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, said.clone()));
        if resp.on_hover_text(*label).clicked() {
            acts.push(Act::Block(leak(kind)));
        }
    }
    let arrow = Rect::from_min_size(pos2(frame.min.x + 479.0 - 8.0, middle - 16.0), vec2(32.0, 32.0));
    let resp = ui.interact(arrow, ui.id().with("home-styles-all"), Sense::click());
    if resp.hovered() {
        painter.rect_filled(arrow, 8.0, ink.hover);
    }
    paint(&painter, pos2(arrow.center().x - 8.0, arrow.center().y - 8.0), 16.0, &CHEVRON, ink.text);
    let enabled = ui.is_enabled();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "More styles"));
    let resp = resp.on_hover_text(crate::i18n::tr("All the styles"));
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(210.0);
        ui.spacing_mut().item_spacing = vec2(2.0, 1.0);
        for (label, kind) in STYLES {
            if crate::ribbon::style_row(ui, label, kind, kind == view.style, t).clicked() {
                acts.push(Act::Block(leak(kind)));
                ui.close();
            }
        }
        ui.separator();
        if ui.button(crate::i18n::tr("Clear formatting")).clicked() {
            acts.push(Act::ClearFormatting);
            ui.close();
        }
    });
}

/// The tags: To Do and Important with their shortcuts, and an arrow that lists them all.
fn tags(ui: &mut Ui, ink: &Ink, t: &Tokens, panel_top: f32, rect: Rect, acts: &mut Vec<Act>) {
    let painter = ui.painter().clone();
    let chip_right = rect.min.x + 190.0;
    for (n, (id, name, tip, icon, label, shortcut)) in [
        ("todo", "To Do Tag", "A to-do box on this line (Ctrl+1)", &TODO[..], "To Do", "Ctrl+1"),
        ("important", "Important (Ctrl+2)", "Mark this line important (Ctrl+2)", &STAR[..], "Important", "Ctrl+2"),
    ]
    .into_iter()
    .enumerate()
    {
        let row = Rect::from_min_size(pos2(rect.min.x, panel_top + 10.0 + 34.0 * n as f32), vec2(190.0, 34.0));
        if row_item(ui, ink, row, id, name, tip, icon, label, true).clicked() {
            acts.push(if n == 0 { Act::Block("todo") } else { Act::Tag(Tag::Important) });
        }
        let w = width_of(&painter, shortcut, 11.0) + 16.0;
        let chip = Rect::from_min_size(pos2(chip_right - w, row.min.y + 8.0), vec2(w, 18.0));
        painter.rect_stroke(chip, 5.0, egui::Stroke::new(1.0, ink.line), egui::StrokeKind::Inside);
        text_on_baseline(&painter, chip.center().x, chip.min.y + 13.5, true, shortcut, 11.0, ink.muted);
    }
    let arrow = Rect::from_min_size(pos2(rect.min.x + 200.0, panel_top + 37.0), vec2(32.0, 32.0));
    let resp = ui.interact(arrow, ui.id().with("home-tags-all"), Sense::click());
    if resp.hovered() {
        painter.rect_filled(arrow, 8.0, ink.hover);
    }
    paint(&painter, pos2(arrow.center().x - 8.0, arrow.center().y - 8.0), 16.0, &CHEVRON, ink.text);
    let enabled = ui.is_enabled();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "More tags"));
    let resp = resp.on_hover_text(crate::i18n::tr("All the tags"));
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(220.0);
        ui.label(egui::RichText::new(crate::i18n::tr("Tags")).strong());
        for tag in Tag::ALL {
            if crate::ribbon::tag_row(ui, tag, t).clicked() {
                acts.push(Act::Tag(tag));
                ui.close();
            }
        }
    });
}
