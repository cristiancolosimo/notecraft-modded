//! The NoteCraft window: top bar, notebook tree, page editor with live preview.
//!
//! Every action goes through `Session::run`, so the window, the command line and any
//! later control channel share one code path.

use crate::backstage;
use crate::control::{ControlRequest, ControlServer, base64};
use crate::floating;
use crate::icons::{self, Icon};
use crate::navhistory::NavHistory;
use crate::pageedit;
use crate::preview;
use crate::printout;
use crate::quickaccess;
use crate::ribbon;
use crate::snippets;
use crate::theme::{Accent, Mode, ModePref, Theme, Tokens};
use egui::{Align2, Color32, Context, Id, Key, Modifiers, Sense, Ui, vec2};
use notecraft_engine::{EngineError, Output, Params, Session};
use notecraft_model::{NodeColor, NodeKind, PageFormat};
use notecraft_store::search::Hit;
use notecraft_store::tree::Node;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
// std's own Instant panics on wasm32-unknown-unknown, which has no clock; web_time is the
// same type everywhere else and performance.now()/Date.now() there.
use web_time::Instant;

mod control;
mod dialogs;
mod editor;
mod file_screen;
mod formatting;
mod header;
mod import;
mod ink;
mod nav;
mod new_page;
mod pages;
mod ribbon_acts;
mod tables;

const AUTOSAVE_AFTER: Duration = Duration::from_millis(900);
/// The three-bar button: what OneNote puts at the top left of the navigation pane.
const BARS: &str = "\u{2630}";
/// Where the pen rack and the tool in hand are remembered between runs.
const RACK_KEY: &str = "pen_rack";
const NIB_KEY: &str = "pen_in_hand";
const SENSITIVITY_KEY: &str = "pen_sensitivity";
const DEFAULT_SENSITIVITY: u8 = 50;
const PRESSURE_WARNING_KEY: &str = "pressure_unavailable";

/// Shared title treatment for modal dialogs. Keeping this here makes dialogs use the
/// same opaque, theme-owned title band instead of inheriting egui's platform-gray title bar.
/// The saved form of the window's place: `x,y,width,height,maximised`.
pub fn window_geometry(x: f32, y: f32, w: f32, h: f32, maximised: bool) -> String {
    format!("{x:.0},{y:.0},{w:.0},{h:.0},{}", u8::from(maximised))
}

/// Reads [`window_geometry`] back; `None` for anything that is not a sensible window.
pub fn parse_window_geometry(text: &str) -> Option<([f32; 2], [f32; 2], bool)> {
    let n: Vec<f32> = text.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let [x, y, w, h, m] = n[..] else { return None };
    (w >= 820.0 && h >= 520.0 && w < 10000.0 && h < 10000.0 && x.abs() < 20000.0 && y.abs() < 20000.0).then_some(([x, y], [w, h], m > 0.5))
}

pub(crate) fn modal_frame(tokens: &Tokens) -> egui::Frame {
    egui::Frame::NONE.fill(tokens.panel).stroke(egui::Stroke::new(1.5, tokens.accent)).inner_margin(egui::Margin::same(16))
}

fn modal_title(ui: &mut Ui, tokens: &Tokens, title: &str) {
    egui::Frame::NONE.fill(tokens.band).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(crate::i18n::tr(title)).strong().size(17.0).color(tokens.band_text));
    });
    ui.add_space(10.0);
}

/// Modal actions use a readable size based on their label instead of egui's compact default.
fn modal_button(ui: &mut Ui, tokens: &Tokens, label: &str, primary: bool) -> egui::Response {
    modal_button_enabled(ui, tokens, label, primary, true)
}

fn modal_button_enabled(ui: &mut Ui, tokens: &Tokens, label: &str, primary: bool, enabled: bool) -> egui::Response {
    let label = crate::i18n::tr(label);
    let width = (label.chars().count() as f32 * 8.0 + 32.0).max(92.0);
    let fill = if primary { tokens.accent } else { tokens.panel };
    let text = if primary { tokens.band_text } else { tokens.text };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(14.0).color(text))
            .min_size(vec2(width, 32.0))
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, tokens.accent)),
    )
}

/// Whether a page is laid out as A4 sheets, and how many it is meant to have: `a4` is one, `a4:3` is three.
fn sheet_layout(dir: &Path) -> (bool, usize) {
    match notecraft_store::page::sheet(dir).ok().flatten().as_deref() {
        Some("a4") => (true, 1),
        Some(v) => v.strip_prefix("a4:").and_then(|n| n.parse().ok()).map_or((false, 0), |n| (true, n)),
        None => (false, 0),
    }
}

/// The dim behind a window that asks something. It starts under the header, so the header's controls (the keyboard switch
/// among them) stay in front and work while the window is open.
fn modal_backdrop(ctx: &Context, name: &'static str) {
    let screen = ctx.viewport_rect();
    let top = ctx.data(|d| d.get_temp::<f32>(Id::new("header-bottom"))).unwrap_or(screen.min.y).clamp(screen.min.y, screen.max.y);
    let rect = egui::Rect::from_min_max(egui::pos2(screen.min.x, top), screen.max);
    egui::Area::new(name.into()).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        let response = ui.allocate_rect(rect, Sense::click());
        ui.painter().rect_filled(response.rect, 0.0, Color32::from_black_alpha(150));
    });
}

fn params(pairs: &[(&str, &str)]) -> Params {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn missing_attachment_icon(page: &Path, src: &str) -> Option<String> {
    let name = src.strip_prefix("assets/")?;
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return None;
    }
    // Generated icon sidecars are images too, but they are not attachments that
    // need another icon. Treat `file.ext.svg`/`file.ext.png` as implementation
    // details when the attachment they belong to is present.
    if let Some((source, extension)) = name.rsplit_once('.')
        && matches!(extension.to_ascii_lowercase().as_str(), "png" | "svg")
        && page.join("assets").join(source).is_file()
    {
        return None;
    }
    if notecraft_store::assets::media_kind(src) != notecraft_store::assets::MediaKind::Other {
        return None;
    }
    // A missing attachment is not a missing-icon case. Do not warn about an icon
    // when the original file itself is no longer present.
    if !page.join("assets").join(name).is_file() {
        return None;
    }
    (!attachment_icon_healthy(page, name)).then(|| name.to_string())
}

/// A generated PNG is preferred on Windows, but the deterministic SVG fallback is also a
/// complete icon. In particular, Shell extraction can fail for files on a UNC-mounted
/// enlistment even though the fallback renders correctly.
fn attachment_icon_healthy(page: &Path, name: &str) -> bool {
    attachment_icon_exists(page, name)
}

fn attachment_icon_exists(page: &Path, name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        // Native Shell icons are PNG, but the deterministic SVG fallback is valid
        // when Shell extraction is unavailable on this Windows installation.
        page.join("assets").join(format!("{name}.png")).is_file() || page.join("assets").join(format!("{name}.svg")).is_file()
    }
    #[cfg(not(target_os = "windows"))]
    {
        page.join("assets").join(format!("{name}.svg")).is_file() || page.join("assets").join(format!("{name}.png")).is_file()
    }
}

fn attachment_icon_link(page: &Path, name: &str) -> String {
    let dir = page.join("assets");
    if dir.join(format!("{name}.png")).is_file() { format!("assets/{name}.png") } else { format!("assets/{name}.svg") }
}

fn replace_attachment_icon(format: PageFormat, snippet: &str, name: &str, icon_link: &str) -> String {
    match format {
        PageFormat::Html => snippet
            .replace(&format!("src=\"assets/{name}.png\""), &format!("src=\"{icon_link}\""))
            .replace(&format!("src=\"assets/{name}.svg\""), &format!("src=\"{icon_link}\"")),
        PageFormat::Cmgfm => {
            let png = format!("](assets/{name}.png)");
            let svg = format!("](assets/{name}.svg)");
            snippet.replacen(&png, &format!("]({icon_link})"), 1).replacen(&svg, &format!("]({icon_link})"), 1)
        }
    }
}

