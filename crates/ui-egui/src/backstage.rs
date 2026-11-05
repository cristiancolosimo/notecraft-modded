//! The File screen: what Office calls the backstage. It takes over the window and shows the
//! notebooks with where they live, and the things done to a whole notebook rather than to a
//! page. Anything that needs a service NoteCraft does not have is listed and marked as not
//! built, rather than hidden, so nobody goes looking for it.

use crate::icons::{self, Icon};
use crate::theme::Tokens;
use egui::{Align2, Sense, Ui, vec2};
use notecraft_model::NodeColor;
use notecraft_store::tree::Node;

/// What the File screen asks the window to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// Leave the File screen.
    Close,
    NewNotebook,
    /// Make a notebook with this name and colour, and open it.
    CreateNotebook(String, Option<NodeColor>),
    /// Write the Welcome notebook into the notebooks folder, or open it if it is there.
    AddWelcome,
    /// Show this notebook's folder in the file manager.
    OpenFolder(std::path::PathBuf),
    /// Show the notebooks folder in the file manager.
    OpenNotebooksFolder,
    /// Ask the system for a folder, and work from the notebooks in it.
    ChooseFolder,
    /// Choose a OneNote notebook folder and pass it to the OfficeIMO bridge.
    ImportNotebookFolder,
    /// Choose one importable file and pass it to the OfficeIMO bridge.
    ImportFile,
    /// Cloud import is intentionally visible but unavailable.
    ImportCloud,
    /// Open a page of this notebook (the first one).
    Open(std::path::PathBuf),
    /// Open a notebook from the recent list.
    OpenRecent(std::path::PathBuf),
    /// Forget a notebook in the known notebooks list without touching its files.
    RemoveRecent(std::path::PathBuf),
    /// Ask for confirmation before deleting a notebook from disk.
    DeleteRecent(std::path::PathBuf, String),
    SetTelemetryLevel(String),
    OpenTelemetryFolder(std::path::PathBuf),
    ClearTelemetry(std::path::PathBuf),
    ExportTelemetry(std::path::PathBuf),
    /// Keep the sync form and start syncing now (or just keep it, for `false`).
    SyncSave(bool),
    /// Show or hide the performance counter.
    SetShowPerf(bool),
    /// Cap the frame rate (0 for no cap).
    SetFpsLimit(u32),
    /// The compact toolbar (true) or the tabbed ribbon (false).
    SetCompactUi(bool),
    /// Write this notebook to a ZIP.
    ExportNotebook(std::path::PathBuf),
    ResetPreferences,
    /// Another page of the File screen: Info, New or Open.
    Page(&'static str),
    /// A side entry that is listed but not built yet.
    NotBuilt(&'static str),
}

/// The entries down the left side, in the order the Office apps use. The flag says whether
/// it works.
pub const SIDE: [(&str, bool); 10] = [
    ("Info", true),
    ("New", true),
    ("Open", true),
    ("Save a Copy", false),
    ("Print", false),
    ("Share", false),
    ("Sync", true),
    ("Export", true),
    ("Account", false),
    ("Options", true),
];

/// Draws the File screen over the whole window. Returns what the user asked for.
pub fn show(
    ui: &mut Ui,
    notebooks: &[Node],
    recent: &[Node],
    notebooks_folder: &str,
    page: &str,
    show_path: &dyn Fn(&std::path::Path) -> String,
    telemetry_root: &std::path::Path,
    telemetry_level: &str,
    session_id: Option<&str>,
    sync: &mut crate::syncing::Syncing,
    show_perf: bool,
    fps_limit: u32,
    compact_ui: bool,
    t: &Tokens,
) -> Vec<Act> {
    let mut acts = Vec::new();
    ui.painter().rect_filled(ui.max_rect(), 0.0, t.editor);
    egui::Frame::NONE.show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            side_bar(ui, page, t, &mut acts);
            ui.add_space(48.0);
            ui.vertical(|ui| {
                // The page has the same room to the right as to the left of it.
                ui.set_max_width((ui.available_width() - 48.0).max(200.0));
                if !matches!(page, "New" | "Open" | "Sync" | "Export" | "Options") {
                    info_page(ui, notebooks, notebooks_folder, show_path, t, &mut acts);
                    return;
                }
                ui.add_space(18.0);
                match page {
                    "New" => new_page(ui, notebooks, notebooks_folder, show_path, t, &mut acts),
                    "Open" => open_page(ui, recent, notebooks_folder, show_path, t, &mut acts),
                    "Sync" => sync_page(ui, sync, t, &mut acts),
                    "Export" => export_page(ui, notebooks, show_path, t, &mut acts),
                    _ => options_page(ui, telemetry_root, telemetry_level, session_id, show_perf, fps_limit, compact_ui, t, &mut acts),
                }
            });
        });
    });
    acts
}

