//! The part of `rfd` that NoteCraft uses, for Android, where rfd has no backend.
//!
//! The desktop dialogs have no Android equivalent here yet (it needs the Storage Access
//! Framework through JNI), so every picker reports that it chose nothing, which the callers
//! already treat as "cancelled". The gap is logged and listed in docs/gaps.md.

use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct FileDialog;

impl FileDialog {
    pub fn new() -> Self {
        Self
    }
    pub fn set_title(self, _title: impl Into<String>) -> Self {
        self
    }
    pub fn set_file_name(self, _name: impl Into<String>) -> Self {
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
        unsupported()
    }
}

fn unsupported() -> Option<PathBuf> {
    eprintln!("NoteCraft: the file picker is not available on Android yet");
    None
}