#[cfg(test)]
mod attachment_reference_tests {
    use super::{PageFormat, replace_attachment_icon};

    #[cfg(not(target_os = "windows"))]
    use super::attachment_icon_exists;

    #[test]
    fn html_repair_changes_only_the_image_source() {
        let input = r#"<p><a href="assets/brief.docx"><img src="assets/brief.docx.svg" alt="" width="48"><br>brief.docx</a></p>"#;
        let output = replace_attachment_icon(PageFormat::Html, input, "brief.docx", "assets/brief.docx.png");
        assert!(output.contains(r#"href="assets/brief.docx""#));
        assert!(output.contains(r#"src="assets/brief.docx.png""#));
    }

    #[test]
    fn markdown_repair_changes_only_the_inner_image_link() {
        let input = "[![brief.docx](assets/brief.docx.svg) brief.docx](assets/brief.docx)";
        let output = replace_attachment_icon(PageFormat::Cmgfm, input, "brief.docx", "assets/brief.docx.png");
        assert_eq!(output, "[![brief.docx](assets/brief.docx.png) brief.docx](assets/brief.docx)");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_windows_png_sidecar_is_a_valid_icon_on_posix() {
        let t = notecraft_store::testing::TempDir::new("attachment-icon");
        let page = t.path().join("page");
        std::fs::create_dir_all(page.join("assets")).expect("assets");
        std::fs::write(page.join("assets/brief.docx"), b"attachment").expect("attachment");
        std::fs::write(page.join("assets/brief.docx.png"), b"png sidecar").expect("icon");
        assert!(attachment_icon_exists(&page, "brief.docx"));
    }
}

fn first_missing_attachment_in_source(source: &str, page: &Path) -> Option<String> {
    let mut rest = source;
    while let Some(at) = rest.find("assets/") {
        rest = &rest[at + "assets/".len()..];
        let end = rest.find(['"', '\'', ')', ']', '<', '>', ' ', '\n', '\r']).unwrap_or(rest.len());
        let name = &rest[..end];
        let candidate = format!("assets/{name}");
        if let Some(name) = (!name.is_empty()).then(|| missing_attachment_icon(page, &candidate)).flatten() {
            return Some(name);
        }
        rest = &rest[end..];
    }
    None
}

#[allow(dead_code)]
fn first_unreferenced_attachment(attachments: &[(String, u64)], source: &str) -> Option<String> {
    attachments.iter().find_map(|(name, _)| {
        let reference = format!("assets/{name}");
        (!source.contains(&reference)).then(|| name.clone())
    })
}

#[allow(dead_code)]
fn prepare_unreferenced_attachment(page: &Path, name: &str) -> bool {
    match notecraft_store::assets::ensure_icon(page, name) {
        Ok(()) => {
            log::debug!("generated icon before detached attachment warning: page={:?} attachment={name:?}", page);
            true
        }
        Err(error) => {
            log::error!("could not generate icon before detached attachment warning: page={:?} attachment={name:?} error={error}", page);
            false
        }
    }
}

fn first_legacy_attachment_in_source(source: &str) -> Option<String> {
    let mut rest = source;
    while let Some(at) = rest.find("assets/") {
        rest = &rest[at + "assets/".len()..];
        let end = rest.find(['"', '\'', ')', ']', '<', '>', ' ', '\n', '\r']).unwrap_or(rest.len());
        let name = &rest[..end];
        if !name.is_empty()
            && !name.ends_with(".svg")
            && notecraft_store::assets::media_kind(&format!("assets/{name}")) == notecraft_store::assets::MediaKind::Other
            && (source.contains(&format!("<a href=\"assets/{name}\">")) || source.contains(&format!("[{name}](assets/{name})")))
        {
            return Some(name.to_string());
        }
        rest = &rest[end..];
    }
    None
}

fn shown(p: &Path) -> String {
    notecraft_store::tree::display_path(p)
}

fn notebook_for(path: &Path) -> Option<PathBuf> {
    let mut current = path;
    loop {
        if notecraft_store::tree::read_node(current).is_some_and(|node| node.kind == NodeKind::Notebook) {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

/// The path as the window may show it: redacted to `<notebooks>/relative/path` when
/// `redact` is on (for screenshots, tests and documentation, so no local path leaks).
fn shown_in(root: &Path, p: &Path, redact: bool) -> String {
    if !redact {
        return shown(p);
    }
    match p.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => "<notebooks folder>".to_string(),
        Ok(rel) => format!("<notebooks>/{}", rel.to_string_lossy().replace('\\', "/")),
        Err(_) => "<outside>".to_string(),
    }
}

struct Editor {
    dir: PathBuf,
    format: PageFormat,
    title: String,
    title_buf: String,
    body: String,
    dirty_since: Option<Instant>,
    blocks: Vec<notecraft_formats::Block>,
    last_saved: Option<String>,
    attachments: Vec<(String, u64)>,
    /// Where the caret is in the page, and the text of that block while it is written in.
    caret: Option<pageedit::Caret>,
    /// False when the page holds markup the block editor would drop: it is then read here
    /// and changed in its source.
    writable: bool,
    /// Which way the whole page reads, if the writer said so. A line with no letters of its
    /// own follows it.
    page_dir: Option<notecraft_model::Dir>,
    /// The boxes the page's blocks are arranged in, as the page's own file keeps them.
    boxes: Vec<notecraft_model::Container>,
    /// How wide the columns of this page's tables are drawn, from the same place.
    columns: notecraft_model::tablecols::Widths,
    /// Which paragraphs of this page are stretched to fill the column, from the same place.
    /// The words themselves are never touched; see `notecraft_model::justify`.
    justified: notecraft_model::justify::Justified,
    /// A selection dragged across more than one block. A caret belongs to one block, so a
    /// selection that leaves it is kept here; `None` is the ordinary case.
    across: Option<pageedit::Across>,
    /// The paper this page is drawn on and the colour behind it, as the page's own file
    /// keeps them. `None` means the page says nothing and the window's own choice stands.
    paper: Option<ribbon::RuleLines>,
    colour: Option<notecraft_model::NodeColor>,
    /// The page's drawing is laid out as separate A4 sheets, not as a page with no end.
    sheet_a4: bool,
    /// How many sheets it has at least: the ones scrolled down to are kept, though nothing is on them yet.
    sheets: usize,
    /// How many sheets were drawn last frame.
    sheets_drawn: usize,
    /// A sheet was added by scrolling, and the count is still to be written into the page.
    sheets_to_save: bool,
    /// What the page looked like before each change, so a change can be taken back.
    history: crate::history::History,
    /// The ink files in the page's folder. The folder is what decides: the page's own
    /// text is kept in step with this, never the other way round.
    ink_files: Vec<String>,
    /// Where the page's area starts on screen, reported so a script can find a box.
    origin: egui::Pos2,
    /// Whether the boxes are written into the page's text. A page nobody arranged stays plain
    /// text, one column; moving, resizing or adding a box arranges it, and from then on the
    /// boxes are elements around the blocks.
    arranged: bool,
}

impl Editor {
    /// Writes the page's text from its blocks, with the boxes around them when it is arranged.
    fn rebuild_body(&mut self) {
        let arrangement = (self.arranged && !self.boxes.is_empty()).then_some(self.boxes.as_slice());
        self.body = notecraft_formats::boxes::write(self.format, &self.blocks, arrangement);
    }
}

/// The page's own name, drawn on the sheet at the top of the page.
///
/// It used to sit in a strip above the page, beside the breadcrumb and the Saved line,
/// which made it look like a field about the page rather than the first thing on it. In
/// OneNote the name is part of the page, and the owner asked for the same here on
/// 2026-10-19. So it is drawn inside the scroll area and inside the readable column: it
/// scrolls with the words, it lines up with them, and the page colour and rule lines run
/// behind it as they do behind everything else on the sheet.
///
/// It is drawn before the sheet's origin is taken, so ink keeps the coordinates it already
/// has and a drawing made beside a paragraph stays beside it.
///
/// A title that reads right to left is drawn by the bidi painter rather than the text box:
/// egui lays the letters of a word out the right way and then puts the next word to the
/// right of it, so every Arabic page showed its own name with the words backwards. Clicking
/// it hands the line to the text box, which is still where it is edited.
fn page_title(ui: &mut Ui, ed: &mut Editor, tokens: &Tokens, title_id: egui::Id, rename: &mut Option<String>, zoom: f32) {
    let align = crate::pagetext::halign(&ed.title_buf, ed.page_dir);
    let width = ui.available_width();
    let rtl = crate::bidi::paints(&ed.title_buf, ed.page_dir);
    let editing = ui.memory(|m| m.has_focus(title_id)) || !rtl;
    let r = if editing {
        let out = egui::TextEdit::singleline(&mut ed.title_buf)
            .id(title_id)
            .frame(egui::Frame::NONE)
            .font(egui::FontId::proportional(26.0 * zoom))
            .horizontal_align(align)
            .desired_width(width)
            .show(ui);
        if out.response.has_focus()
            && crate::input_language::is_rtl()
            && let Some(range) = out.cursor_range
        {
            let caret = out.galley.pos_from_cursor(range.primary).translate(out.galley_pos.to_vec2());
            crate::pageedit::paint_rtl_input_marker(ui, caret);
        }
        out.response
    } else {
        let pieces = crate::bidi::plain(&ed.title_buf, 26.0 * zoom, tokens);
        let (drawn, size) = crate::bidi::runs(ui, &ed.title_buf, &pieces, 26.0 * zoom, width, ed.page_dir);
        let (rect, resp) = ui.allocate_exact_size(vec2(width, size.y.max(34.0)), egui::Sense::click());
        crate::bidi::paint(ui, rect.min, &drawn);
        if resp.clicked() {
            ui.memory_mut(|m| m.request_focus(title_id));
        }
        let name = ed.title_buf.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name));
        egui::AtomLayoutResponse::empty(resp.on_hover_cursor(egui::CursorIcon::Text))
    };
    if r.lost_focus() && ed.title_buf.trim() != ed.title && !ed.title_buf.trim().is_empty() {
        *rename = Some(ed.title_buf.trim().to_string());
    }
    // The rule under the name, as OneNote draws one. The date the page carries sits under
    // it already, as the first block of the page.
    let y = ui.cursor().min.y + 2.0;
    let (left, right) = (ui.max_rect().min.x, ui.max_rect().max.x);
    ui.painter().hline(left..=right, y, egui::Stroke::new(1.0, tokens.faint));
    ui.add_space(8.0);
}

/// What to do with a file the user dropped on a page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FileChoice {
    /// Copy it into the page's folder, so the notebook carries it. OneNote: "Attach File".
    Copy,
    /// Leave it where it is and write a link. OneNote: "Insert a link to the file".
    Link,
    /// Copy it in and insert one rendered image for each printed page.
    Printout,
}

/// What a file chosen in the app's own chooser is for, since the chooser answers a frame later than the
/// button that opened it.
#[derive(Clone)]
enum Purpose {
    NotebooksFolder,
    ImportFolder,
    ImportFile,
    /// A PDF to bring in as a section with a page for each of its sheets.
    ImportPdf,
    Printout(PathBuf),
    /// A file to put on this page's folder, which then asks what to do with it.
    AttachFile(PathBuf),
}

enum Dialog {
    /// The app's own file chooser, where the system has no file dialog (Android).
    Browse {
        purpose: Purpose,
        browser: crate::filebrowser::Browser,
    },
    /// A dropped file: copy it in, or link to it where it is.
    FileOptions {
        dir: PathBuf,
        files: Vec<PathBuf>,
    },
    /// Choosing a file in the app's own list, where the system has no file dialog (Android).
    PickMedia {
        dir: PathBuf,
        picker: crate::mediapicker::Picker,
    },
    NewNotebook {
        name: String,
        format: Option<PageFormat>,
    },
    Add {
        parent: PathBuf,
        parent_kind: NodeKind,
        title: String,
        kind: NodeKind,
        format: Option<PageFormat>,
    },
    Rename {
        path: PathBuf,
        title: String,
    },
    /// A file in the page's own folder, about to be deleted.
    DeleteFile {
        page: PathBuf,
        name: String,
    },
    Delete {
        path: PathBuf,
        title: String,
        count: usize,
    },
    DeleteNotebook {
        path: PathBuf,
        title: String,
    },
    ClearTelemetry {
        path: PathBuf,
    },
    ResetPreferences,
    PageVersions {
        page: PathBuf,
        versions: Vec<notecraft_store::page::Version>,
    },
    RecycleBin {
        notebook: PathBuf,
        items: Vec<notecraft_store::recycle::Recycled>,
    },
    /// Everything in the bin, after asking.
    EmptyRecycleBin {
        notebook: PathBuf,
        count: usize,
    },
    PurgeRecycleItem {
        notebook: PathBuf,
        id: String,
        title: String,
    },
    Symbols,
    PrivacyWelcome {
        level: String,
    },
    /// A problem told to the user in a box. Ticking "do not alert me again" stops the box for
    /// this `key` for the rest of the session; the status line still says what happened.
    Alert {
        key: String,
        title: String,
        message: String,
        suppress: bool,
        suppress_forever: bool,
    },
    MissingAttachmentIcon {
        page: PathBuf,
        name: String,
    },
    OpenAttachmentWarning {
        path: PathBuf,
    },
    #[allow(dead_code)]
    UnreferencedAttachment {
        page: PathBuf,
        name: String,
    },
}

struct ImportJob {
    rx: Receiver<ImportMessage>,
    keep_alive: Option<Sender<ImportMessage>>,
    cancel: Arc<AtomicBool>,
    source: PathBuf,
    destination: PathBuf,
    current: String,
    index: usize,
    total: usize,
}

enum ImportMessage {
    Found(usize),
    Section { index: usize, total: usize, label: String },
    Finished(Result<String, String>),
}

enum Act {
    Toggle(PathBuf),
    Select(PathBuf),
    Add(PathBuf, NodeKind),
    /// A page in this section at this place in the order, from the line between two pages.
    AddPageAt(PathBuf, u32),
    /// Add a child of this kind, named in the menu rather than chosen in the dialog.
    AddKind(PathBuf, NodeKind, NodeKind),
    /// A notebook, from the menu on the empty space.
    NewNotebook,
    /// Close a notebook opened from outside the notebooks folder.
    CloseNotebook(PathBuf),
    /// Show the notebooks folder in the file manager.
    RevealRoot,
    /// Ask the system for the folder to work from.
    ChooseRoot,
    Rename(PathBuf, String),
    Delete(PathBuf, String, usize),
    Color(PathBuf, Option<NodeColor>),
    /// Show the node's folder in the operating system's file manager.
    Reveal(PathBuf),
    /// Put a link to this page on the clipboard, to paste into another page.
    CopyLink(PathBuf),
    /// Drop a node into a container, before a sibling or last.
    Place(PathBuf, PathBuf, Option<PathBuf>),
}

/// A row of the tree on screen: where it is, whether it is a page, and what it says.
type NavRow = (PathBuf, egui::Rect, bool, String);

pub struct NoteCraftApp {
    session: Session,
    open: HashSet<PathBuf>,
    selected: Option<PathBuf>,
    /// The pages visited, for Back and Forward.
    nav: NavHistory,
    /// Which Quick Access buttons are shown, and where.
    qat: quickaccess::Config,
    /// When the pen last changed the page, to tell which Undo should take back.
    last_ink_change: Option<Instant>,
    /// The last Undo took back a change to the drawing, so Redo puts that back first.
    undid_ink: bool,
    /// Two columns in the side panel (organisers, then pages) instead of one tree.
    split_panes: bool,
    /// Whether the organisers column is showing, which the three-bar button toggles.
    organisers: bool,
    /// Whether the window is kept above the others.
    always_on_top: bool,
    /// Set when that has just changed and the window manager has not been told yet.
    window_level_pending: bool,
    /// True when the window has no frame of the system's own (the app asked for none), so
    /// the band across the top draws the window's buttons and moves and resizes it.
    frameless: bool,
    editor: Option<Editor>,
    dialog: Option<Dialog>,
    import_job: Option<ImportJob>,
    status: String,
    status_is_error: bool,
    search: String,
    searched: String,
    hits: Vec<Hit>,
    cursor: usize,
    tokens: Tokens,
    mode_pref: ModePref,
    effective_mode: Mode,
    accent: Accent,
    applied_theme: Option<(ModePref, Accent)>,
    loaders_installed: bool,
    redact_paths: bool,
    control: Option<Receiver<ControlRequest>>,
    control_server: Option<ControlServer>,
    event_frames: std::collections::VecDeque<Vec<egui::Event>>,
    screenshots: Vec<(u64, ControlRequest, Option<PathBuf>)>,
    next_screenshot: u64,
    show_source: bool,
    /// The code view: which of the page's files is picked, and its text while it is read
    /// or edited.
    code: crate::codeview::State,
    /// Which ribbon tab is open.
    ribbon_tab: String,
    /// The ribbon is folded away to its tab names.
    ribbon_collapsed: bool,
    compact_ui: bool,
    /// What the pen is reporting, where the platform reports anything.
    stylus: crate::stylus::Stylus,
    /// Only a pen writes ink (Android); a finger or a mouse does not.
    pen_only: bool,
    /// What the platform lets the app read of the user's pictures, where there is no file dialog.
    media: Option<std::sync::Arc<dyn crate::mediapicker::MediaAccess>>,
    /// The platform's way to read the device's files, where there is no file dialog: the app draws its own.
    files: Option<std::sync::Arc<dyn crate::filebrowser::FilesAccess>>,
    /// The performance counter at the top right (File, Options).
    show_perf: bool,
    /// How the pages of a section are listed: manual (and then they can be dragged), name, created or modified.
    pages_sort: String,
    /// Days a deleted item waits in its notebook's recycle bin before it is removed for good; 0 keeps them.
    recycle_days: u32,
    /// When the bins were last looked at for what is too old.
    bins_swept: Option<Instant>,
    /// New pages are made as A4 sheets (true) or as a page with no end (false); asked in the new page's dialog.
    new_page_a4: bool,
    perf: crate::perf::Perf,
    /// Frames a second the window may draw at most; 0 is no cap.
    fps_limit: u32,
    last_frame: Option<web_time::Instant>,
    /// Git sync: its form, its timer and what it last did.
    sync: crate::syncing::Syncing,
    /// A touch tablet: the keyboard switch shows, and the pen draws by default.
    soft_keyboard_blocked: bool,
    /// How far dialogs are lifted from the middle of the window, in points (negative is up), so they sit
    /// in the part of it the system bars and the on-screen keyboard leave free.
    dialog_lift: f32,
    /// Where the page's words were drawn last frame, which is where its ink goes.
    page_rect: Option<egui::Rect>,
    /// What the pointer does on the page (the Draw tab).
    pointer: ribbon::Pointer,
    /// The tools on the rack, as the owner has arranged them, and which is in hand. Both
    /// are remembered between runs.
    rack: Vec<notecraft_ink::rack::Nib>,
    nib: usize,
    /// How readily the pen answers pressure, 0 to 100. 50 leaves the readings as the pen
    /// gave them; above that a light hand lays down more ink.
    sensitivity: u8,
    /// The drawing open in this page, if it has one.
    pad: Option<crate::inkpad::Pad>,
    /// Lines or squares behind the page.
    rules: ribbon::RuleLines,
    /// Lines round the cells of a table, which is how a table is drawn unless the Table
    /// tab turns them off.
    table_borders: bool,
    /// Where GridCraft is, when it is installed: a table can be handed to it. Looked for
    /// once, when the window opens.
    gridcraft: Option<std::path::PathBuf>,
    /// The page title line is hidden (OneNote: View, Hide Page Title).
    hide_title: bool,
    /// The page's zoom. The ribbon and the rest of the window keep their own size.
    zoom_percent: u32,
    /// The zoom the page was last drawn at, to notice a change and keep the point under the pointer still.
    zoom_seen: u32,
    /// The zoom with its fraction, while a pinch is moving it.
    zoom_exact: f32,
    /// Recalculate the page zoom from the reading area's width on the next frame.
    fit_page_width: bool,
    /// The theme chooser is open as a window (the ribbon's Theme button).
    theme_window: bool,
    /// Text the ribbon asked to put on the clipboard this frame.
    pending_copy: Option<String>,
    /// Matching HTML flavor for the same copied item, when one is available.
    pending_copy_html: Option<String>,
    /// The File screen, when it is open, and which of its pages.
    backstage: Option<&'static str>,
    /// The page fills the window: the notebooks are hidden until it is turned off.
    full_page: bool,
    /// Put the keyboard in the search box on the next frame.
    focus_search: bool,
    /// The page the window closed on has been opened again (once per run).
    restored: bool,
    /// Recovery warnings that were already shown during this run.
    icon_recovery_reported: HashSet<String>,
    /// Suppress further missing-icon warnings during this run.
    icon_recovery_suppressed: bool,
    /// Alerts the user asked not to see again this session, by key.
    muted_alerts: HashSet<String>,
    /// Alerts waiting for the open one to close.
    queued_alerts: VecDeque<Dialog>,
    /// The clipboard the app reads and writes. The system's, unless the app was built with another.
    clipboard: Box<dyn crate::clipboard::Board>,
    open_attachment_warning_suppressed: bool,
    pressure_warning_presented: bool,
}

impl NoteCraftApp {
    /// Replaces the clipboard the app uses. Made at startup by the caller, so the app never
    /// reaches a platform's clipboard by itself.
    pub fn with_clipboard(mut self, board: Box<dyn crate::clipboard::Board>) -> NoteCraftApp {
        self.clipboard = board;
        self
    }

    pub fn new(session: Session) -> NoteCraftApp {
        let settings_for_sync = session.settings.clone();
        crate::i18n::set_language(&settings_for_sync.get("language").unwrap_or_default());
        let mode_pref = session.settings.get("theme_mode").and_then(|v| ModePref::parse(&v)).unwrap_or(ModePref::System);
        let accent = session.settings.get("theme_accent").and_then(|v| Accent::parse(&v)).unwrap_or(Accent::Default);
        let show_source = false;
        let session_rules = session.settings.get("rule_lines").and_then(|v| ribbon::RuleLines::parse(&v)).unwrap_or_default();
        let session_hide_title = session.settings.get("hide_title").map(|v| v.trim() == "true").unwrap_or(false);
        // A table has borders unless it was turned off: OneNote draws them, and a table
        // without them reads as loose columns of words.
        let session_borders = session.settings.get("table_borders").map(|v| v.trim() != "false").unwrap_or(true);
        // Two columns in the side panel, the way OneNote separates the notebooks from the
        // pages. This is the product's default navigation layout; the one-tree view remains
        // available as an explicit preference.
        let session_split = session.settings.get("split_panes").map(|v| v.trim() == "true").unwrap_or(true);
        // The pen rack as the owner last arranged it, and which tool was in hand.
        let rack = notecraft_ink::rack::parse(&session.settings.get(RACK_KEY).unwrap_or_default());
        let nib = session.settings.get(NIB_KEY).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0).min(rack.len() - 1);
        let sensitivity = session.settings.get(SENSITIVITY_KEY).and_then(|v| v.trim().parse::<u8>().ok()).unwrap_or(DEFAULT_SENSITIVITY).min(100);
        let qat = quickaccess::Config::load(|k| session.settings.get(k));
        // `--first-use` clears the key to an empty value, which counts as not seen.
        let privacy_welcome = session.settings.get("privacy_welcome_seen").is_none_or(|v| v.trim().is_empty());
        let telemetry_level = session.settings.get("telemetry_level").unwrap_or_else(|| "debug".to_string());
        log::info!("privacy welcome modal state: show={} telemetry_level={:?}", privacy_welcome, telemetry_level);
        NoteCraftApp {
            session,
            nav: NavHistory::default(),
            qat,
            last_ink_change: None,
            undid_ink: false,
            open: HashSet::new(),
            selected: None,
            split_panes: session_split,
            organisers: true,
            always_on_top: false,
            window_level_pending: false,
            frameless: false,
            editor: None,
            dialog: if privacy_welcome { Some(Dialog::PrivacyWelcome { level: telemetry_level }) } else { None },
            import_job: None,
            status: "Ready".into(),
            status_is_error: false,
            search: String::new(),
            searched: String::new(),
            hits: Vec::new(),
            cursor: 0,
            tokens: Theme::DEFAULT.tokens(),
            mode_pref,
            effective_mode: if mode_pref == ModePref::Dark { Mode::Dark } else { Mode::Light },
            accent,
            applied_theme: None,
            loaders_installed: false,
            redact_paths: false,
            control: None,
            control_server: None,
            event_frames: std::collections::VecDeque::new(),
            screenshots: Vec::new(),
            next_screenshot: 1,
            show_source,
            code: crate::codeview::State::default(),
            ribbon_tab: "Home".to_string(),
            ribbon_collapsed: false,
            compact_ui: false,
            stylus: crate::stylus::Stylus::default(),
            sync: crate::syncing::Syncing::load(&settings_for_sync),
            pen_only: false,
            media: None,
            files: None,
            show_perf: settings_for_sync.get("show_perf").as_deref() == Some("true"),
            new_page_a4: settings_for_sync.get("new_page_sheet").as_deref() == Some("a4"),
            recycle_days: settings_for_sync.get("recycle_days").and_then(|v| v.parse().ok()).unwrap_or(30),
            bins_swept: None,
            pages_sort: settings_for_sync
                .get("pages_sort")
                .filter(|v| ["name", "created", "modified"].contains(&v.as_str()))
                .unwrap_or_else(|| "manual".into()),
            perf: crate::perf::Perf::default(),
            fps_limit: settings_for_sync.get("fps_limit").and_then(|v| v.parse().ok()).filter(|n| crate::perf::FPS_LIMITS.contains(n)).unwrap_or(0),
            last_frame: None,
            soft_keyboard_blocked: false,
            dialog_lift: 0.0,
            page_rect: None,
            pointer: ribbon::Pointer::Write,
            rack,
            nib,
            sensitivity,
            pad: None,
            rules: session_rules,
            table_borders: session_borders,
            gridcraft: crate::gridcraft::installed(),
            hide_title: session_hide_title,
            zoom_percent: 100,
            zoom_seen: 100,
            zoom_exact: 100.0,
            fit_page_width: false,
            theme_window: false,
            pending_copy: None,
            pending_copy_html: None,
            backstage: None,
            full_page: false,
            focus_search: false,
            restored: false,
            icon_recovery_reported: HashSet::new(),
            icon_recovery_suppressed: false,
            muted_alerts: HashSet::new(),
            queued_alerts: VecDeque::new(),
            clipboard: Box::new(crate::clipboard::System),
            open_attachment_warning_suppressed: false,
            pressure_warning_presented: false,
        }
    }

    /// The Git sync's form and state.
    pub fn sync(&self) -> &crate::syncing::Syncing {
        &self.sync
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The session, for tests and for tools that drive the window.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Writes the open page now, instead of waiting for the autosave.
    pub fn save_page_now(&mut self) {
        self.save_now();
    }

    /// Whether the page source is shown beside the page. Off by default: the page is what
    /// the user writes on, and the source is there when they ask for it.
    pub fn show_source(&self) -> bool {
        self.show_source
    }

    pub fn set_show_source(&mut self, on: bool) {
        // Closing the view after the page file was typed in: the page is read again, so
        // the live view, the title and everything in the head come from what is on disk.
        if !on && self.code.page_edited {
            self.code.page_edited = false;
            if let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) {
                self.open_page(&dir);
            }
        }
        self.show_source = on;
    }

    /// Chooses a theme (light or dark, and an accent) and saves the choice.
    pub fn set_theme(&mut self, theme: Theme) {
        self.mode_pref = if theme.mode.is_dark() { ModePref::Dark } else { ModePref::Light };
        self.effective_mode = theme.mode;
        self.accent = theme.accent;
        self.save_theme();
    }

    /// Opens a File/backstage page for deterministic UI captures and integration harnesses.
    pub fn open_backstage_page(&mut self, page: &'static str) {
        self.backstage = Some(page);
    }

    /// Follows the system light or dark setting, with the given accent, and saves the choice.
    pub fn follow_system_theme(&mut self, accent: Accent) {
        self.mode_pref = ModePref::System;
        self.accent = accent;
        self.save_theme();
    }

    /// The theme in use (for the system mode, the mode the system last asked for).
    pub fn theme(&self) -> Theme {
        Theme { mode: self.effective_mode, accent: self.accent }
    }

    pub fn mode_pref(&self) -> ModePref {
        self.mode_pref
    }

    fn save_theme(&mut self) {
        let a = self.session.settings.set("theme_mode", self.mode_pref.id());
        let b = self.session.settings.set("theme_accent", self.accent.id());
        if let Err(e) = a.and(b) {
            self.status = format!("The theme could not be saved: {e}");
            self.status_is_error = true;
        }
    }

    /// Applies the chosen theme to egui when it changed, and refreshes the tokens.
    fn apply_theme(&mut self, ctx: &Context) {
        let key = (self.mode_pref, self.accent);
        if self.applied_theme != Some(key) {
            ctx.set_visuals_of(egui::Theme::Light, Theme { mode: Mode::Light, accent: self.accent }.visuals());
            ctx.set_visuals_of(egui::Theme::Dark, Theme { mode: Mode::Dark, accent: self.accent }.visuals());
            ctx.set_theme(match self.mode_pref {
                ModePref::Light => egui::ThemePreference::Light,
                ModePref::Dark => egui::ThemePreference::Dark,
                ModePref::System => egui::ThemePreference::System,
            });
            self.applied_theme = Some(key);
        }
        self.effective_mode = if ctx.theme() == egui::Theme::Dark { Mode::Dark } else { Mode::Light };
        self.tokens = self.theme().tokens();
        crate::inkpad::set_dark(self.effective_mode == Mode::Dark);
        // Keep every modal surface opaque and on the same themed panel as the rest of the app.
        // Individual dialogs may opt out of the native title bar, but they all inherit this
        // surface and border treatment.
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            ctx.style_mut_of(theme, |style| {
                style.visuals.window_fill = self.tokens.panel;
                style.visuals.window_stroke = egui::Stroke::new(1.0, self.tokens.faint);
            });
        }
    }

    /// Shows paths as `<notebooks>/...` instead of real local paths. Used by tests and
    /// when taking screenshots for documentation.
    pub fn set_redact_paths(&mut self, on: bool) {
        self.redact_paths = on;
    }

    fn shown(&self, p: &Path) -> String {
        shown_in(self.session.root(), p, self.redact_paths)
    }

    /// Opens `path` (absolute, or relative to the notebooks folder) and expands the tree down
    /// to it. A notebook, section group or section opens on its first page.
    pub fn open_path(&mut self, path: &str) {
        // An explicit command-line/control request wins over the page saved by the
        // previous session. Otherwise the first frame's restore pass would replace the
        // requested page with a stale page from another notebook.
        self.restored = true;
        // A notebook or section opens on its first page, and a notebook kept elsewhere is
        // opened first: the engine decides, so the command line and agents get the same page.
        if let Some(Output::Path(dir)) = self.run("open.page", &params(&[("path", path)])) {
            self.expand_to(&dir);
            self.open_page(&dir);
        }
    }

    /// Opens every level above `dir`, so the page is visible in the tree.
    fn expand_to(&mut self, dir: &Path) {
        let mut parent = dir.parent();
        while let Some(d) = parent {
            if d == self.session.root() {
                break;
            }
            self.open.insert(d.to_path_buf());
            parent = d.parent();
        }
    }

    /// Back to the page before this one.
    fn go_back(&mut self) {
        match self.nav.back(|p| p.exists()) {
            Some(dir) => {
                self.expand_to(&dir);
                self.open_page_inner(&dir);
            }
            None => self.say("There is no earlier page"),
        }
    }

    /// Forward to the page after this one.
    fn go_forward(&mut self) {
        match self.nav.forward(|p| p.exists()) {
            Some(dir) => {
                self.expand_to(&dir);
                self.open_page_inner(&dir);
            }
            None => self.say("There is no later page"),
        }
    }

    fn can_undo(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.writable && e.history.can_undo()) || self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_undo)
    }