/// The navigation down the left of the File screen, to a drawn design: 264 wide with a line on its right,
/// rows 48 high 50 apart, each an icon of 20 and a label of 15 on a pill that is lit for the page shown.
fn side_bar(ui: &mut Ui, page: &str, t: &Tokens, acts: &mut Vec<Act>) {
    use crate::svgicon::{Prim, paint};
    use crate::viewtab::text_on_baseline;
    const BACK: [Prim; 1] = [Prim::Path("M19 12H5M11 6l-6 6 6 6")];
    const INFO: [Prim; 2] = [Prim::Circle(12.0, 12.0, 9.0), Prim::Path("M12 11v6M12 7.5v.01")];
    const NEW: [Prim; 1] = [Prim::Path("M12 5v14M5 12h14")];
    const OPEN: [Prim; 1] = [Prim::Path("M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z")];
    const SYNC: [Prim; 1] = [Prim::Path("M20 11a8 8 0 00-14.3-4.9L4 8M4 4v4h4M4 13a8 8 0 0014.3 4.9L20 16M20 20v-4h-4")];
    const EXPORT: [Prim; 1] = [Prim::Path("M12 15V3M7 8l5-5 5 5M4 15v4a2 2 0 002 2h12a2 2 0 002-2v-4")];
    const GEAR: [Prim; 2] = [
        Prim::Circle(12.0, 12.0, 3.0),
        Prim::Path(
            "M19.4 15a1.7 1.7 0 00.3 1.8l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.8-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 11-4 0v-.1a1.7 1.7 0 00-1.1-1.5 1.7 1.7 0 00-1.8.3l-.1.1a2 2 0 11-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.8 1.7 1.7 0 00-1.5-1H3a2 2 0 110-4h.1a1.7 1.7 0 001.5-1.1 1.7 1.7 0 00-.3-1.8l-.1-.1a2 2 0 112.8-2.8l.1.1a1.7 1.7 0 001.8.3H9a1.7 1.7 0 001-1.5V3a2 2 0 114 0v.1a1.7 1.7 0 001 1.5 1.7 1.7 0 001.8-.3l.1-.1a2 2 0 112.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.8V9a1.7 1.7 0 001.5 1H21a2 2 0 110 4h-.1a1.7 1.7 0 00-1.5 1z",
        ),
    ];
    let height = ui.available_height().max(420.0);
    let (rect, _) = ui.allocate_exact_size(vec2(264.0, height), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, t.panel);
    painter.vline(rect.max.x - 0.5, rect.y_range(), egui::Stroke::new(1.0, t.faint));
    let row = |ui: &mut Ui, y: f32, icon: &[Prim], label: &str, name: &str, lit: bool, muted: bool, tip: &str| -> egui::Response {
        let pill = egui::Rect::from_min_size(egui::pos2(rect.min.x + 12.0, rect.min.y + y), vec2(239.0, 48.0));
        let resp = ui.interact(pill, ui.id().with(("backstage-row", name)), Sense::click());
        if lit {
            painter.rect_filled(pill, 10.0, t.row_selected);
        } else if resp.hovered() {
            painter.rect_filled(pill, 10.0, t.row_hover);
        }
        let colour = if lit {
            t.accent
        } else if muted {
            t.muted
        } else {
            t.text
        };
        paint(&painter, egui::pos2(pill.min.x + 14.0, pill.min.y + 14.0), 20.0, icon, colour);
        text_on_baseline(&painter, pill.min.x + 46.0, pill.min.y + 33.0, false, label, 15.0, colour);
        let (enabled, said) = (ui.is_enabled(), name.to_string());
        resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, lit, said.clone()));
        resp.on_hover_text(tip)
    };
    if row(ui, 16.0, &BACK, "Back to notes", "Back to notes", false, true, "Back to the notebook").clicked() {
        acts.push(Act::Close);
    }
    let mut y = 78.0;
    for (name, built) in SIDE {
        if !built || name == "Options" {
            continue;
        }
        let icon: &[Prim] = match name {
            "Info" => &INFO,
            "New" => &NEW,
            "Open" => &OPEN,
            "Sync" => &SYNC,
            _ => &EXPORT,
        };
        if row(ui, y, icon, name, name, name == page, false, name).clicked() {
            acts.push(Act::Page(name));
        }
        y += 50.0;
    }
    if row(ui, height - 64.0, &GEAR, "Options", "Options", page == "Options", false, "Options").clicked() {
        acts.push(Act::Page("Options"));
    }
}

fn heading(ui: &mut Ui, text: &str, t: &Tokens) {
    ui.label(egui::RichText::new(text).size(22.0).color(t.strong));
    ui.add_space(8.0);
}

