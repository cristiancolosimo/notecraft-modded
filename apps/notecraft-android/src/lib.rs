#![cfg(target_os = "android")]
//! NoteCraft on Android: the desktop window over the same engine, started from a
//! NativeActivity. The notebooks and the settings live in the app's private storage.
//!
//! The pen arrives as `egui::Event::Touch { force }`, which `stylus.rs` already reads, so a
//! stylus writes with its pressure and a finger scrolls and taps. winit reports a force for
//! every contact, so a finger is read as a light pen; telling the tool types apart needs
//! the MotionEvent tool type, which winit does not forward (listed in docs/gaps.md).
//!
//! Gaps with a stand-in: the file pickers and the clipboard (vendor/*-android-stub).

use android_activity::AndroidApp;
use notecraft_engine::Session;
use notecraft_store::Settings;
use notecraft_ui_egui::NoteCraftApp;
use std::path::{Path, PathBuf};

struct Shell(NoteCraftApp);

impl eframe::App for Shell {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.events.extend(self.0.take_events());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.show(ui);
    }
}

/// Where the app keeps its data: its private folder, which needs no storage permission.
fn data_dir(app: &AndroidApp) -> PathBuf {
    app.internal_data_path().unwrap_or_else(|| PathBuf::from("/data/local/tmp/notecraft"))
}

/// The notebooks folder, with the Welcome notebook installed the first time.
fn open_session(data: &Path) -> Result<Session, String> {
    let root = data.join("NoteCraft");
    let mut session = Session::new(&root, Settings::at(data.join("settings.toml"))).map_err(|e| e.to_string())?;
    if session.tree().is_empty() {
        session.run("notebook.welcome", &Default::default()).map_err(|e| e.to_string())?;
    }
    Ok(session)
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("NoteCraft"));
    let session = match open_session(&data_dir(&app)) {
        Ok(session) => session,
        Err(e) => {
            log::error!("cannot open the notebooks folder: {e}");
            return;
        }
    };
    let clipboard: Box<dyn notecraft_ui_egui::clipboard::Board> = match notecraft_ui_egui::clipboard::open() {
        Ok(board) => board,
        Err(_) => Box::new(notecraft_ui_egui::clipboard::Unavailable),
    };
    let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
    let result =
        eframe::run_native("NoteCraft", options, Box::new(move |_cc| Ok(Box::new(Shell(NoteCraftApp::new(session).with_clipboard(clipboard))))));
    if let Err(e) = result {
        log::error!("the window failed: {e}");
    }
}