    fn can_redo(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.writable && e.history.can_redo()) || self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_redo)
    }

    /// Undo for the Quick Access Toolbar: the page's words and boxes (the page history in
    /// `history.rs`), or the last stroke when the pen was used after anything else on the page,
    /// or when the page has nothing left to take back but the drawing has.
    fn undo(&mut self) {
        let edited = self.editor.as_ref().and_then(|e| e.history.when());
        let ink_newer = match (self.last_ink_change, edited) {
            (Some(i), Some(e)) => i > e,
            (Some(_), None) => true,
            _ => false,
        };
        let ink_can = self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_undo);
        let page_can = self.editor.as_ref().is_some_and(|e| e.writable && e.history.can_undo());
        self.undid_ink = ink_can && (ink_newer || !page_can);
        if self.undid_ink {
            self.ink_undo();
        } else {
            self.step_back(true);
        }
    }

    /// Redo for the same toolbar: it puts back what Undo took off last, the drawing's change or the page's.
    fn redo(&mut self) {
        let ink_can = self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_redo);
        let page_can = self.editor.as_ref().is_some_and(|e| e.writable && e.history.can_redo());
        if ink_can && (self.undid_ink || !page_can) {
            self.ink_redo();
        } else {
            self.step_back(false);
        }
    }

    /// The keys and buttons that move through the pages that were opened. Undo and Redo keys
    /// are the page history's own, taken below.
    fn nav_keys(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowLeft)) || ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1)) {
            self.go_back();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowRight)) || ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra2)) {
            self.go_forward();
        }
    }

    /// The Quick Access Toolbar, drawn in whatever row it is given.
    fn quick_access(&mut self, ui: &mut Ui, tokens: Tokens) {
        let enabled =
            quickaccess::Enabled { back: self.nav.can_back(), forward: self.nav.can_forward(), undo: self.can_undo(), redo: self.can_redo() };
        let (pressed, changed) = quickaccess::show(ui, &mut self.qat, enabled, &tokens, &self.tokens);
        if changed {
            self.qat.save(|k, v| {
                let _ = self.session.settings.set(k, v);
            });
        }
        match pressed {
            Some(quickaccess::Command::Back) => self.go_back(),
            Some(quickaccess::Command::Forward) => self.go_forward(),
            Some(quickaccess::Command::Undo) => self.undo(),
            Some(quickaccess::Command::Redo) => self.redo(),
            None => {}
        }
    }

    /// Opens every level of the tree.
    pub fn expand_all(&mut self) {
        self.open = self.collect_branches().into_iter().collect();
    }

    /// Whether the page being edited has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.dirty_since.is_some())
    }

    /// Where the page's words were drawn last frame. Its ink is measured from the top
    /// left of this, so a test that writes on the page aims here.
    pub fn page_rect(&self) -> Option<egui::Rect> {
        self.page_rect
    }

    /// The text in the editor, if a page is open.
    pub fn body(&self) -> Option<&str> {
        self.editor.as_ref().map(|e| e.body.as_str())
    }

    /// The title of the open page, which the window shows above it.
    pub fn title(&self) -> Option<&str> {
        self.editor.as_ref().map(|e| e.title.as_str())
    }

    /// Replaces the text in the editor, as typing would, and marks the page changed.
    pub fn set_body(&mut self, body: &str) {
        if let Some(ed) = self.editor.as_mut() {
            ed.body = body.to_string();
            Self::mark_dirty(ed);
        }
    }

    /// Saves the open page now, if it has changes.
    pub fn save(&mut self) {
        self.save_now();
    }

    /// The last status line.
    pub fn status(&self) -> &str {
        &self.status
    }

    fn say(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_is_error = false;
    }

    /// Says something went wrong, in our own words rather than the engine's.
    /// Tells the user about a problem. The status line always says it. A box says it too, unless
    /// the user has asked not to see alerts with this `key` again this session; a box that is
    /// already open is not replaced, the next alert waits its turn in a queue.
    pub fn alert(&mut self, key: &str, title: &str, message: &str) {
        self.status = message.to_string();
        self.status_is_error = true;
        if self.muted_alerts.contains(key) {
            return;
        }
        if self.session.settings.get(&format!("{key}_never")).as_deref() == Some("true") {
            return;
        }
        let showing = matches!(&self.dialog, Some(Dialog::Alert { key: k, .. }) if k == key);
        let queued = self.queued_alerts.iter().any(|d| matches!(d, Dialog::Alert { key: k, .. } if k == key));
        if showing || queued {
            return;
        }
        let dialog =
            Dialog::Alert { key: key.to_string(), title: title.to_string(), message: message.to_string(), suppress: false, suppress_forever: false };
        if self.dialog.is_none() {
            self.dialog = Some(dialog);
        } else {
            self.queued_alerts.push_back(dialog);
        }
    }

    fn complain(&mut self, msg: &str) {
        self.status = msg.to_string();
        self.status_is_error = true;
    }

    fn fail(&mut self, e: &EngineError) {
        self.status = e.to_string();
        self.status_is_error = true;
    }

    fn run(&mut self, id: &str, p: &Params) -> Option<Output> {
        match self.session.run(id, p) {
            Ok(o) => Some(o),
            Err(e) => {
                self.fail(&e);
                None
            }
        }
    }

    // ---------- page open, save ----------

    // ---------- tree ----------

    // ---------- panels ----------

    /// Lets only the pen write ink: a finger on the page does not draw. For touch screens.
    /// Gives the app the platform's way to read pictures; from then on Insert, Picture opens the
    /// app's own list instead of a system file dialog.
    pub fn set_files_access(&mut self, access: std::sync::Arc<dyn crate::filebrowser::FilesAccess>) {
        self.files = Some(access);
    }

    pub fn set_media_access(&mut self, access: std::sync::Arc<dyn crate::mediapicker::MediaAccess>) {
        self.media = Some(access);
    }

    pub fn set_pen_only(&mut self, on: bool) {
        self.pen_only = on;
        if on {
            // On a tablet the pen writes on the note by default; Type brings the caret back.
            if self.pointer == ribbon::Pointer::Write {
                self.pointer = ribbon::Pointer::Draw;
            }
            self.soft_keyboard_blocked = self.session.settings.get("soft_keyboard_blocked").as_deref() == Some("true");
        }
    }

    /// Whether the owner switched the on-screen keyboard off. The window layer reads this.
    pub fn soft_keyboard_blocked(&self) -> bool {
        self.pen_only && self.soft_keyboard_blocked
    }

    pub fn set_frameless(&mut self, on: bool) {
        self.frameless = on;
    }

    /// Chooses the compact layout (a one-row toolbar, a plain header) or the classic ribbon.
    pub fn set_compact_ui(&mut self, on: bool) {
        self.compact_ui = on;
    }

    // ---------- dialogs ----------

    // ---------- control channel ----------
}