/// The notebooks, to a drawn design: a title and the count, the button for a new one, then a card for
/// each with its cover, its name, how much is in it and where it lives, and what can be done to it.
fn info_page(ui: &mut Ui, notebooks: &[Node], folder: &str, show_path: &dyn Fn(&std::path::Path) -> String, t: &Tokens, acts: &mut Vec<Act>) {
    use crate::svgicon::{Prim, paint};
    use crate::viewtab::{text_on_baseline, text_on_baseline_in};
    const PLUS: [Prim; 1] = [Prim::Path("M12 5v14M5 12h14")];
    let ink = egui::Color32::from_rgb(26, 16, 48);
    let width = ui.available_width();
    let (head, _) = ui.allocate_exact_size(vec2(width, 126.0), Sense::hover());
    let painter = ui.painter().clone();
    text_on_baseline(&painter, head.min.x, head.min.y + 75.0, false, "Notebooks", 28.0, t.text);
    let items: usize = notebooks.iter().map(Node::count).sum();
    let sub = format!("{} notebook{} \u{b7} {items} items", notebooks.len(), if notebooks.len() == 1 { "" } else { "s" });
    text_on_baseline(&painter, head.min.x, head.min.y + 99.0, false, &sub, 14.0, t.muted);
    let new = egui::Rect::from_min_size(egui::pos2(head.max.x - 175.1, head.min.y + 58.0), vec2(175.1, 44.0));
    let press = ui.interact(new, ui.id().with("backstage-new-notebook"), Sense::click());
    painter.rect_filled(new, 10.0, if press.hovered() { crate::theme::mix(t.accent, egui::Color32::WHITE, 0.12) } else { t.accent });
    paint(&painter, egui::pos2(new.min.x + 14.0, new.min.y + 13.0), 18.0, &PLUS, ink);
    text_on_baseline(&painter, new.min.x + 40.0, new.min.y + 30.0, false, "New notebook", 14.0, ink);
    let enabled = ui.is_enabled();
    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "New notebook"));
    if press.clicked() {
        acts.push(Act::NewNotebook);
    }
    if notebooks.is_empty() {
        ui.label(egui::RichText::new("There are no notebooks yet. \"New\" starts one.").color(t.muted));
    }
    egui::ScrollArea::vertical().id_salt("backstage-info").show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 16.0;
        for n in notebooks {
            let (card, _) = ui.allocate_exact_size(vec2(width, 108.0), Sense::hover());
            let painter = ui.painter().clone();
            painter.rect_filled(card, 16.0, t.panel);
            painter.rect_stroke(card, 16.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
            // The cover, in the notebook's own colour.
            let color = n
                .info
                .color
                .map(|c| {
                    let (r, g, b) = c.rgb(t.panel.r() < 128);
                    egui::Color32::from_rgb(r, g, b)
                })
                .unwrap_or(t.notebook);
            let cover = egui::Rect::from_min_size(card.min + vec2(21.0, 24.0), vec2(48.0, 60.0));
            painter.rect_filled(cover, 6.0, color);
            let words = egui::Rect::from_min_max(egui::pos2(cover.min.x, card.min.y), egui::pos2(card.max.x - 260.0, card.max.y));
            let open = ui.interact(words, ui.id().with(("backstage-notebook", &n.path)), Sense::click());
            let name = n.info.title.clone();
            let enabled = ui.is_enabled();
            open.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, format!("Open {name}")));
            let title_colour = if open.hovered() { t.accent } else { t.text };
            if open.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Open this notebook").clicked() {
                acts.push(Act::Open(n.path.clone()));
            }
            text_on_baseline(&painter, card.min.x + 89.0, card.min.y + 43.5, false, &n.info.title, 18.0, title_colour);
            let colour = n.info.color.map(NodeColor::label).unwrap_or("no");
            text_on_baseline(
                &painter,
                card.min.x + 89.0,
                card.min.y + 65.5,
                false,
                &format!("{} items \u{b7} {colour} colour", n.count()),
                13.0,
                t.muted,
            );
            text_on_baseline_in(&painter, card.min.x + 89.0, card.min.y + 87.5, false, &show_path(&n.path), egui::FontId::monospace(12.0), t.muted);
            // What can be done to it: show it in the file manager, or open it.
            let go = egui::Rect::from_min_size(egui::pos2(card.max.x - 21.0 - 68.0, card.min.y + 32.0), vec2(68.0, 44.0));
            let folder_btn = egui::Rect::from_min_size(egui::pos2(go.min.x - 12.0 - 131.85, card.min.y + 32.0), vec2(131.85, 44.0));
            let show = ui.interact(folder_btn, ui.id().with(("backstage-folder", &n.path)), Sense::click());
            if show.hovered() {
                painter.rect_filled(folder_btn, 10.0, t.row_hover);
            }
            painter.rect_stroke(folder_btn, 10.0, egui::Stroke::new(1.0, crate::theme::mix(t.faint, t.muted, 0.55)), egui::StrokeKind::Inside);
            text_on_baseline(&painter, folder_btn.center().x, folder_btn.min.y + 27.0, true, "Open folder", 14.0, t.text);
            let enabled = ui.is_enabled();
            show.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Open folder"));
            if show.clicked() {
                acts.push(Act::OpenFolder(n.path.clone()));
            }
            let enter = ui.interact(go, ui.id().with(("backstage-enter", &n.path)), Sense::click());
            painter.rect_filled(go, 10.0, if enter.hovered() { crate::theme::mix(t.accent, egui::Color32::WHITE, 0.12) } else { t.accent });
            text_on_baseline(&painter, go.center().x, go.min.y + 27.0, true, "Open", 14.0, ink);
            let (enabled, name) = (ui.is_enabled(), format!("Open {} notebook", n.info.title));
            enter.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name.clone()));
            if enter.clicked() {
                acts.push(Act::Open(n.path.clone()));
            }
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new(format!("All of them live in {folder}")).size(12.0).color(t.muted));
        if crate::ribbon::SHOW_UNBUILT {
            ui.label(
                egui::RichText::new(
                    "Online notebooks (OneDrive, SharePoint) are not built. A notebook in a folder your sync tool watches is synced by it.",
                )
                .size(11.5)
                .color(t.muted),
            );
        }
    });
}

