//! The part of `arboard` that NoteCraft uses, for Android, where arboard has no backend.
//!
//! `Clipboard::new` fails, so the app starts with its "clipboard unavailable" notice and
//! egui's own platform clipboard keeps handling ordinary text copy and paste. Pictures and
//! HTML on the clipboard are a gap listed in docs/gaps.md.

use std::borrow::Cow;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    ClipboardNotSupported,
    ClipboardOccupied,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the clipboard is not available on Android yet")
    }
}

pub struct ImageData<'a> {
    pub width: usize,
    pub height: usize,
    pub bytes: Cow<'a, [u8]>,
}

pub struct Clipboard;

pub struct Get<'a>(&'a mut Clipboard);
pub struct Set<'a>(&'a mut Clipboard);

impl Clipboard {
    pub fn new() -> Result<Self, Error> {
        Err(Error::ClipboardNotSupported)
    }
    pub fn get_text(&mut self) -> Result<String, Error> {
        Err(Error::ClipboardNotSupported)
    }
    pub fn set_text(&mut self, _text: String) -> Result<(), Error> {
        Err(Error::ClipboardNotSupported)
    }
    pub fn get_image(&mut self) -> Result<ImageData<'static>, Error> {
        Err(Error::ClipboardNotSupported)
    }
    pub fn get(&mut self) -> Get<'_> {
        Get(self)
    }
    pub fn set(&mut self) -> Set<'_> {
        Set(self)
    }
}

impl Get<'_> {
    pub fn html(self) -> Result<String, Error> {
        let _ = self.0;
        Err(Error::ClipboardNotSupported)
    }
}

impl Set<'_> {
    pub fn html(self, _html: String, _alt_text: Option<String>) -> Result<(), Error> {
        let _ = self.0;
        Err(Error::ClipboardNotSupported)
    }
}
