//! The View tab, laid out to a drawn design to the point: every size, gap and baseline below is a number
//! read off that drawing, in points, with the ribbon panel's top edge as zero.
//!
//! The groups run left to right: Views, Layout, Page, Zoom, Window, Appearance, Source. A button is 72
//! high with a 24 icon over a 12 label; a group's name sits at the foot, and a 1 pixel line closes the
//! group. The colours are the theme's, so the tab follows light and dark and every accent.

use crate::ribbon::{Act, View};
use crate::svgicon::{Prim, paint};
use crate::theme::{Tokens, mix};
use egui::{Color32, FontId, Painter, Rect, Sense, Ui, pos2, vec2};

/// How far the ribbon's scrolling content starts below the panel's top edge (`ribbon::show` leaves it).
pub const CONTENT_TOP: f32 = 3.0;
/// Where the first group's content starts, from the left edge of the content: the panel's padding.
const FIRST_LEFT: f32 = 6.0;
/// A group's content is this far from the line before it, and the line is this far after the content.
const PAD_LEFT: f32 = 10.0;
const PAD_RIGHT: f32 = 11.0;
/// Between two buttons of a group.
const GAP: f32 = 2.0;
/// A button: its top below the panel's top, its height, its corner, its side padding.
const BTN_TOP: f32 = 8.0;
const BTN_H: f32 = 72.0;
const BTN_R: f32 = 10.0;
const BTN_PAD: f32 = 8.0;
/// What a label's arrow takes: the gap and the arrow.
const CARET_ROOM: f32 = 11.0;
/// Icon top below a button's top: with one line of writing, and with two.
const ICON_ONE: f32 = 15.0;
const ICON_TWO: f32 = 8.0;
/// Label baselines below a button's top: one line, then the two lines.
const BASE_ONE: f32 = 57.0;
const BASE_TWO: [f32; 2] = [50.0, 64.0];
/// Where the group lines run, and the group names' baseline, below the panel's top.
const LINE_FROM: f32 = 6.0;
const LINE_TO: f32 = 100.0;
const NAME_BASE: f32 = 99.0;
/// The zoom column: the box's width, top and height, and the second row.
const ZOOM_W: f32 = 167.13;
const ZOOM_BOX_TOP: f32 = 11.0;
const ZOOM_BOX_H: f32 = 32.0;
const ZOOM_ROW_TOP: f32 = 48.0;
const ZOOM_ROW_H: f32 = 28.0;

const NORMAL: [Prim; 2] = [Prim::Rect(3.0, 4.0, 18.0, 16.0, 2.0), Prim::Path("M9 4v16")];
const FULL_PAGE: [Prim; 1] = [Prim::Path("M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5")];
const SPLIT: [Prim; 2] = [Prim::Rect(3.0, 4.0, 18.0, 16.0, 2.0), Prim::Path("M12 4v16")];
const BACKGROUND: [Prim; 2] = [Prim::Rect(4.0, 3.0, 16.0, 18.0, 2.0), Prim::Path("M4 9h16M4 15h16")];
const ZOOM_OUT: [Prim; 2] = [Prim::Circle(11.0, 11.0, 7.0), Prim::Path("M20 20l-3.5-3.5M8 11h6")];
const ZOOM_IN: [Prim; 2] = [Prim::Circle(11.0, 11.0, 7.0), Prim::Path("M20 20l-3.5-3.5M8 11h6M11 8v6")];
const FIT_WIDTH: [Prim; 1] = [Prim::Path("M3 12h18M7 8l-4 4 4 4M17 8l4 4-4 4")];
const CHEVRON_DOWN: [Prim; 1] = [Prim::Path("M6 9l6 6 6-6")];
const PIN: [Prim; 1] = [Prim::Path("M12 17v5M9 3h6l-1 6 3 4H7l3-4z")];
const MOON: [Prim; 1] = [Prim::Path("M20 14.5A8 8 0 019.5 4a8 8 0 1010.5 10.5z")];
const CODE: [Prim; 1] = [Prim::Path("M8 7l-5 5 5 5M16 7l5 5-5 5")];

/// Writes `text` with its baseline on `baseline`, from `x` (left) or about it (centre); gives the width.
pub fn text_on_baseline(painter: &Painter, x: f32, baseline: f32, centre: bool, text: &str, size: f32, colour: Color32) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(size), colour);
    let drop = galley.rows.first().and_then(|row| row.glyphs.first().map(|g| row.pos.y + g.pos.y)).unwrap_or(size * 0.8);
    let width = galley.size().x;
    let left = if centre { x - width / 2.0 } else { x };
    painter.galley(pos2(left, baseline - drop), galley, colour);
    width
}

fn width_of(painter: &Painter, text: &str, size: f32) -> f32 {
    painter.layout_no_wrap(text.to_string(), FontId::proportional(size), Color32::WHITE).size().x
}

/// What is drawn on a button and what a reader of the screen is told it is.
struct Face<'a> {
    name: &'a str,
    lines: &'a [&'a str],
    icon: &'a [Prim],
    on: bool,
    /// A small down arrow after the label: the button opens something.
    caret: bool,
}

