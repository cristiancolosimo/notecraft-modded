//! The Insert symbol dialog, laid out to a drawn design: a 388 by 495 sheet with rounded corners and a
//! soft shadow, a search field, the symbols in groups of eight to a row, and a foot with the one
//! chosen (its picture, its name and code point) and the Insert button.
//!
//! One press picks a symbol, a double press (or Insert, or Enter) puts it in the page, Escape or the
//! cross closes the dialog.

use crate::svgicon::{Prim, paint};
use crate::theme::{Tokens, mix};
use crate::viewtab::text_on_baseline;
use egui::{Align2, Color32, FontId, Id, Rect, Sense, pos2, vec2};

const W: f32 = 388.0;
const H: f32 = 495.0;
const CELL: f32 = 42.0;
const PITCH: f32 = 46.0;
const LEFT: f32 = 12.0;
const COLUMNS: usize = 8;

/// The groups, each with its symbols and their names.
const GROUPS: [(&str, &[(&str, &str)]); 4] = [
    (
        "COMMON",
        &[
            ("©", "Copyright"),
            ("®", "Registered trademark"),
            ("™", "Trademark"),
            ("§", "Section sign"),
            ("¶", "Pilcrow"),
            ("•", "Bullet"),
            ("°", "Degree"),
            ("…", "Ellipsis"),
        ],
    ),
    (
        "ARROWS",
        &[
            ("→", "Right arrow"),
            ("←", "Left arrow"),
            ("↑", "Up arrow"),
            ("↓", "Down arrow"),
            ("↔", "Left right arrow"),
            ("⇒", "Implies"),
            ("⇐", "Implied by"),
            ("⇔", "If and only if"),
        ],
    ),
    (
        "MATH",
        &[
            ("±", "Plus or minus"),
            ("×", "Times"),
            ("÷", "Divided by"),
            ("√", "Square root"),
            ("∞", "Infinity"),
            ("≠", "Not equal to"),
            ("≤", "Less than or equal to"),
            ("≥", "Greater than or equal to"),
        ],
    ),
    ("CURRENCY", &[("€", "Euro"), ("£", "Pound"), ("¥", "Yen"), ("¢", "Cent"), ("$", "Dollar")]),
];

const CLOSE: [Prim; 1] = [Prim::Path("M6 6l12 12M18 6L6 18")];
const SEARCH: [Prim; 2] = [Prim::Circle(11.0, 11.0, 7.0), Prim::Path("M20 20l-3.5-3.5")];

/// What the dialog asks for this frame.
pub enum Outcome {
    Open,
    Close,
    Insert(String),
}

fn state_id() -> Id {
    Id::new("symbol-dialog-state")
}

/// `(query, chosen symbol)`, kept between frames.
fn load(ctx: &egui::Context) -> (String, String) {
    ctx.data(|d| d.get_temp::<(String, String)>(state_id())).unwrap_or_else(|| (String::new(), GROUPS[0].1[0].0.to_string()))
}

/// Forgets what was typed and chosen, so the dialog opens afresh the next time.
pub fn reset(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove_temp::<(String, String)>(state_id()));
}

fn matches(query: &str, symbol: &str, name: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || name.to_lowercase().contains(&q) || symbol.contains(&q) || code(symbol).to_lowercase().contains(&q)
}

/// The code point, as `U+2260`.
pub fn code(symbol: &str) -> String {
    symbol.chars().next().map_or_else(String::new, |c| format!("U+{:04X}", c as u32))
}