/// Sync, to a drawn design: the repository form, the switch for syncing by itself, the buttons, and on
/// the right how it works.
fn sync_page(ui: &mut Ui, sync: &mut crate::syncing::Syncing, t: &Tokens, acts: &mut Vec<Act>) {
    use crate::svgicon::{Prim, paint};
    use crate::viewtab::text_on_baseline;
    const EYE: [Prim; 2] = [Prim::Path("M2 12A10.64 10.64 0 0 1 22 12A10.64 10.64 0 0 1 2 12Z"), Prim::Circle(12.0, 12.0, 3.0)];
    const SYNC: [Prim; 1] = [Prim::Path("M20 11a8 8 0 00-14.3-4.9L4 8M4 4v4h4M4 13a8 8 0 0014.3 4.9L20 16M20 20v-4h-4")];
    let ink = egui::Color32::from_rgb(26, 16, 48);
    let line = crate::theme::mix(t.faint, t.muted, 0.7);
    let dim = crate::theme::mix(t.panel, t.muted, 0.6);
    let field_fill = crate::theme::mix(t.editor, egui::Color32::BLACK, 0.2);
    let pill_fill = crate::theme::mix(t.panel, t.muted, 0.2);
    let width = ui.available_width();
    let form_width = width.min(596.0);
    let painter = ui.painter().clone();
    let complete = ![&sync.form.url, &sync.form.username, &sync.form.token].iter().any(|v| v.trim().is_empty());
    let busy = sync.busy();
    // The title, and where the sync stands.
    let (head, _) = ui.allocate_exact_size(vec2(width, 100.0), Sense::hover());
    let title = text_on_baseline(&painter, head.min.x, head.min.y + 75.0, false, "Git sync", 28.0, t.text);
    let (state, dot) = if busy {
        ("Syncing", Some(t.accent))
    } else if sync.failed {
        ("Failed", Some(t.warn))
    } else if complete {
        ("Configured", Some(egui::Color32::from_rgb(0x6c, 0xd0, 0x8a)))
    } else {
        ("Not configured", None)
    };
    let words = painter.layout_no_wrap(state.into(), egui::FontId::proportional(12.0), t.muted).size().x;
    let pill = egui::Rect::from_min_size(egui::pos2(head.min.x + title + 14.0, head.min.y + 46.0), vec2(words + 34.0, 24.0));
    painter.rect_filled(pill, 12.0, pill_fill);
    let at = egui::pos2(pill.min.x + 14.0, pill.center().y);
    match dot {
        Some(c) => painter.circle_filled(at, 4.0, c),
        None => painter.circle_stroke(at, 3.5, egui::Stroke::new(1.0, t.muted)),
    };
    text_on_baseline(&painter, pill.min.x + 24.0, pill.min.y + 19.0, false, state, 12.0, t.muted);
    // The form's card.
    let top = ui.cursor().min;
    let (card, _) = ui.allocate_exact_size(vec2(form_width, 402.0), Sense::hover());
    painter.rect_filled(card, 16.0, t.panel);
    painter.rect_stroke(card, 16.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    let x = card.min.x + 25.0;
    let inner = form_width - 50.0;
    text_on_baseline(&painter, x, card.min.y + 45.5, false, "Repository", 16.0, t.text);
    let shown_id = ui.id().with("sync-token-shown");
    let mut shown: bool = ui.data_mut(|d| d.get_temp(shown_id).unwrap_or(false));
    let group = |ui: &mut Ui, y: f32, label: &str, hint: &str, value: &mut String, secret: bool, shown: &mut bool| {
        text_on_baseline(&painter, x, card.min.y + y + 33.0, false, label, 14.0, t.text);
        let field = egui::Rect::from_min_size(egui::pos2(x, card.min.y + y + 41.0), vec2(inner, 44.0));
        painter.rect_filled(field, 10.0, field_fill);
        let room = field.shrink2(vec2(13.0, 10.0));
        let room = if secret { egui::Rect::from_min_max(room.min, egui::pos2(room.max.x - 34.0, room.max.y)) } else { room };
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(room));
        let edit = child.add(
            egui::TextEdit::singleline(value)
                .frame(egui::Frame::NONE)
                .desired_width(f32::INFINITY)
                .password(secret && !*shown)
                .font(egui::FontId::proportional(15.0))
                .text_color(t.text),
        );
        edit.widget_info({
            let (enabled, said) = (true, label.to_string());
            move || egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, enabled, said.clone())
        });
        let focus = edit.has_focus();
        painter.rect_stroke(
            field,
            10.0,
            egui::Stroke::new(if focus { 2.0 } else { 1.0 }, if focus { t.accent } else { line }),
            egui::StrokeKind::Inside,
        );
        if secret {
            let eye = egui::Rect::from_min_size(egui::pos2(field.max.x - 36.0, field.min.y + 13.0), vec2(18.0, 18.0));
            let press = ui.interact(eye.expand(6.0), ui.id().with("sync-eye"), Sense::click());
            paint(&painter, eye.min, 18.0, &EYE, if *shown { t.accent } else { t.text });
            let (enabled, said) = (ui.is_enabled(), if *shown { "Hide the token" } else { "Show the token" });
            press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, said));
            if press.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                *shown = !*shown;
            }
        }
        text_on_baseline(&painter, x, card.min.y + y + 33.0 + 73.0, false, hint, 12.0, t.muted);
    };
    group(ui, 50.0, "Address", "GitHub or any Git server that takes a username and a token over HTTPS.", &mut sync.form.url, false, &mut shown);
    group(ui, 160.0, "Username", "On GitHub it is your username.", &mut sync.form.username, false, &mut shown);
    group(ui, 270.0, "Access token", "Kept in a file of its own, not with the settings.", &mut sync.form.token, true, &mut shown);
    ui.data_mut(|d| d.insert_temp(shown_id, shown));
    // The switch, and the time between syncs.
    ui.add_space(16.0);
    let (auto, _) = ui.allocate_exact_size(vec2(form_width, 136.0), Sense::hover());
    painter.rect_filled(auto, 16.0, t.panel);
    painter.rect_stroke(auto, 16.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    text_on_baseline(&painter, x, auto.min.y + 41.5, false, "Automatic sync", 16.0, t.text);
    text_on_baseline(&painter, x, auto.min.y + 60.0, false, "While NoteCraft is open and the server can be reached.", 12.0, t.muted);
    let on = sync.form.enabled;
    let switch = egui::Rect::from_min_size(egui::pos2(auto.max.x - 25.0 - 48.0, auto.min.y + 27.0), vec2(48.0, 28.0));
    let flip = ui.interact(switch, ui.id().with("sync-switch"), Sense::click());
    painter.rect_filled(switch, 14.0, if on { t.accent } else { pill_fill });
    if !on {
        painter.rect_stroke(switch, 14.0, egui::Stroke::new(1.0, line), egui::StrokeKind::Inside);
    }
    let knob = egui::pos2(if on { switch.max.x - 14.0 } else { switch.min.x + 14.0 }, switch.center().y);
    painter.circle_filled(knob, 10.0, if on { ink } else { t.muted });
    let enabled = ui.is_enabled();
    flip.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, on, "Automatic sync"));
    if flip.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        sync.form.enabled = !on;
    }
    let every = if on { t.text } else { dim };
    let every_words = text_on_baseline(&painter, x, auto.min.y + 103.0, false, "Every", 14.0, every);
    let box_ = egui::Rect::from_min_size(egui::pos2(x + every_words + 10.0, auto.min.y + 75.0), vec2(72.0, 40.0));
    painter.rect_filled(box_, 10.0, field_fill);
    painter.rect_stroke(box_, 10.0, egui::Stroke::new(1.0, if on { line } else { t.faint }), egui::StrokeKind::Inside);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(box_.shrink2(vec2(14.0, 8.0))));
    child.add_enabled(
        on,
        egui::TextEdit::singleline(&mut sync.form.minutes)
            .frame(egui::Frame::NONE)
            .desired_width(f32::INFINITY)
            .font(egui::FontId::proportional(14.0))
            .text_color(every),
    );
    text_on_baseline(&painter, box_.max.x + 10.0, auto.min.y + 103.0, false, "minutes", 14.0, every);
    // Save, sync now, and what is missing or what happened last.
    ui.add_space(16.0);
    let (row, _) = ui.allocate_exact_size(vec2(form_width, 44.0), Sense::hover());
    let save_words = painter.layout_no_wrap("Save".into(), egui::FontId::proportional(14.0), ink).size().x;
    let save = egui::Rect::from_min_size(row.min, vec2(save_words + 32.0, 44.0));
    let press = ui.interact(save, ui.id().with("sync-save"), Sense::click());
    painter.rect_filled(save, 10.0, if press.hovered() { crate::theme::mix(t.accent, egui::Color32::WHITE, 0.12) } else { t.accent });
    text_on_baseline(&painter, save.min.x + 16.0, save.min.y + 30.0, false, "Save", 14.0, ink);
    let enabled = ui.is_enabled();
    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Save"));
    if press.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        acts.push(Act::SyncSave(false));
    }
    let label = if busy { "Syncing..." } else { "Sync now" };
    let now_words = painter.layout_no_wrap(label.into(), egui::FontId::proportional(14.0), t.text).size().x;
    let now = egui::Rect::from_min_size(egui::pos2(save.max.x + 12.0, row.min.y), vec2(now_words + 60.0, 44.0));
    let ready = complete && !busy;
    let press = ui.interact(now, ui.id().with("sync-now"), if ready { Sense::click() } else { Sense::hover() });
    if press.hovered() && ready {
        painter.rect_filled(now, 10.0, t.row_hover);
    }
    painter.rect_stroke(now, 10.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    let ink_now = if ready { t.text } else { dim };
    paint(&painter, egui::pos2(now.min.x + 17.0, now.min.y + 13.0), 18.0, &SYNC, ink_now);
    text_on_baseline(&painter, now.min.x + 43.0, now.min.y + 30.0, false, label, 14.0, ink_now);
    let enabled = ready && ui.is_enabled();
    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Sync now"));
    if press.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && ready {
        acts.push(Act::SyncSave(true));
    }
    let note = if !sync.status.is_empty() {
        sync.status.clone()
    } else if !complete {
        "Fill in the address, the username and the token to sync.".to_string()
    } else {
        String::new()
    };
    let colour = if sync.failed && !sync.status.is_empty() { t.warn } else { t.muted };
    text_on_baseline(&painter, now.max.x + 12.0, row.min.y + 29.0, false, &note, 12.0, colour);
    // What the sync is doing now, or did last: a bar for the step, how long it has been going, and its log.
    if let Ok(live) = sync.live.lock()
        && !live.log.is_empty()
    {
        if sync.busy() {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                if let Some(started) = live.started {
                    ui.label(egui::RichText::new(format!("{} s", started.elapsed().as_secs())).color(t.muted));
                }
            });
            match live.fraction {
                Some(f) => ui.add(egui::ProgressBar::new(f).desired_width(420.0).show_percentage()),
                None => ui.add(egui::ProgressBar::new(0.0).desired_width(420.0).animate(true)),
            };
        }
        egui::Frame::NONE.fill(t.faint).corner_radius(4.0).inner_margin(6.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("sync-log").max_height(150.0).stick_to_bottom(true).show(ui, |ui| {
                ui.set_width(420.0);
                for line in &live.log {
                    ui.label(egui::RichText::new(line).monospace().size(11.0).color(t.muted));
                }
            });
        });
    }
    // How it works, to the right of the form when there is room for it.
    if width >= 860.0 {
        let side = width - 620.0;
        let mut y = top.y + 22.0;
        let mut block = |head: &str, parts: &[(&str, bool)]| {
            text_on_baseline(&painter, top.x + 620.0, y, false, head, 14.0, t.text);
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = side;
            for (text, code) in parts {
                let font = if *code { egui::FontId::monospace(13.0) } else { egui::FontId::proportional(13.0) };
                job.append(
                    text,
                    0.0,
                    egui::TextFormat { font_id: font, color: if *code { t.text } else { t.muted }, line_height: Some(19.0), ..Default::default() },
                );
            }
            let galley = painter.layout_job(job);
            let height = galley.size().y;
            painter.galley(egui::pos2(top.x + 620.0, y + 11.0), galley, t.muted);
            y += 11.0 + height + 22.0;
        };
        block("How it works", &[("The notebooks folder is a Git repository. Nothing is sent until you save these details.", false)]);
        block(
            "If you change the same file on two devices",
            &[
                ("Nothing is lost: this device keeps its version and the other one is saved beside it as ", false),
                ("name.conflict-xxxxxxx", true),
                (".", false),
            ],
        );
    }
}

