//! The part of `rfd` that NoteCraft uses, for Android, where rfd has no backend.
//!
//! The desktop dialogs have no Android equivalent here yet (it needs the Storage Access
//! Framework through JNI), so every picker reports that it chose nothing, which the callers
//! already treat as "cancelled". The one exception is saving: with a folder set by
//! `set_save_dir`, `save_file` answers with that folder and the suggested name, so an export
//! lands in the app's own storage without a dialog. The gaps are listed in docs/gaps.md.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static SAVE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Where `save_file` puts what the app saves (the app calls this once, at start).
pub fn set_save_dir(dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    let _ = SAVE_DIR.set(dir);
}

#[derive(Default)]
pub struct FileDialog {
    name: Option<String>,
}

impl FileDialog {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_title(self, _title: impl Into<String>) -> Self {
        self
    }
    pub fn set_file_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn set_directory(self, _dir: impl AsRef<Path>) -> Self {
        self
    }
    pub fn add_filter(self, _name: impl Into<String>, _extensions: &[impl ToString]) -> Self {
        self
    }
    pub fn pick_file(self) -> Option<PathBuf> {
        unsupported()
    }
    pub fn pick_folder(self) -> Option<PathBuf> {
        unsupported()
    }
    pub fn save_file(self) -> Option<PathBuf> {
        match (SAVE_DIR.get(), self.name) {
            (Some(dir), Some(name)) => Some(dir.join(name)),
            _ => unsupported(),
        }
    }
}

static NOTICE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// What the owner should be told about a picker that chose nothing, once: the app shows it in its status bar.
pub fn take_notice() -> Option<String> {
    NOTICE.lock().ok().and_then(|mut n| n.take())
}

fn unsupported() -> Option<PathBuf> {
    eprintln!("NoteCraft: the file picker is not available on Android yet");
    if let Ok(mut notice) = NOTICE.lock() {
        *notice = Some("There is no file chooser on Android yet; pictures come from Insert, Picture, and notes travel by Sync".into());
    }
    None
}
