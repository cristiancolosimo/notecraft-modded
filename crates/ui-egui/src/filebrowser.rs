//! A plain file chooser drawn by the app itself, for a platform with no file dialog to call (Android):
//! folders to go into, files to pick, a folder to use. What the platform decides, access to the files,
//! is behind [`FilesAccess`]; the window gives it to the app.

use egui::{Align2, Context, Key, RichText, ScrollArea, vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What a platform has to say about reading the device's files.
pub trait FilesAccess: Send + Sync {
    /// Whether the app may read the files in shared storage now.
    fn granted(&self) -> bool;
    /// Takes the user to where that access is given. The answer shows up in [`FilesAccess::granted`].
    fn request(&self);
    /// Where browsing begins, and the shortcuts under it (`Download`, `Documents`, ...).
    fn root(&self) -> PathBuf;
}

/// One line of a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub folder: bool,
}

/// What `dir` holds: folders first, then files, each in name order. Hidden names are left out. When only
/// folders are wanted the files are not listed; otherwise `extensions` (lower case, no dot; empty means all)
/// says which files are.
pub fn list(dir: &Path, folders_only: bool, extensions: &[String]) -> std::io::Result<Vec<Row>> {
    let mut rows = Vec::new();
    for entry in std::fs::read_dir(dir)?.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        // A link is followed: a folder in shared storage is often one.
        let Ok(meta) = std::fs::metadata(entry.path()) else { continue };
        if meta.is_dir() {
            rows.push(Row { name, folder: true });
        } else if !folders_only {
            let ext = Path::new(&name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if extensions.is_empty() || extensions.contains(&ext) {
                rows.push(Row { name, folder: false });
            }
        }
    }
    rows.sort_by(|a, b| b.folder.cmp(&a.folder).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(rows)
}

/// What the chooser's window answered this frame.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Cancel,
    Chosen(PathBuf),
}

pub struct Browser {
    title: String,
    folders_only: bool,
    extensions: Vec<String>,
    dir: PathBuf,
    rows: Option<Result<Vec<Row>, String>>,
    asked: bool,
}

impl Browser {
    /// A chooser for a folder (`folders_only`) or for a file whose extension is one of `extensions` (any, if empty).
    pub fn new(title: &str, start: PathBuf, folders_only: bool, extensions: &[&str]) -> Browser {
        Browser {
            title: title.to_string(),
            folders_only,
            extensions: extensions.iter().map(|e| e.to_lowercase()).collect(),
            dir: start,
            rows: None,
            asked: false,
        }
    }

    fn go(&mut self, dir: PathBuf) {
        self.dir = dir;
        self.rows = None;
    }

    pub fn show(&mut self, ctx: &Context, access: &Arc<dyn FilesAccess>, tokens: &crate::theme::Tokens, lift: f32) -> Outcome {
        let mut outcome = Outcome::Open;
        let root = access.root();
        let granted = access.granted();
        if self.rows.is_none() {
            self.rows = Some(list(&self.dir, self.folders_only, &self.extensions).map_err(|e| e.to_string()));
        }
        let mut goto: Option<PathBuf> = None;
        let room = (ctx.content_rect().height() * 0.5).clamp(160.0, 520.0);
        egui::Window::new("file-browser")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, lift))
            .frame(crate::app::modal_frame(tokens))
            .show(ctx, |ui| {
                ui.set_min_width(520.0);
                ui.label(RichText::new(&self.title).strong().size(18.0).color(tokens.text));
                ui.add_space(4.0);
                ui.label(RichText::new(self.dir.display().to_string()).small().monospace().color(tokens.muted));
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(self.dir != root && self.dir.parent().is_some(), egui::Button::new(RichText::new("Up").size(16.0))).clicked()
                        && let Some(parent) = self.dir.parent()
                    {
                        goto = Some(parent.to_path_buf());
                    }
                    for (label, path) in [
                        ("Storage", root.clone()),
                        ("Download", root.join("Download")),
                        ("Documents", root.join("Documents")),
                        ("Pictures", root.join("Pictures")),
                    ] {
                        if path.is_dir() && ui.button(RichText::new(label).size(16.0)).clicked() {
                            goto = Some(path);
                        }
                    }
                });
                ui.add_space(6.0);
                if !granted {
                    ui.label(RichText::new("Other apps' files stay hidden until you allow NoteCraft to read them.").color(tokens.muted));
                    if ui.button(RichText::new("Allow access to files").size(16.0)).clicked() {
                        access.request();
                        self.asked = true;
                    }
                    if self.asked {
                        ui.label(RichText::new("Come back here after you allow it, then press Refresh.").small().color(tokens.muted));
                        if ui.button("Refresh").clicked() {
                            self.rows = None;
                        }
                    }
                    ui.add_space(6.0);
                }
                ScrollArea::vertical().id_salt("file-browser-list").max_height(room).show(ui, |ui| {
                    ui.set_width(520.0);
                    match &self.rows {
                        Some(Ok(rows)) if rows.is_empty() => {
                            ui.label(
                                RichText::new(if self.folders_only { "No folders here." } else { "Nothing to choose here." }).color(tokens.muted),
                            );
                        }
                        Some(Ok(rows)) => {
                            for row in rows {
                                let text = if row.folder { format!("{}/", row.name) } else { row.name.clone() };
                                let label = RichText::new(text).size(17.0).color(if row.folder { tokens.accent } else { tokens.text });
                                // A row is tall enough for a finger.
                                let clicked = ui.add_sized(vec2(ui.available_width(), 38.0), egui::Button::new(label).frame(false)).clicked();
                                if clicked && row.folder {
                                    goto = Some(self.dir.join(&row.name));
                                } else if clicked {
                                    outcome = Outcome::Chosen(self.dir.join(&row.name));
                                }
                            }
                        }
                        Some(Err(e)) => {
                            ui.label(RichText::new(format!("This folder cannot be read: {e}")).color(tokens.warn));
                        }
                        None => {}
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if self.folders_only && ui.button(RichText::new("Use this folder").size(16.0)).clicked() {
                        outcome = Outcome::Chosen(self.dir.clone());
                    }
                    if ui.button(RichText::new("Cancel").size(16.0)).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                        outcome = Outcome::Cancel;
                    }
                });
            });
        if let Some(dir) = goto {
            self.go(dir);
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn folders_come_first_hidden_names_are_skipped_and_files_can_be_filtered() {
        let root = std::env::temp_dir().join(format!("notecraft-browser-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("b-folder")).expect("dir");
        fs::create_dir_all(root.join(".hidden")).expect("hidden");
        fs::write(root.join("a.PDF"), b"x").expect("write");
        fs::write(root.join("c.txt"), b"x").expect("write");

        let all = list(&root, false, &[]).expect("list");
        let names: Vec<(&str, bool)> = all.iter().map(|r| (r.name.as_str(), r.folder)).collect();
        assert_eq!(names, vec![("b-folder", true), ("a.PDF", false), ("c.txt", false)]);

        let pdfs = list(&root, false, &["pdf".to_string()]).expect("list");
        assert_eq!(pdfs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["b-folder", "a.PDF"], "folders stay, to go into");

        let folders = list(&root, true, &[]).expect("list");
        assert_eq!(folders, vec![Row { name: "b-folder".into(), folder: true }]);
        assert!(list(&root.join("missing"), false, &[]).is_err(), "a folder that is not there is an error to show");
        let _ = fs::remove_dir_all(&root);
    }
}