/// Export: each notebook as one ZIP of its folder.
fn export_page(ui: &mut Ui, notebooks: &[Node], show_path: &dyn Fn(&std::path::Path) -> String, t: &Tokens, acts: &mut Vec<Act>) {
    heading(ui, "Export a notebook", t);
    ui.label(egui::RichText::new("A ZIP holds the notebook folder as it is, so unzipping it gives the notebook back.").color(t.muted));
    ui.add_space(8.0);
    if notebooks.is_empty() {
        ui.label(egui::RichText::new("There are no notebooks yet. \"New\" starts one.").color(t.muted));
    }
    egui::ScrollArea::vertical().id_salt("backstage-export").show(ui, |ui| {
        for n in notebooks {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&n.info.title).size(16.0).color(t.accent));
                    ui.label(egui::RichText::new(show_path(&n.path)).size(11.5).monospace().color(t.muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if open_action(ui, "Export as ZIP", 150.0, 30.0, t, true).clicked() {
                        acts.push(Act::ExportNotebook(n.path.clone()));
                    }
                });
            });
            ui.add_space(8.0);
        }
    });
}

/// New, to a drawn design: a card to start an empty notebook (name and colour), and one for the Welcome notebook.
fn new_page(ui: &mut Ui, notebooks: &[Node], folder: &str, show_path: &dyn Fn(&std::path::Path) -> String, t: &Tokens, acts: &mut Vec<Act>) {
    use crate::svgicon::{Prim, paint};
    use crate::viewtab::{text_on_baseline, text_on_baseline_in};
    const BOOK: [Prim; 2] = [Prim::Path("M4 5a2 2 0 012-2h12v18H6a2 2 0 01-2-2z"), Prim::Path("M8 3v18M12 8h4M12 12h4")];
    let ink = egui::Color32::from_rgb(26, 16, 48);
    let outline = crate::theme::mix(t.faint, t.muted, 0.55);
    let field_fill = crate::theme::mix(t.editor, egui::Color32::BLACK, 0.2);
    let width = ui.available_width().min(760.0);
    let dark = t.panel.r() < 128;
    let (name_id, colour_id) = (ui.id().with("new-notebook-name"), ui.id().with("new-notebook-colour"));
    let mut name: String = ui.data_mut(|d| d.get_temp(name_id).unwrap_or_default());
    let mut colour: Option<NodeColor> = ui.data_mut(|d| d.get_temp(colour_id).unwrap_or(Some(NodeColor::Purple)));
    let (head, _) = ui.allocate_exact_size(vec2(width, 100.0), Sense::hover());
    text_on_baseline(ui.painter(), head.min.x, head.min.y + 75.0, false, "New", 28.0, t.text);
    // The card to start an empty notebook.
    let (card, _) = ui.allocate_exact_size(vec2(width, 352.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(card, 16.0, t.panel);
    painter.rect_stroke(card, 16.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    let x = card.min.x + 25.0;
    text_on_baseline(&painter, x, card.min.y + 47.5, false, "Empty notebook", 18.0, t.text);
    text_on_baseline(&painter, x, card.min.y + 71.0, false, "It starts with one section and one page, ready to write in.", 14.0, t.muted);
    text_on_baseline(&painter, x, card.min.y + 112.0, false, "Name", 14.0, t.text);
    let field = egui::Rect::from_min_size(egui::pos2(x, card.min.y + 121.0), vec2(width - 50.0, 46.0));
    painter.rect_filled(field, 10.0, field_fill);
    // Placed without moving the page's cursor: the cards below are laid out from the card's own corner.
    let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(field.shrink2(vec2(13.0, 11.0))));
    let edit = inner.add(
        egui::TextEdit::singleline(&mut name)
            .frame(egui::Frame::NONE)
            .desired_width(f32::INFINITY)
            .font(egui::FontId::proportional(15.0))
            .text_color(t.text),
    );
    painter.rect_stroke(
        field,
        10.0,
        egui::Stroke::new(
            if edit.has_focus() { 2.0 } else { 1.0 },
            if edit.has_focus() { t.accent } else { crate::theme::mix(t.faint, t.muted, 0.7) },
        ),
        egui::StrokeKind::Inside,
    );
    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    text_on_baseline(&painter, x, card.min.y + 206.0, false, "Colour", 14.0, t.text);
    let choices = [
        Some(NodeColor::Purple),
        Some(NodeColor::Blue),
        Some(NodeColor::Green),
        Some(NodeColor::Orange),
        Some(NodeColor::Red),
        Some(NodeColor::Gray),
        None,
    ];
    for (i, choice) in choices.into_iter().enumerate() {
        let dot = egui::Rect::from_min_size(egui::pos2(x + 44.0 * i as f32, card.min.y + 225.0), vec2(24.0, 24.0));
        let label = choice.map(NodeColor::label).unwrap_or("No colour");
        let hit = ui.interact(dot.expand(4.0), ui.id().with(("new-notebook-swatch", i)), Sense::click());
        let enabled = ui.is_enabled();
        let chosen = colour == choice;
        hit.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, chosen, label));
        if chosen {
            painter.circle_filled(dot.center(), 16.0, t.text);
            painter.circle_filled(dot.center(), 14.0, t.panel);
        }
        match choice {
            Some(c) => {
                let (r, g, b) = c.rgb(dark);
                painter.circle_filled(dot.center(), 12.0, egui::Color32::from_rgb(r, g, b));
            }
            None => {
                painter.circle_stroke(dot.center(), 11.5, egui::Stroke::new(1.0, crate::theme::mix(t.faint, t.muted, 0.7)));
            }
        }
        if hit.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(label).clicked() {
            colour = choice;
        }
    }
    // The button, as wide as its words, and where the notebook will be written.
    let font = egui::FontId::proportional(14.0);
    let words = painter.layout_no_wrap("Create notebook".into(), font, ink).size().x;
    let create = egui::Rect::from_min_size(egui::pos2(x, card.min.y + 283.0), vec2(words + 32.0, 44.0));
    let ready = !name.trim().is_empty();
    let press = ui.interact(create, ui.id().with("new-notebook-create"), if ready { Sense::click() } else { Sense::hover() });
    let fill = if press.hovered() { crate::theme::mix(t.accent, egui::Color32::WHITE, 0.12) } else { t.accent };
    painter.rect_filled(create, 10.0, fill);
    text_on_baseline(&painter, create.min.x + 16.0, create.min.y + 30.0, false, "Create notebook", 14.0, ink);
    let enabled = ready && ui.is_enabled();
    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Create notebook"));
    let there = format!("In {}", show_path(std::path::Path::new(folder)));
    let after = create.max.x + 16.0;
    text_on_baseline(&painter, after, create.min.y + 26.25, false, "In ", 12.0, t.muted);
    text_on_baseline_in(&painter, after + 13.0, create.min.y + 27.25, false, &there[3..], egui::FontId::monospace(12.0), t.muted);
    if ready && (press.clicked() || enter) {
        acts.push(Act::CreateNotebook(name.trim().to_string(), colour));
        name.clear();
    }
    ui.data_mut(|d| {
        d.insert_temp(name_id, name);
        d.insert_temp(colour_id, colour);
    });
    // The card for the Welcome notebook.
    ui.add_space(24.0);
    let (card, _) = ui.allocate_exact_size(vec2(width, 90.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(card, 16.0, t.panel);
    painter.rect_stroke(card, 16.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    let tile = egui::Rect::from_min_size(card.min + vec2(25.0, 21.0), vec2(48.0, 48.0));
    painter.rect_filled(tile, 12.0, t.row_selected);
    paint(&painter, tile.min + vec2(12.0, 12.0), 24.0, &BOOK, t.accent);
    let title = "Welcome notebook";
    let words = text_on_baseline(&painter, card.min.x + 93.0, card.min.y + 43.5, false, title, 18.0, t.text);
    let present = notecraft_engine::welcome::installed(notebooks).is_some();
    if present {
        let badge = egui::Rect::from_min_size(egui::pos2(card.min.x + 93.0 + words + 10.0, card.min.y + 23.0), vec2(85.5, 20.0));
        painter.rect_filled(badge, 10.0, crate::theme::mix(t.panel, t.muted, 0.2));
        text_on_baseline(&painter, badge.min.x + 8.0, badge.min.y + 17.0, false, "Already here", 12.0, t.muted);
    }
    text_on_baseline(
        &painter,
        card.min.x + 93.0,
        card.min.y + 67.0,
        false,
        "A guided tour of NoteCraft, written in pages you can edit.",
        14.0,
        t.muted,
    );
    let label = if present { "Open" } else { "Add" };
    let go = egui::Rect::from_min_size(egui::pos2(card.max.x - 25.0 - 60.2, card.min.y + 23.0), vec2(60.2, 44.0));
    let press = ui.interact(go, ui.id().with("new-welcome"), Sense::click());
    if press.hovered() {
        painter.rect_filled(go, 10.0, t.row_hover);
    }
    painter.rect_stroke(go, 10.0, egui::Stroke::new(1.0, outline), egui::StrokeKind::Inside);
    text_on_baseline(&painter, go.center().x, go.min.y + 30.0, true, label, 14.0, t.text);
    let enabled = ui.is_enabled();
    press.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, format!("{label} the welcome notebook")));
    if press.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        acts.push(Act::AddWelcome);
    }
}

