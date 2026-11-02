//! Choosing a picture (or another file) where the system has no file dialog to offer, as on
//! Android: the app lists what it can read in the usual photo folders, newest first, with a
//! preview of the one chosen.
//!
//! What the platform decides, access, is behind [`MediaAccess`]; the window gives it to the app.

use egui::{Align2, Context, Key, RichText, ScrollArea, vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// What a platform has to say about reading the user's photos.
pub trait MediaAccess: Send + Sync {
    /// Whether the app may read pictures in shared storage now.
    fn granted(&self) -> bool;
    /// Asks the user for that access. The answer shows up in [`MediaAccess::granted`].
    fn request(&self);
    /// The folders worth listing, most likely first.
    fn roots(&self) -> Vec<PathBuf>;
}

/// One file on offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub folder: String,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
}

const PICTURES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];
/// How deep under a root, and how many files, are looked at: a phone can hold a hundred thousand.
const DEPTH: usize = 3;
const LIMIT: usize = 600;

/// The pictures under `roots`, newest first. Hidden folders (`.thumbnails`, `.trash`) are skipped.
pub fn scan(roots: &[PathBuf], pictures_only: bool) -> Vec<Entry> {
    let mut found = Vec::new();
    for root in roots {
        walk(root, root, 0, pictures_only, &mut found);
        if found.len() >= LIMIT * 4 {
            break;
        }
    }
    found.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    found.truncate(LIMIT);
    found
}

fn walk(root: &Path, dir: &Path, depth: usize, pictures_only: bool, out: &mut Vec<Entry>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < DEPTH {
                walk(root, &entry.path(), depth + 1, pictures_only, out);
            }
        } else if kind.is_file() {
            let ext = Path::new(&name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if pictures_only && !PICTURES.contains(&ext.as_str()) {
                continue;
            }
            let meta = entry.metadata().ok();
            let folder = entry
                .path()
                .parent()
                .and_then(|p| p.strip_prefix(root.parent().unwrap_or(root)).ok())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push(Entry {
                path: entry.path(),
                name,
                folder,
                bytes: meta.as_ref().map_or(0, std::fs::Metadata::len),
                modified: meta.and_then(|m| m.modified().ok()),
            });
        }
    }
}

/// What the picker's window answered this frame.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Cancel,
    Chosen(PathBuf),
}

pub struct Picker {
    pictures_only: bool,
    entries: Option<Vec<Entry>>,
    selected: Option<usize>,
    asked: bool,
}

impl Picker {
    pub fn new(pictures_only: bool) -> Picker {
        Picker { pictures_only, entries: None, selected: None, asked: false }
    }

    pub fn show(&mut self, ctx: &Context, access: &Arc<dyn MediaAccess>, tokens: &crate::theme::Tokens) -> Outcome {
        let mut outcome = Outcome::Open;
        let granted = access.granted();
        if granted && self.entries.is_none() {
            self.entries = Some(scan(&access.roots(), self.pictures_only));
        }
        egui::Window::new("media-picker")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .frame(crate::app::modal_frame(tokens))
            .show(ctx, |ui| {
                ui.set_min_width(560.0);
                ui.label(RichText::new(if self.pictures_only { "Choose a picture" } else { "Choose a file" }).strong().size(18.0).color(tokens.text));
                ui.add_space(6.0);
                if !granted {
                    ui.label(
                        RichText::new("NoteCraft needs your permission to see the pictures on this device. It only reads the one you choose.")
                            .color(tokens.muted),
                    );
                    ui.add_space(8.0);
                    if ui.button(RichText::new("Allow access to pictures").size(16.0)).clicked() {
                        access.request();
                        self.asked = true;
                    }
                    if self.asked {
                        ui.label(RichText::new("Waiting for your answer...").small().color(tokens.muted));
                        ctx.request_repaint_after(std::time::Duration::from_millis(400));
                    }
                } else if let Some(entries) = &self.entries {
                    if entries.is_empty() {
                        ui.label(RichText::new("No pictures were found in Camera, Pictures, Screenshots or Downloads.").color(tokens.muted));
                    } else {
                        ui.horizontal_top(|ui| {
                            ScrollArea::vertical().id_salt("media-list").max_height(360.0).min_scrolled_width(300.0).show(ui, |ui| {
                                ui.set_width(300.0);
                                for (i, e) in entries.iter().enumerate() {
                                    let line = format!("{}\n{} · {}", e.name, e.folder, size_text(e.bytes));
                                    if ui.selectable_label(self.selected == Some(i), line).clicked() {
                                        self.selected = Some(i);
                                    }
                                }
                            });
                            ui.add_space(8.0);
                            ui.allocate_ui(vec2(230.0, 340.0), |ui| match self.selected.and_then(|i| entries.get(i)) {
                                Some(e) => {
                                    let uri = format!("file://{}", e.path.display());
                                    ui.add(egui::Image::new(uri).max_size(vec2(230.0, 340.0)).corner_radius(4.0));
                                }
                                None => {
                                    ui.label(RichText::new("Pick a picture to see it here.").color(tokens.muted));
                                }
                            });
                        });
                    }
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let chosen = self.selected.and_then(|i| self.entries.as_ref().and_then(|e| e.get(i)));
                    if ui.add_enabled(chosen.is_some(), egui::Button::new(RichText::new("Use this picture").size(16.0))).clicked()
                        && let Some(e) = chosen
                    {
                        outcome = Outcome::Chosen(e.path.clone());
                    }
                    if ui.button(RichText::new("Cancel").size(16.0)).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                        outcome = Outcome::Cancel;
                    }
                });
            });
        outcome
    }
}

fn size_text(bytes: u64) -> String {
    if bytes >= 1_000_000 { format!("{:.1} MB", bytes as f64 / 1e6) } else { format!("{} KB", (bytes / 1000).max(1)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn pictures_are_found_newest_first_and_hidden_places_are_skipped() {
        let root = std::env::temp_dir().join(format!("notecraft-media-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dcim = root.join("DCIM").join("Camera");
        fs::create_dir_all(&dcim).expect("dirs");
        fs::create_dir_all(root.join("DCIM").join(".thumbnails")).expect("hidden");
        fs::write(dcim.join("old.jpg"), b"x").expect("write");
        std::thread::sleep(std::time::Duration::from_millis(30));
        fs::write(dcim.join("new.PNG"), b"x").expect("write");
        fs::write(dcim.join("notes.txt"), b"x").expect("write");
        fs::write(root.join("DCIM").join(".thumbnails").join("t.jpg"), b"x").expect("write");

        let got = scan(&[root.join("DCIM")], true);
        let names: Vec<&str> = got.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["new.PNG", "old.jpg"], "newest first, pictures only, nothing from the hidden folder");
        assert_eq!(scan(&[root.join("DCIM")], false).len(), 3, "any file when pictures are not asked for");
        assert!(scan(&[root.join("missing")], true).is_empty(), "a folder that is not there is not an error");
        let _ = fs::remove_dir_all(&root);
    }
}