/// Draws the dialog centred, `lift` above the middle. Gives what was asked.
pub fn show(ctx: &egui::Context, t: &Tokens, lift: f32) -> Outcome {
    let (mut query, mut chosen) = load(ctx);
    let mut outcome = Outcome::Open;
    let screen = ctx.content_rect();
    let at = pos2(screen.center().x - W / 2.0, (screen.center().y + lift - H / 2.0).max(8.0));
    let sheet = mix(t.panel, t.text, 0.045);
    let field = mix(sheet, t.text, 0.06);
    egui::Area::new(Id::new("symbol-dialog")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(W, H), Sense::hover());
        let painter = ui.painter().clone();
        let corner = 16;
        // The shadow, in two layers, then the sheet.
        for (offset, blur, alpha) in [(1_i8, 3_u8, 102_u8), (8, 24, 115)] {
            let shadow = egui::Shadow { offset: [0, offset], blur, spread: 0, color: Color32::from_black_alpha(alpha) };
            painter.add(shadow.as_shape(rect, corner));
        }
        painter.rect_filled(rect, corner, sheet);
        let rel = |x: f32, y: f32| pos2(rect.min.x + x, rect.min.y + y);

        text_on_baseline(&painter, rect.min.x + 16.0, rect.min.y + 44.0, false, "Insert symbol", 17.0, t.text);
        let cross = Rect::from_min_size(rel(344.0, 24.0), vec2(20.0, 20.0));
        let cross_resp = ui.interact(cross.expand(6.0), Id::new("symbol-close"), Sense::click());
        if cross_resp.hovered() {
            painter.rect_filled(cross.expand(6.0), 8.0, t.row_hover);
        }
        paint(&painter, cross.min, 20.0, &CLOSE, t.text);
        let (enabled, label) = (ui.is_enabled(), "Close");
        cross_resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        if cross_resp.on_hover_text("Close (Esc)").clicked() {
            outcome = Outcome::Close;
        }

        // The search field.
        painter.rect_filled(Rect::from_min_size(rel(LEFT, 66.0), vec2(364.0, 40.0)), 10.0, field);
        paint(&painter, rel(24.0, 77.0), 18.0, &SEARCH, t.muted);
        let input = Rect::from_min_size(rel(50.0, 76.0), vec2(314.0, 20.0));
        let edit = egui::TextEdit::singleline(&mut query)
            .frame(egui::Frame::NONE)
            .hint_text(egui::RichText::new("Search for a symbol").color(t.muted))
            .font(FontId::proportional(16.0))
            .text_color(t.text)
            .desired_width(314.0)
            .margin(egui::Margin::ZERO);
        ui.put(input, edit);

        // The groups that have something to show, a row of eight cells each.
        let mut shown: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
        for (heading, symbols) in GROUPS {
            let found: Vec<(&str, &str)> = symbols.iter().copied().filter(|(s, n)| matches(&query, s, n)).collect();
            if !found.is_empty() {
                shown.push((heading, found));
            }
        }
        if !shown.iter().any(|(_, list)| list.iter().any(|(s, _)| *s == chosen))
            && let Some((s, _)) = shown.first().and_then(|(_, list)| list.first())
        {
            chosen = (*s).to_string();
        }
        let mut insert_now = false;
        for (g, (heading, symbols)) in shown.iter().enumerate() {
            let top = 140.0 + 76.0 * g as f32;
            // Letter-spaced capitals, as the design has them.
            let mut x = rect.min.x + 16.0;
            for ch in heading.chars() {
                x += text_on_baseline(&painter, x, rect.min.y + top - 5.5, false, &ch.to_string(), 11.0, t.muted) + 0.66;
            }
            for (i, (symbol, name)) in symbols.iter().enumerate() {
                let cell = Rect::from_min_size(rel(LEFT + PITCH * (i % COLUMNS) as f32, top + PITCH * (i / COLUMNS) as f32), vec2(CELL, CELL));
                let resp = ui.interact(cell, Id::new(("symbol-cell", *symbol)), Sense::click());
                let on = *symbol == chosen;
                if on {
                    painter.rect_filled(cell, 8.0, t.row_selected);
                } else if resp.hovered() {
                    painter.rect_filled(cell, 8.0, t.row_hover);
                }
                text_on_baseline(&painter, cell.center().x, cell.min.y + 33.5, true, symbol, 20.0, if on { t.accent } else { t.text });
                let (enabled, label, selected) = (ui.is_enabled(), (*name).to_string(), on);
                resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label.clone()));
                if resp.clone().on_hover_text(*name).clicked() {
                    chosen = (*symbol).to_string();
                }
                if resp.double_clicked() {
                    chosen = (*symbol).to_string();
                    insert_now = true;
                }
            }
        }
        if shown.is_empty() {
            text_on_baseline(&painter, rect.center().x, rect.min.y + 200.0, true, "No symbol matches", 14.0, t.muted);
        }

        // The foot: a line, the symbol chosen, its name and code, and the Insert button.
        painter.hline((rect.min.x + LEFT)..=(rect.max.x - LEFT), rect.min.y + 424.0, egui::Stroke::new(1.0, t.faint));
        let tile = Rect::from_min_size(rel(16.0, 435.0), vec2(48.0, 48.0));
        painter.rect_filled(tile, 10.0, mix(sheet, t.text, 0.055));
        text_on_baseline(&painter, tile.center().x, tile.min.y + 40.5, true, &chosen, 26.0, t.text);
        let name = GROUPS.iter().flat_map(|(_, list)| list.iter()).find(|(s, _)| *s == chosen).map_or("", |(_, n)| *n);
        text_on_baseline(&painter, rect.min.x + 76.0, rect.min.y + 459.0, false, name, 14.0, t.text);
        text_on_baseline(&painter, rect.min.x + 76.0, rect.min.y + 476.0, false, &code(&chosen), 12.0, t.muted);
        let button = Rect::from_min_size(rel(291.7, 437.0), vec2(80.3, 44.0));
        let press = ui.interact(button, Id::new("symbol-insert"), Sense::click());
        painter.rect_filled(button, 10.0, if press.hovered() { mix(t.accent, Color32::WHITE, 0.12) } else { t.accent });
        painter.text(button.center(), Align2::CENTER_CENTER, "Insert", FontId::proportional(14.0), Color32::from_rgb(26, 16, 48));
        let (enabled, label) = (ui.is_enabled(), "Insert");
        press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        if press.clicked() || insert_now || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            outcome = Outcome::Insert(chosen.clone());
        }
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        outcome = Outcome::Close;
    }
    ctx.data_mut(|d| d.insert_temp(state_id(), (query, chosen)));
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_points_are_written_as_the_design_has_them() {
        assert_eq!(code("≠"), "U+2260");
        assert_eq!(code("©"), "U+00A9");
    }

    #[test]
    fn the_search_finds_by_name_by_symbol_and_by_code() {
        assert!(matches("euro", "€", "Euro"));
        assert!(matches("€", "€", "Euro"));
        assert!(matches("2260", "≠", "Not equal to"));
        assert!(matches("", "≠", "Not equal to"));
        assert!(!matches("zebra", "≠", "Not equal to"));
    }
}