fn open_page(ui: &mut Ui, recent: &[Node], folder: &str, show_path: &dyn Fn(&std::path::Path) -> String, t: &Tokens, acts: &mut Vec<Act>) {
    heading(ui, "Open a notebook", t);
    ui.horizontal(|ui| {
        if ui.add_sized([230.0, 42.0], egui::Button::new(egui::RichText::new("Browse for a notebook…").size(15.0))).clicked() {
            acts.push(Act::ChooseFolder);
        }
        if open_action(ui, "Show notebooks folder", 210.0, 42.0, t, false).clicked() {
            acts.push(Act::OpenNotebooksFolder);
        }
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if open_action(ui, "Import notebook folder", 230.0, 36.0, t, true).clicked() {
            acts.push(Act::ImportNotebookFolder);
        }
        if open_action(ui, "Import file", 230.0, 36.0, t, true).clicked() {
            acts.push(Act::ImportFile);
        }
        let cloud = ui.add_enabled(false, egui::Button::new(egui::RichText::new("Import from the cloud").size(13.0)).min_size(vec2(190.0, 36.0)));
        cloud.on_hover_text("Cloud import is not implemented yet.");
    });
    ui.add_space(22.0);
    ui.label(egui::RichText::new("Recently opened").size(16.0).color(t.strong));
    ui.add_space(8.0);
    if recent.is_empty() {
        ui.label(egui::RichText::new("Notebooks you open will appear here.").size(13.0).color(t.muted));
        return;
    }
    egui::ScrollArea::vertical().id_salt("backstage-recent-notebooks").show(ui, |ui| {
        for notebook in recent {
            let width = ui.available_width().min(680.0);
            let (rect, response) = ui.allocate_exact_size(vec2(width, 70.0), Sense::click());
            let background = if response.hovered() { t.row_hover } else { t.panel };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 5.0, background);
            painter.rect_stroke(rect, 5.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
            let cover = egui::Rect::from_center_size(egui::pos2(rect.left() + 32.0, rect.center().y), vec2(40.0, 46.0));
            let color = notebook
                .info
                .color
                .map(|c| {
                    let (r, g, b) = c.rgb(t.panel.r() < 128);
                    egui::Color32::from_rgb(r, g, b)
                })
                .unwrap_or(t.notebook);
            icons::paint(&painter, cover, Icon::Notebook, color);
            painter.text(
                egui::pos2(rect.left() + 64.0, rect.center().y - 9.0),
                Align2::LEFT_CENTER,
                &notebook.info.title,
                egui::FontId::proportional(16.0),
                t.text,
            );
            painter.text(
                egui::pos2(rect.left() + 64.0, rect.center().y + 13.0),
                Align2::LEFT_CENTER,
                show_path(&notebook.path),
                egui::FontId::monospace(11.0),
                t.muted,
            );
            painter.text(egui::pos2(rect.right() - 18.0, rect.center().y), Align2::CENTER_CENTER, "›", egui::FontId::proportional(25.0), t.accent);
            let name = notebook.info.title.clone();
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Open {name}")));
            let path = notebook.path.clone();
            response.context_menu(|ui| {
                if ui.button("Remove from list").clicked() {
                    acts.push(Act::RemoveRecent(path.clone()));
                    ui.close();
                }
                if ui.button("Delete notebook...").clicked() {
                    acts.push(Act::DeleteRecent(path.clone(), name.clone()));
                    ui.close();
                }
            });
            if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                acts.push(Act::OpenRecent(notebook.path.clone()));
            }
            ui.add_space(6.0);
        }
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new(format!("Browse notebooks in {folder} or choose another folder.")).size(11.5).color(t.muted));
}