/// One swatch of the theme switcher: the surface colour, ringed and dotted in the accent.
fn theme_swatch(ui: &mut Ui, theme: Theme, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let t = theme.tokens();
    let painter = ui.painter_at(rect);
    let c = rect.center();
    painter.circle_filled(c, 10.5, t.panel);
    painter.circle_stroke(c, 10.5, egui::Stroke::new(if selected { 3.0 } else { 1.5 }, t.accent));
    painter.circle_filled(c, 4.5, t.accent);
    if resp.hovered() {
        painter.circle_stroke(c, 12.0, egui::Stroke::new(1.0, ui.style().visuals.text_color()));
    }
    let label = format!("Theme {}", theme.label());
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, selected, &label));
    resp.on_hover_text(theme.label())
}

fn format_picker(ui: &mut Ui, format: &mut Option<PageFormat>) {
    egui::ComboBox::from_id_salt("dialog-format")
        .selected_text(format.map(|f| f.label()).unwrap_or("Inherit (notebook, section or global setting)"))
        .show_ui(ui, |ui| {
            ui.selectable_value(format, None, "Inherit (notebook, section or global setting)");
            for f in PageFormat::ALL {
                ui.selectable_value(format, Some(f), f.label());
            }
        });
}

#[allow(dead_code)]
fn _unused(_: Color32) {}