/// The tab's own colours.
struct Ink {
    line: Color32,
    text: Color32,
    muted: Color32,
    on_fill: Color32,
    on_text: Color32,
    hover: Color32,
    outline: Color32,
}

impl Ink {
    fn new(t: &Tokens) -> Ink {
        Ink {
            line: t.faint,
            text: t.text,
            muted: t.muted,
            on_fill: t.row_selected,
            on_text: t.accent,
            hover: t.row_hover,
            outline: mix(t.faint, t.muted, 0.55),
        }
    }
}

/// One big button at `x`, `top` being the panel's top. Gives its width and whether it was pressed.
fn button(ui: &mut Ui, ink: &Ink, x: f32, panel_top: f32, face: &Face<'_>, id: &str) -> (f32, egui::Response) {
    let painter = ui.painter().clone();
    let widest = face.lines.iter().map(|l| width_of(&painter, l, 12.0)).fold(0.0_f32, f32::max) + if face.caret { CARET_ROOM } else { 0.0 };
    let width = widest + 2.0 * BTN_PAD;
    let rect = Rect::from_min_size(pos2(x, panel_top + BTN_TOP), vec2(width, BTN_H));
    let resp = ui.interact(rect, ui.id().with(("view-button", id)), Sense::click());
    if face.on {
        painter.rect_filled(rect, BTN_R, ink.on_fill);
    } else if resp.hovered() {
        painter.rect_filled(rect, BTN_R, ink.hover);
    }
    let colour = if face.on { ink.on_text } else { ink.text };
    let two = face.lines.len() > 1;
    paint(&painter, pos2(rect.center().x - 12.0, rect.min.y + if two { ICON_TWO } else { ICON_ONE }), 24.0, face.icon, colour);
    for (i, line) in face.lines.iter().enumerate() {
        let base = rect.min.y + if two { BASE_TWO[i.min(1)] } else { BASE_ONE };
        if face.caret {
            // The label and the arrow after it, together about the middle.
            let text_w = width_of(&painter, line, 12.0);
            let left = rect.center().x - (text_w + CARET_ROOM) / 2.0;
            text_on_baseline(&painter, left, base, false, line, 12.0, colour);
            let c = pos2(left + text_w + 7.0, base - 4.0);
            painter.add(egui::Shape::convex_polygon(vec![c + vec2(-3.5, -2.0), c + vec2(3.5, -2.0), c + vec2(0.0, 2.5)], colour, egui::Stroke::NONE));
        } else {
            text_on_baseline(&painter, rect.center().x, base, true, line, 12.0, colour);
        }
    }
    let (enabled, name, on) = (ui.is_enabled(), face.name.to_string(), face.on);
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name.clone()));
    (width, resp.on_hover_text(face.name))
}

/// A group's name under its content, and the line after it.
fn close_group(ui: &Ui, ink: &Ink, panel_top: f32, from: f32, to: f32, name: &str, line: bool) -> f32 {
    let painter = ui.painter();
    text_on_baseline(painter, (from + to) / 2.0, panel_top + NAME_BASE, true, name, 12.0, ink.muted);
    let x = to + PAD_RIGHT;
    if line {
        painter.vline(x, (panel_top + LINE_FROM)..=(panel_top + LINE_TO), egui::Stroke::new(1.0, ink.line));
    }
    x
}

