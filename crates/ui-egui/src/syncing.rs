//! Sync as the window sees it: the form on the File screen, the settings it is kept in, and the
//! timer that syncs by itself while the app is open and the server can be reached.
//!
//! The engine is `notecraft-sync`. Here only the address, the username, the interval and the on or
//! off switch go in the settings file; the token goes in a file of its own beside it, readable by
//! the owner alone where the system can say so (as the control channel's key does).

use notecraft_store::Settings;
use notecraft_sync::{Config, Report};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What the form edits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Form {
    pub url: String,
    pub username: String,
    pub token: String,
    /// Minutes between automatic syncs, as typed.
    pub minutes: String,
    pub enabled: bool,
}

/// The default time between automatic syncs.
pub const DEFAULT_MINUTES: u64 = 15;

/// What a running sync is doing, written by its thread and read by the window.
#[derive(Default)]
pub struct Live {
    /// The step it is on, as one line.
    pub step: String,
    /// How far along that step is, when it is known.
    pub fraction: Option<f32>,
    /// The steps so far, newest last; a step that only moves on (a count going up) replaces its line.
    pub log: Vec<String>,
    pub started: Option<Instant>,
}

/// The lines of a sync's log that are kept.
const LOG_LINES: usize = 60;

impl Live {
    fn say(&mut self, text: &str, fraction: Option<f32>) {
        self.step = text.to_string();
        self.fraction = fraction;
        // "Receiving objects 12/90 (1 KB)" and "Receiving objects 13/90 (2 KB)" are one line, moving.
        let head = |t: &str| t.split(|c: char| c.is_ascii_digit()).next().unwrap_or("").to_string();
        match self.log.last_mut() {
            Some(last) if fraction.is_some() && head(last) == head(text) => *last = text.to_string(),
            _ => self.log.push(text.to_string()),
        }
        if self.log.len() > LOG_LINES {
            self.log.remove(0);
        }
    }
}

pub struct Syncing {
    /// The running sync's progress, and the log of the last one after it ends.
    pub live: Arc<Mutex<Live>>,
    pub form: Form,
    /// What the last sync did, or why it did not, for the File screen and the status bar.
    pub status: String,
    pub failed: bool,
    next_due: Instant,
    running: Option<Receiver<Result<Report, String>>>,
}

/// A finished sync, handed to the window to refresh the notebooks.
pub struct Finished {
    pub report: Report,
}