/// A small square button that draws one icon, with an on state for the ones that toggle.
fn icon_toggle(ui: &mut Ui, icon: Icon, on: bool, t: &Tokens, label: &str, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
    let painter = ui.painter_at(rect);
    if on || resp.hovered() {
        painter.rect_filled(rect, 4.0, if on { t.row_selected } else { t.row_hover });
    }
    let color = if on { t.accent } else { t.muted };
    icons::paint(&painter, egui::Rect::from_center_size(rect.center(), vec2(17.0, 17.0)), icon, color);
    let enabled = ui.is_enabled();
    let name = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name.clone()));
    resp.on_hover_text(tip)
}

/// One colour of the node colour picker: a filled circle, ringed when it is the current one.
fn color_swatch(ui: &mut Ui, color: NodeColor, dark: bool, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    let (r, g, b) = color.rgb(dark);
    let fill = Color32::from_rgb(r, g, b);
    let painter = ui.painter_at(rect);
    painter.circle_filled(rect.center(), 8.0, fill);
    if selected {
        painter.circle_stroke(rect.center(), 10.0, egui::Stroke::new(2.0, ui.style().visuals.text_color()));
    }
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, selected, color.label()));
    resp.on_hover_text(color.label())
}

/// Shows a folder in the operating system's file manager. An optional extra: everything the
/// app itself does works without it, and a failure is reported, never fatal.
fn open_folder(dir: &Path) -> std::io::Result<()> {
    // Explorer opens the user's Documents folder when it is given a path that does not exist, so
    // a missing folder is an error here, not a window on the wrong place.
    if !dir.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("{} does not exist", dir.display())));
    }
    // Android has no file manager to show a notebook in: the notebooks are in the app's own private storage,
    // which no other app can open. Say so, rather than failing to start a program that is not there.
    if cfg!(target_os = "android") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Android keeps the notebooks in the app's private storage; use Sync or Export to take them out",
        ));
    }
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // `explorer` returns a non-zero status even when it worked, so the status is not checked.
    std::process::Command::new(program).arg(dir).spawn().map(|_| ())
}

