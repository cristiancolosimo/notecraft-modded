//! The two languages of the window: English, which the code is written in, and Italian.
//!
//! A text is looked up by its English words. One that has no Italian stays English, so a string
//! nobody translated yet is still readable. The language is the `language` setting (`en` or
//! `it`); English is what a new installation uses, which keeps the tests and the screenshots stable.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::LazyLock;

thread_local! {
    // The window is drawn on one thread, so the language is that thread's; a test that changes it
    // changes nothing for the tests beside it.
    static ITALIAN: Cell<bool> = const { Cell::new(false) };
}

/// The languages that can be chosen: the code kept in the settings, and its name in itself.
pub const LANGUAGES: [(&str, &str); 2] = [("en", "English"), ("it", "Italiano")];

pub fn set_language(code: &str) {
    ITALIAN.with(|c| c.set(code == "it"));
}

pub fn language() -> &'static str {
    if ITALIAN.with(Cell::get) { "it" } else { "en" }
}

static DICTIONARY: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| include!("i18n_it.rs").into_iter().collect());

/// `text` in the chosen language: itself in English, or in Italian when it is known there.
pub fn tr(text: &str) -> &str {
    if ITALIAN.with(Cell::get) { DICTIONARY.get(text).copied().unwrap_or(text) } else { text }
}

/// Messages with a part that changes ("Opened {}"): the English template, with `{}` where the part is, and the Italian one.
static TEMPLATES: LazyLock<Vec<(Vec<&'static str>, &'static str)>> =
    LazyLock::new(|| include!("i18n_dyn.rs").into_iter().map(|(en, it): (&'static str, &'static str)| (en.split("{}").collect(), it)).collect());

/// What a template's `{}` stood for in `text`, when `text` is that template filled in.
fn parts<'a>(pieces: &[&str], text: &'a str) -> Option<Vec<&'a str>> {
    let (first, rest) = pieces.split_first()?;
    let mut hay = text.strip_prefix(first)?;
    let mut found = Vec::new();
    for (i, piece) in rest.iter().enumerate() {
        let stop = if i + 1 == rest.len() { hay.ends_with(piece).then(|| hay.len() - piece.len())? } else { hay.find(piece)? };
        found.push(&hay[..stop]);
        hay = &hay[stop + piece.len()..];
    }
    if rest.is_empty() { hay.is_empty().then_some(found) } else { Some(found) }
}

/// Like [`tr`] for a message that has been filled in: "Opened My Notebook" says "Aperto My Notebook".
pub fn tr_dyn(text: &str) -> std::borrow::Cow<'_, str> {
    if !ITALIAN.with(Cell::get) {
        return std::borrow::Cow::Borrowed(text);
    }
    if let Some(known) = DICTIONARY.get(text) {
        return std::borrow::Cow::Borrowed(known);
    }
    for (pieces, it) in TEMPLATES.iter() {
        if let Some(found) = parts(pieces, text) {
            let mut out = String::new();
            let mut args = found.into_iter();
            for (i, chunk) in it.split("{}").enumerate() {
                if i > 0 {
                    out.push_str(tr(args.next().unwrap_or("")));
                }
                out.push_str(chunk);
            }
            return std::borrow::Cow::Owned(out);
        }
    }
    std::borrow::Cow::Borrowed(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_text_changes_with_the_language_and_an_unknown_one_does_not() {
        set_language("it");
        assert_eq!(tr("New page"), "Nuova pagina");
        assert_eq!(tr("A text nobody translated"), "A text nobody translated");
        set_language("en");
        assert_eq!(tr("New page"), "New page");
    }

    #[test]
    fn a_message_with_a_changing_part_is_translated_around_it() {
        set_language("it");
        assert_eq!(tr_dyn("Opened My Notebook"), "Aperto My Notebook");
        assert_eq!(
            tr_dyn("Imported lecture.pdf: 2 pages. The PDF itself is not changed"),
            "Importato lecture.pdf: 2 pagine. Il PDF non viene modificato"
        );
        assert_eq!(tr_dyn("Something nobody wrote a template for"), "Something nobody wrote a template for");
        set_language("en");
        assert_eq!(tr_dyn("Opened My Notebook"), "Opened My Notebook");
    }

    #[test]
    fn every_italian_text_differs_from_its_english_and_none_is_listed_twice() {
        let entries: Vec<(&str, &str)> = include!("i18n_it.rs").into_iter().collect();
        let mut seen = std::collections::HashSet::new();
        for (en, it) in &entries {
            assert!(!it.is_empty() && en != it, "{en:?} has a translation of its own");
            assert!(seen.insert(*en), "{en:?} is listed twice");
        }
    }
}