/// The View tab.
pub fn view_tab(ui: &mut Ui, source: bool, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    let origin = ui.cursor().min;
    let panel_top = origin.y - CONTENT_TOP;
    let ink = Ink::new(t);
    let mut x = origin.x + FIRST_LEFT;

    // Views
    let start = x;
    let (w, normal) =
        button(ui, &ink, x, panel_top, &Face { name: "Normal view", lines: &["Normal"], icon: &NORMAL, on: !view.full_page, caret: false }, "normal");
    if normal.clicked() && view.full_page {
        acts.push(Act::ToggleFullPage);
    }
    x += w + GAP;
    let (w, full) = button(
        ui,
        &ink,
        x,
        panel_top,
        &Face { name: "Full Page View", lines: &["Full page"], icon: &FULL_PAGE, on: view.full_page, caret: false },
        "full",
    );
    if full.clicked() {
        acts.push(Act::ToggleFullPage);
    }
    x = close_group(ui, &ink, panel_top, start, x + w, "Views", true) + PAD_LEFT;

    // Layout
    let start = x;
    let (w, split) =
        button(ui, &ink, x, panel_top, &Face { name: "Split", lines: &["Split"], icon: &SPLIT, on: view.split_panes, caret: false }, "split");
    if split.clicked() {
        acts.push(Act::TogglePanes);
    }
    x = close_group(ui, &ink, panel_top, start, x + w, "Layout", true) + PAD_LEFT;

    // Page: the paper and the page colour, in a menu under the button.
    let start = x;
    let (w, background) = button(
        ui,
        &ink,
        x,
        panel_top,
        &Face { name: "Format Background", lines: &["Background"], icon: &BACKGROUND, on: false, caret: true },
        "background",
    );
    crate::ribbon::paper_popup(ui, &background, egui::Id::new("view-tab-paper"), view, t, acts);
    x = close_group(ui, &ink, panel_top, start, x + w, "Page", true) + PAD_LEFT;

    // Zoom
    let start = x;
    let (w, out) =
        button(ui, &ink, x, panel_top, &Face { name: "Zoom Out", lines: &["Zoom out"], icon: &ZOOM_OUT, on: false, caret: false }, "zoom-out");
    if out.clicked() {
        acts.push(Act::Zoom(-1));
    }
    x += w + GAP;
    let (w, inn) = button(ui, &ink, x, panel_top, &Face { name: "Zoom In", lines: &["Zoom in"], icon: &ZOOM_IN, on: false, caret: false }, "zoom-in");
    if inn.clicked() {
        acts.push(Act::Zoom(1));
    }
    x += w + GAP;
    zoom_column(ui, &ink, x, panel_top, view, acts);
    x = close_group(ui, &ink, panel_top, start, x + ZOOM_W, "Zoom", true) + PAD_LEFT;

    // Window
    let start = x;
    let (w, top) = button(
        ui,
        &ink,
        x,
        panel_top,
        &Face { name: "Always on top", lines: &["Always on", "top"], icon: &PIN, on: view.always_on_top, caret: false },
        "top",
    );
    if top.clicked() {
        acts.push(Act::ToggleAlwaysOnTop);
    }
    x = close_group(ui, &ink, panel_top, start, x + w, "Window", true) + PAD_LEFT;

    // Appearance
    let start = x;
    let (w, theme) = button(ui, &ink, x, panel_top, &Face { name: "Theme", lines: &["Theme"], icon: &MOON, on: false, caret: true }, "theme");
    if theme.clicked() {
        acts.push(Act::OpenThemes);
    }
    x = close_group(ui, &ink, panel_top, start, x + w, "Appearance", true) + PAD_LEFT;

    // Source: the last group has no line after it.
    let start = x;
    let (w, code) =
        button(ui, &ink, x, panel_top, &Face { name: "Show HTML", lines: &["Show HTML"], icon: &CODE, on: source, caret: false }, "source");
    if code.clicked() {
        acts.push(Act::ToggleSource);
    }
    x = close_group(ui, &ink, panel_top, start, x + w, "Source", false);

    // The room all this took, so the ribbon knows how wide it is and can scroll.
    ui.allocate_rect(Rect::from_min_max(origin, pos2(x, panel_top + CONTENT_TOP + LINE_TO)), Sense::hover());
}

/// The zoom column: the percent in a box that opens the list of sizes, and Fit width under it.
fn zoom_column(ui: &mut Ui, ink: &Ink, x: f32, panel_top: f32, view: &View<'_>, acts: &mut Vec<Act>) {
    let painter = ui.painter().clone();
    let boxed = Rect::from_min_size(pos2(x, panel_top + ZOOM_BOX_TOP), vec2(ZOOM_W, ZOOM_BOX_H));
    let resp = ui.interact(boxed, ui.id().with("view-zoom-box"), Sense::click());
    if resp.hovered() {
        painter.rect_filled(boxed, 8.0, ink.hover);
    }
    painter.rect_stroke(boxed, 8.0, egui::Stroke::new(1.0, ink.outline), egui::StrokeKind::Inside);
    text_on_baseline(&painter, boxed.min.x + 11.0, boxed.min.y + 23.5, false, &format!("{}%", view.zoom_percent), 13.0, ink.text);
    paint(&painter, pos2(boxed.min.x + 70.0, boxed.min.y + 9.0), 14.0, &CHEVRON_DOWN, ink.text);
    let (enabled, label) = (ui.is_enabled(), format!("Zoom {}%", view.zoom_percent));
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, enabled, &label));
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(ZOOM_W - 16.0);
        for percent in [50, 60, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250] {
            if ui.selectable_label(view.zoom_percent == percent, format!("{percent}%")).clicked() {
                acts.push(Act::ZoomTo(percent));
                ui.close();
            }
        }
    });

    let row = Rect::from_min_size(pos2(x, panel_top + ZOOM_ROW_TOP), vec2(ZOOM_W, ZOOM_ROW_H));
    let fit = ui.interact(row, ui.id().with("view-zoom-fit"), Sense::click());
    if fit.hovered() {
        painter.rect_filled(row, 8.0, ink.hover);
    }
    paint(&painter, pos2(row.min.x + 10.0, row.min.y + 4.0), 18.0, &FIT_WIDTH, ink.text);
    text_on_baseline(&painter, row.min.x + 36.0, row.min.y + 20.5, false, "Fit width", 13.0, ink.text);
    let enabled = ui.is_enabled();
    fit.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Page Width"));
    if fit.on_hover_text("Fit the page to the available width").clicked() {
        acts.push(Act::PageWidth);
    }
}