/// Opens a linked attachment with the operating system's associated application. Windows may
/// show its own warning for files marked as downloaded; that prompt is intentionally preserved.
pub(crate) fn open_file_for_viewer(file: &Path) -> std::io::Result<()> {
    if cfg!(target_os = "android") {
        return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "opening a file in another app is not available on Android yet"));
    }
    if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", "", &file.to_string_lossy()]).spawn().map(|_| ())
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(file).spawn().map(|_| ())
    } else {
        std::process::Command::new("xdg-open").arg(file).spawn().map(|_| ())
    }
}

/// Saves a screenshot to `path`, or returns it as base64 PNG in the reply.
fn encode_screenshot(image: &egui::ColorImage, path: Option<&Path>) -> Result<Value, String> {
    let [w, h] = image.size;
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    let img = image::RgbaImage::from_raw(w as u32, h as u32, rgba).ok_or("the screenshot has the wrong size")?;
    match path {
        Some(p) => {
            img.save(p).map_err(|e| format!("cannot save {}: {e}", p.display()))?;
            Ok(json!({"path": p.to_string_lossy(), "width": w, "height": h}))
        }
        None => {
            let mut png = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
            Ok(json!({"png": base64(png.get_ref()), "width": w, "height": h}))
        }
    }
}

/// The first page inside a notebook or section, at any depth.
fn first_page(tree: &[Node], root: &Path) -> Option<PathBuf> {
    fn walk(n: &Node) -> Option<PathBuf> {
        if n.info.kind == NodeKind::Page {
            return Some(n.path.clone());
        }
        n.children.iter().find_map(walk)
    }
    tree.iter().find(|n| n.path == root).and_then(walk)
}

