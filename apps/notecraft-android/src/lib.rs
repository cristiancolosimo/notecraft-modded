#![cfg(target_os = "android")]
//! NoteCraft on Android: the desktop window over the same engine, started from a
//! NativeActivity. The notebooks and the settings live in the app's private storage.
//!
//! The pen arrives as `egui::Event::Touch { force }`, which `stylus.rs` already reads, so a
//! stylus writes with its pressure. Upstream winit gives every contact a force, so vendor/winit
//! sends one only for a stylus or eraser tool, and a finger never draws (`set_pen_only`).
//!
//! The window is laid out inside the system insets (status bar, navigation bar, keyboard), and
//! a header switch keeps the on-screen keyboard from opening (vendor/winit).
//!
//! Gaps with a stand-in: the file pickers and the clipboard (vendor/*-android-stub).

use android_activity::AndroidApp;
use notecraft_engine::Session;
use notecraft_store::Settings;
use notecraft_ui_egui::NoteCraftApp;
use std::path::{Path, PathBuf};

struct Shell {
    app: NoteCraftApp,
    android: AndroidApp,
}

/// What the status bar, the navigation bar and the on-screen keyboard cover, in pixels
/// (left, top, right, bottom), from the window's `WindowInsets` (Android 11 and later).
fn system_insets_px(app: &AndroidApp) -> Option<[i32; 4]> {
    use jni::JavaVM;
    use jni::objects::{JObject, JValue};
    // SAFETY: the pointers are the process's JavaVM and the running Activity, which
    // android-activity keeps valid for as long as `app` lives.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread().ok()?;
    let read = env.with_local_frame(16, |env| -> jni::errors::Result<[i32; 4]> {
        let window = env.call_method(&activity, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
        let decor = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
        let insets = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])?.l()?;
        if insets.is_null() {
            return Err(jni::errors::Error::NullPtr("no insets yet"));
        }
        // 7 is Type.systemBars() (status, navigation, caption), 8 is Type.ime().
        let both = env.call_method(&insets, "getInsets", "(I)Landroid/graphics/Insets;", &[JValue::Int(15)])?.l()?;
        let mut side = |name: &str| env.get_field(&both, name, "I")?.i();
        Ok([side("left")?, side("top")?, side("right")?, side("bottom")?])
    });
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    read.ok()
}

impl Shell {
    /// How much of the window the system covers, in points: the app is laid out inside what is
    /// left, so no button sits under a bar and the keyboard does not hide the page.
    fn insets(&self, ppp: f32) -> egui::Margin {
        let Some([left, top, right, bottom]) = system_insets_px(&self.android) else { return egui::Margin::ZERO };
        let points = |px: i32| (px.max(0) as f32 / ppp).round().min(400.0) as i8;
        egui::Margin { left: points(left), top: points(top), right: points(right), bottom: points(bottom) }
    }
}

impl eframe::App for Shell {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.events.extend(self.app.take_events());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        winit::platform::android::set_soft_keyboard_blocked(self.app.soft_keyboard_blocked());
        let margin = self.insets(ui.ctx().pixels_per_point());
        let rect = ui.max_rect() - margin;
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        self.app.show(&mut inner);
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
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Debug).with_tag("NoteCraft"));
    // Sync talks HTTPS with the system's trusted certificates.
    // SAFETY: called once at start-up, before libgit2 is used from any other thread.
    let _ = unsafe { git2::opts::set_ssl_cert_dir("/system/etc/security/cacerts") };
    // Exports go to the app's folder on shared storage, readable over USB and by adb
    // (there is no save dialog yet).
    rfd_android::set_save_dir(app.external_data_path().unwrap_or_else(|| data_dir(&app)).join("Exports"));
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
    let app_handle = app.clone();
    let options = eframe::NativeOptions { android_app: Some(app.clone()), ..Default::default() };
    let result = eframe::run_native(
        "NoteCraft",
        options,
        Box::new(move |_cc| {
            let mut app = NoteCraftApp::new(session).with_clipboard(clipboard);
            // The system draws the status bar; the desktop app's own title band has no place here.
            app.set_frameless(false);
            // A finger scrolls and taps; only the stylus draws, and it draws by default.
            app.set_pen_only(true);
            Ok(Box::new(Shell { app, android: app_handle }))
        }),
    );
    if let Err(e) = result {
        log::error!("the window failed: {e}");
    }
}