fn open_action(ui: &mut Ui, label: &str, width: f32, height: f32, t: &Tokens, primary: bool) -> egui::Response {
    let (fill, stroke, text) = if primary { (t.accent, t.accent, t.band_text) } else { (t.panel, t.faint, t.text) };
    let response = ui
        .add(
            egui::Button::new(egui::RichText::new(label).size(if primary { 13.5 } else { 14.0 }).color(text))
                .min_size(vec2(width, height))
                .fill(fill)
                .stroke(egui::Stroke::new(if primary { 1.0 } else { 0.8 }, stroke)),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label);
    if response.hovered() {
        ui.painter().rect_stroke(response.rect, 4.0, egui::Stroke::new(2.0, t.band_text), egui::StrokeKind::Inside);
    }
    if response.is_pointer_button_down_on() {
        ui.painter().rect_stroke(response.rect, 4.0, egui::Stroke::new(3.0, t.strong), egui::StrokeKind::Inside);
    }
    response
}

fn options_page(
    ui: &mut Ui,
    root: &std::path::Path,
    level: &str,
    session_id: Option<&str>,
    show_perf: bool,
    fps_limit: u32,
    compact_ui: bool,
    t: &Tokens,
    acts: &mut Vec<Act>,
) {
    heading(ui, "Options", t);
    ui.label(
        egui::RichText::new("Privacy is our top priority. Logs and telemetry stay on this device. NoteCraft does not send them to us.")
            .strong()
            .color(t.strong),
    );
    ui.add_space(12.0);
    ui.label(egui::RichText::new("Local diagnostics and usage").strong().size(15.0).color(t.strong));
    ui.label(egui::RichText::new("Choose what NoteCraft records locally. Nothing is uploaded automatically.").size(12.0).color(t.muted));
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Level:").strong());
        egui::ComboBox::from_id_salt("telemetry-level").selected_text(level).show_ui(ui, |ui| {
            for choice in ["none", "usage", "warning", "error", "debug"] {
                if ui.selectable_label(level == choice, choice).clicked() {
                    acts.push(Act::SetTelemetryLevel(choice.into()));
                    ui.close();
                }
            }
        });
    });
    if let Some(id) = session_id {
        ui.label(egui::RichText::new(format!("Current session: {id}")).strong().monospace().size(11.0).color(t.text));
    }
    ui.label(egui::RichText::new(format!("Folder: {}", root.display())).strong().size(11.0).color(t.text));
    ui.horizontal(|ui| {
        if open_action(ui, "Open telemetry folder", 190.0, 34.0, t, true).clicked() {
            acts.push(Act::OpenTelemetryFolder(root.to_path_buf()));
        }
        if open_action(ui, "Export current session ZIP", 220.0, 34.0, t, true).clicked() {
            acts.push(Act::ExportTelemetry(root.to_path_buf()));
        }
        if open_action(ui, "Clear telemetry", 150.0, 34.0, t, true).clicked() {
            acts.push(Act::ClearTelemetry(root.to_path_buf()));
        }
    });
    ui.add_space(12.0);
    ui.label(egui::RichText::new("Export creates a user-selected ZIP, then removes the exported session folder.").size(11.5).color(t.muted));
    ui.add_space(18.0);
    ui.separator();
    ui.label(egui::RichText::new("Toolbar layout").strong().size(15.0).color(t.strong));
    ui.label(
        egui::RichText::new("Compact: one row of tools, the rest in the header's menu. Ribbon: the tabs File, Home, Insert, Draw, History and View.")
            .size(11.5)
            .color(t.muted),
    );
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Layout:").strong());
        for (label, compact) in [("Compact", true), ("Ribbon", false)] {
            if ui.selectable_label(compact_ui == compact, label).clicked() && compact_ui != compact {
                acts.push(Act::SetCompactUi(compact));
            }
        }
    });
    ui.add_space(18.0);
    ui.separator();
    ui.label(egui::RichText::new("Frame rate").strong().size(15.0).color(t.strong));
    ui.label(
        egui::RichText::new(
            "Cap how many frames a second the window draws. A lower cap saves battery and heat; Off draws as fast as the screen allows.",
        )
        .size(11.5)
        .color(t.muted),
    );
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Limit:").strong());
        let name = |n: u32| if n == 0 { "Off".to_string() } else { format!("{n} fps") };
        egui::ComboBox::from_id_salt("fps-limit").selected_text(name(fps_limit)).show_ui(ui, |ui| {
            for n in crate::perf::FPS_LIMITS {
                if ui.selectable_label(fps_limit == n, name(n)).clicked() {
                    acts.push(Act::SetFpsLimit(n));
                    ui.close();
                }
            }
        });
    });
    ui.add_space(18.0);
    ui.separator();
    ui.label(egui::RichText::new("Performance counter").strong().size(15.0).color(t.strong));
    ui.label(
        egui::RichText::new("A small readout at the top right with the frame rate, the time frames take, processor and memory use, and the size of the page. Off by default.")
            .size(11.5)
            .color(t.muted),
    );
    let mut on = show_perf;
    if ui.checkbox(&mut on, "Show the performance counter").changed() {
        acts.push(Act::SetShowPerf(on));
    }
    ui.add_space(18.0);
    ui.separator();
    ui.label(egui::RichText::new("Application preferences").strong().size(15.0).color(t.strong));
    ui.label(
        egui::RichText::new("Reset themes, layout, ribbon choices, and other saved NoteCraft preferences to their defaults.")
            .size(11.5)
            .color(t.muted),
    );
    ui.colored_label(t.warn, "This does not delete notebooks or pages. It cannot be undone.");
    if open_action(ui, "Reset app preferences…", 210.0, 34.0, t, false).clicked() {
        acts.push(Act::ResetPreferences);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_side_entries_say_what_works() {
        let built: Vec<&str> = SIDE.iter().filter(|(_, b)| *b).map(|(n, _)| *n).collect();
        assert_eq!(built, vec!["Info", "New", "Open", "Sync", "Export", "Options"]);
        assert_eq!(SIDE.len(), 10, "the rest are listed and marked, not hidden");
        assert!(SIDE.iter().any(|(n, b)| *n == "Print" && !*b));
    }
}