fn token_file(settings: &Settings) -> PathBuf {
    settings.path().parent().unwrap_or(Path::new(".")).join("sync-token.local")
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?.write_all(text.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}

impl Syncing {
    pub fn load(settings: &Settings) -> Syncing {
        let form = Form {
            url: settings.get("sync_url").unwrap_or_default(),
            username: settings.get("sync_user").unwrap_or_default(),
            token: std::fs::read_to_string(token_file(settings)).map(|t| t.trim().to_string()).unwrap_or_default(),
            minutes: settings.get("sync_minutes").unwrap_or_else(|| DEFAULT_MINUTES.to_string()),
            enabled: settings.get("sync_enabled").as_deref() == Some("true"),
        };
        // The first automatic sync waits a little, so the window opens first.
        Syncing {
            live: Arc::default(),
            form,
            status: String::new(),
            failed: false,
            next_due: Instant::now() + Duration::from_secs(20),
            running: None,
        }
    }

    /// Writes the form to the settings and the token file.
    pub fn save(&mut self, settings: &Settings) -> Result<(), String> {
        let minutes = self.minutes();
        self.form.minutes = minutes.to_string();
        let url = self.form.url.trim().to_string();
        if !notecraft_sync::address_allowed(&url) {
            return Err("the address must start with https:// (a token is never sent over plain http)".into());
        }
        for (key, value) in [
            ("sync_url", url.as_str()),
            ("sync_user", self.form.username.trim()),
            ("sync_minutes", self.form.minutes.as_str()),
            ("sync_enabled", if self.form.enabled { "true" } else { "false" }),
        ] {
            settings.set(key, value).map_err(|e| format!("cannot save the settings: {e}"))?;
        }
        write_private(&token_file(settings), self.form.token.trim()).map_err(|e| format!("cannot save the token: {e}"))?;
        self.next_due = Instant::now();
        Ok(())
    }

    pub fn minutes(&self) -> u64 {
        self.form.minutes.trim().parse::<u64>().ok().filter(|m| *m >= 1).unwrap_or(DEFAULT_MINUTES).min(24 * 60)
    }

    pub fn config(&self) -> Config {
        Config::new(&self.form.url, &self.form.username, &self.form.token)
    }

    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// Starts a sync of `root` in the background.
    pub fn start(&mut self, root: &Path, ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        let config = self.config();
        if !config.is_complete() {
            self.status = "Sync is not set up: it needs an address, a username and a token.".into();
            self.failed = true;
            return;
        }
        self.status = "Syncing...".into();
        self.failed = false;
        if let Ok(mut live) = self.live.lock() {
            *live = Live { started: Some(Instant::now()), ..Live::default() };
            live.say("Starting", None);
        }
        let live = self.live.clone();
        let (tx, rx) = mpsc::channel();
        self.running = Some(rx);
        let (root, ctx) = (root.to_path_buf(), ctx.clone());
        std::thread::spawn(move || {
            let say = |text: &str, fraction: Option<f32>| {
                if let Ok(mut live) = live.lock() {
                    live.say(text, fraction);
                }
                ctx.request_repaint();
            };
            say("Checking that the server can be reached", None);
            let result = if notecraft_sync::reachable(&config.url) {
                notecraft_sync::sync_with(&root, &config, &say)
            } else {
                Err("the server cannot be reached (no internet?)".to_string())
            };
            if let Err(e) = &result {
                say(&format!("Failed: {e}"), None);
            }
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    /// Called every frame: collects a finished sync, and starts the next one when it is due.
    pub fn poll(&mut self, root: &Path, ctx: &egui::Context) -> Option<Finished> {
        let mut finished = None;
        if let Some(rx) = &self.running
            && let Ok(result) = rx.try_recv()
        {
            self.running = None;
            self.next_due = Instant::now() + Duration::from_secs(self.minutes() * 60);
            let at = notecraft_model::time::iso_now();
            match result {
                Ok(report) => {
                    self.status = format!("Synced {at}: {}", report.summary());
                    self.failed = false;
                    finished = Some(Finished { report });
                }
                Err(e) => {
                    self.status = format!("Sync failed {at}: {e}");
                    self.failed = true;
                }
            }
        }
        if self.busy()
            && let Ok(live) = self.live.lock()
        {
            // The status bar shows the step too; the File screen shows the whole log.
            self.status = format!("Syncing: {}", live.step);
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        if self.form.enabled && !self.busy() {
            let now = Instant::now();
            if now >= self.next_due {
                self.start(root, ctx);
                // A failed start (no setup) must not be retried every frame.
                if !self.busy() {
                    self.next_due = now + Duration::from_secs(self.minutes() * 60);
                }
            } else {
                ctx.request_repaint_after(self.next_due - now);
            }
        }
        finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(label: &str) -> (PathBuf, Settings) {
        let dir = std::env::temp_dir().join(format!("notecraft-syncing-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        (dir.clone(), Settings::at(dir.join("settings.toml")))
    }

    #[test]
    fn the_form_is_kept_and_the_token_lives_in_its_own_file() {
        let (dir, settings) = settings("keep");
        let mut sync = Syncing::load(&settings);
        sync.form = Form {
            url: "https://github.com/me/notes.git".into(),
            username: "me".into(),
            token: "ghp_secret".into(),
            minutes: "30".into(),
            enabled: true,
        };
        sync.save(&settings).expect("save");

        let text = std::fs::read_to_string(settings.path()).expect("settings file");
        assert!(!text.contains("ghp_secret"), "the token is not in the settings file: {text}");
        let again = Syncing::load(&settings);
        assert_eq!(again.form, sync.form);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_address_that_is_not_https_is_refused_and_the_interval_is_sane() {
        let (dir, settings) = settings("refuse");
        let mut sync = Syncing::load(&settings);
        sync.form.url = "http://example.test/notes.git".into();
        assert!(sync.save(&settings).is_err(), "a token must not cross plain http");
        sync.form.url = "https://example.test/notes.git".into();
        sync.form.minutes = "0".into();
        assert_eq!(sync.minutes(), DEFAULT_MINUTES);
        sync.form.minutes = "999999".into();
        assert_eq!(sync.minutes(), 24 * 60);
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[test]
    fn a_count_going_up_is_one_line_and_a_new_step_is_a_new_one() {
        let mut live = Live::default();
        live.say("Fetching", None);
        live.say("Receiving objects 1/90 (1 KB)", Some(0.01));
        live.say("Receiving objects 2/90 (2 KB)", Some(0.02));
        live.say("Merging", None);
        assert_eq!(live.log, vec!["Fetching", "Receiving objects 2/90 (2 KB)", "Merging"]);
        assert_eq!(live.step, "Merging");
        for i in 0..200 {
            live.say(&format!("step {i}x"), None);
        }
        assert_eq!(live.log.len(), LOG_LINES);
    }
}