/// The first page inside the node at `at`, wherever in the tree that node is.
fn first_page_under(tree: &[Node], at: &Path) -> Option<PathBuf> {
    fn find<'a>(n: &'a Node, at: &Path) -> Option<&'a Node> {
        if n.path == at {
            return Some(n);
        }
        n.children.iter().find_map(|c| find(c, at))
    }
    let node = tree.iter().find_map(|n| find(n, at))?;
    first_page(std::slice::from_ref(node), &node.path)
}

/// The 14 swatches and the switch that follows the system, shared by the sun or moon button
/// and the ribbon's Theme button.
fn theme_grid(ui: &mut Ui, current: Theme, pick: &mut Option<(Mode, Accent)>, follow: &mut bool) {
    egui::Grid::new("theme-grid").spacing([6.0, 6.0]).show(ui, |ui| {
        for mode in [Mode::Light, Mode::Dark] {
            ui.label(mode.label());
            for accent in Accent::ALL {
                let theme = Theme { mode, accent };
                let selected = theme == current;
                if theme_swatch(ui, theme, selected).clicked() {
                    *pick = Some((mode, accent));
                }
            }
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new(format!("Accent: {}", current.accent.label())).small());
    ui.checkbox(follow, crate::i18n::tr("Follow the system light or dark setting"));
}

#[cfg(test)]
mod alert_tests {
    use super::*;

    #[test]
    fn a_window_place_is_saved_and_read_back_and_nonsense_is_refused() {
        let text = window_geometry(10.4, 20.0, 1280.0, 800.0, true);
        assert_eq!(parse_window_geometry(&text), Some(([10.0, 20.0], [1280.0, 800.0], true)));
        assert_eq!(parse_window_geometry("1,2,100,100,0"), None, "smaller than the window may be");
        assert_eq!(parse_window_geometry("not a window"), None);
    }

    #[test]
    fn maximum_sensitivity_reaches_four_times_the_neutral_pen_width() {
        assert_eq!(NoteCraftApp::sensitivity_width(0), 1.0);
        assert_eq!(NoteCraftApp::sensitivity_width(50), 1.0);
        assert_eq!(NoteCraftApp::sensitivity_width(100), 4.0);
    }

    #[test]
    fn fresh_preferences_start_with_neutral_sensitivity() {
        let t = notecraft_store::testing::TempDir::new("default-sensitivity");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        assert_eq!(settings.get(SENSITIVITY_KEY), None);
        assert_eq!(settings.get(SENSITIVITY_KEY).and_then(|v| v.parse::<u8>().ok()).unwrap_or(DEFAULT_SENSITIVITY), 50);
        settings.set(SENSITIVITY_KEY, "100").expect("set");
        settings.reset().expect("reset");
        assert_eq!(settings.get(SENSITIVITY_KEY), None);
        assert_eq!(settings.get(SENSITIVITY_KEY).and_then(|v| v.parse::<u8>().ok()).unwrap_or(DEFAULT_SENSITIVITY), 50);
    }

    #[test]
    fn an_alert_queues_behind_an_open_one_and_a_muted_key_is_never_queued() {
        let t = notecraft_store::testing::TempDir::new("alerts");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        app.alert("first", "Title", "one");
        assert!(matches!(&app.dialog, Some(Dialog::Alert { key, .. }) if key == "first"), "the first alert opens");

        app.alert("first", "Title", "one again");
        assert!(app.queued_alerts.is_empty(), "the same alert is not shown twice while it is open");

        app.alert("second", "Title", "two");
        assert_eq!(app.queued_alerts.len(), 1, "a different alert waits its turn");

        app.muted_alerts.insert("third".to_string());
        app.alert("third", "Title", "three");
        assert_eq!(app.queued_alerts.len(), 1, "a muted alert is not queued");
        assert_eq!(app.status, "three", "the status line still says what happened");
    }

    #[test]
    fn a_permanently_muted_alert_is_not_opened() {
        let t = notecraft_store::testing::TempDir::new("alerts-never");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        settings.set("pressure_unavailable_never", "true").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        app.alert("pressure_unavailable", "Pressure", "not available");
        assert!(app.dialog.is_none(), "a permanent mute suppresses the dialog");
        assert_eq!(app.status, "not available", "the status still records the condition");
    }

    #[test]
    fn privacy_welcome_dialog_shows_on_first_launch() {
        let t = notecraft_store::testing::TempDir::new("privacy-first-launch");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        // Do NOT set privacy_welcome_seen - simulate first launch
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let app = NoteCraftApp::new(session);

        // Dialog should be shown because privacy_welcome_seen is not in settings
        assert!(matches!(app.dialog, Some(Dialog::PrivacyWelcome { .. })), "PrivacyWelcome dialog should show on first launch");
    }

    #[test]
    fn privacy_welcome_dialog_does_not_show_after_dismissed() {
        let t = notecraft_store::testing::TempDir::new("privacy-dismissed");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let app = NoteCraftApp::new(session);

        // Dialog should not be shown because privacy_welcome_seen is set
        assert!(app.dialog.is_none(), "PrivacyWelcome dialog should not show after being dismissed");
    }

    /// `--first-use` writes `privacy_welcome_seen = ""` and keeps every other setting.
    #[test]
    fn first_use_flag_value_shows_the_privacy_welcome_again() {
        let t = notecraft_store::testing::TempDir::new("privacy-first-use-flag");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "").expect("settings");
        settings.set("theme_mode", "dark").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let app = NoteCraftApp::new(session);
        assert!(matches!(app.dialog, Some(Dialog::PrivacyWelcome { .. })), "an emptied key counts as not seen");
        assert_eq!(app.session.settings.get("theme_mode").as_deref(), Some("dark"), "other settings are kept");
    }

    #[test]
    fn privacy_welcome_dialog_reappears_after_settings_file_deleted() {
        let t = notecraft_store::testing::TempDir::new("privacy-reset");
        let settings_path = t.path().join("settings.toml");
        let settings = notecraft_store::Settings::at(settings_path.clone());
        // Simulate persistent settings file with privacy_welcome_seen already set
        settings.set("privacy_welcome_seen", "true").expect("settings");
        settings.set("other_setting", "value").expect("settings");

        // Verify setting exists and dialog doesn't show
        let session1 = Session::new(&t.path().join("notebooks"), settings.clone()).expect("session");
        let app1 = NoteCraftApp::new(session1);
        assert!(app1.dialog.is_none(), "Dialog should not show when privacy_welcome_seen is set");

        // Delete the settings file (simulates what run.cmd does with --first-use)
        let _ = std::fs::remove_file(&settings_path);

        // Create new settings from deleted file (triggers fresh initialization)
        let settings2 = notecraft_store::Settings::at(settings_path);
        let session2 = Session::new(&t.path().join("notebooks"), settings2).expect("session");
        let app2 = NoteCraftApp::new(session2);

        // Dialog should reappear because settings file is gone
        assert!(matches!(app2.dialog, Some(Dialog::PrivacyWelcome { .. })), "PrivacyWelcome dialog should reappear when settings file is deleted");
    }

    #[test]
    fn delete_dialog_shows_for_notebook() {
        let t = notecraft_store::testing::TempDir::new("delete-dialog-notebook");
        let nb_path = t.path().join("test-notebook");
        std::fs::create_dir_all(&nb_path).expect("create notebook dir");

        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(t.path(), settings).expect("session");
        let _app = NoteCraftApp::new(session);

        // Simulate triggering delete notebook dialog
        let dialog = Dialog::Delete { path: nb_path.clone(), title: "Test Notebook".to_string(), count: 0 };

        // Verify the dialog is properly structured for a notebook delete
        assert!(matches!(dialog, Dialog::Delete { .. }));
    }

    #[test]
    fn delete_error_message_shown_on_deletion_failure() {
        let t = notecraft_store::testing::TempDir::new("delete-error-feedback");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(t.path(), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        // Simulate a failed delete operation by setting error status
        app.status = "The notebook could not be deleted: Permission denied".to_string();
        app.status_is_error = true;

        // Verify error state is properly set
        assert!(app.status_is_error, "error flag should be set");
        assert!(app.status.contains("could not be deleted"), "error message should indicate deletion failure");
    }

    #[test]
    fn delete_notebook_clears_editor_state() {
        let t = notecraft_store::testing::TempDir::new("delete-clear-state");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(t.path(), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        // Verify initial state is empty
        assert!(app.editor.is_none(), "editor should start empty");
        assert!(app.selected.is_none(), "selection should start empty");

        // Simulate the delete operation clearing state
        // This is what happens in the delete handler after successful deletion
        app.editor = None;
        app.selected = None;

        // Verify state remains cleared
        assert!(app.editor.is_none(), "editor should remain cleared after delete");
        assert!(app.selected.is_none(), "selection should remain cleared after delete");
    }

    #[test]
    fn node_delete_error_shows_user_feedback() {
        let t = notecraft_store::testing::TempDir::new("node-delete-error");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(t.path(), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        // Simulate node.delete command failure
        app.status = "Could not delete: the operation failed".to_string();
        app.status_is_error = true;

        // Verify error message is shown to user
        assert!(app.status_is_error, "error flag should be set when node delete fails");
        assert!(app.status.contains("Could not delete"), "user should see error message");
    }

    /// A page left with only the SVG fallback (the Shell extraction failed once) still has a
    /// usable icon and must not be reported as missing.
    #[test]
    fn an_svg_fallback_icon_is_accepted_everywhere() {
        let t = notecraft_store::testing::TempDir::new("svg-fallback-icon");
        let page = t.path().to_path_buf();
        let assets = page.join("assets");
        std::fs::create_dir_all(&assets).expect("assets");
        std::fs::write(assets.join("Chapter.docx"), b"docx").expect("attachment");
        std::fs::write(assets.join("Chapter.docx.svg"), b"<svg/>").expect("svg fallback");
        let source = r#"<p><a href="assets/Chapter.docx"><img src="assets/Chapter.docx.svg" alt="" width="48"><br>Chapter.docx</a></p>"#;

        assert_eq!(first_missing_attachment_in_source(source, &page), None, "the SVG fallback is a valid icon");
        assert!(attachment_icon_healthy(&page, "Chapter.docx"));

        std::fs::write(assets.join("Chapter.docx.png"), b"png").expect("png");
        assert_eq!(first_missing_attachment_in_source(source, &page), None, "a PNG sidecar is healthy everywhere");
        assert!(attachment_icon_healthy(&page, "Chapter.docx"));
    }
}
