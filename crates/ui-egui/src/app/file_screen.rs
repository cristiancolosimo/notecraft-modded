//! The File screen, the notebooks that are open, and how files and links are opened.

use super::*;

impl NoteCraftApp {
    /// Asks for a file or a folder. With the system's dialog the answer is here and now; with the app's own
    /// chooser (no system dialog) it opens and the answer comes later, to [`App::finish_pick`], so this gives `None`.
    pub(super) fn pick(&mut self, purpose: Purpose, title: &str, folder: bool, extensions: &[&str], start: Option<PathBuf>) -> Option<PathBuf> {
        if let Some(access) = self.files.clone() {
            let start = start.filter(|p| p.is_dir()).unwrap_or_else(|| access.root());
            self.dialog = Some(Dialog::Browse { purpose, browser: crate::filebrowser::Browser::new(title, start, folder, extensions) });
            return None;
        }
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if !extensions.is_empty() {
            dialog = dialog.add_filter("Files", extensions);
        }
        if let Some(start) = start {
            dialog = dialog.set_directory(start);
        }
        if folder { dialog.pick_folder() } else { dialog.pick_file() }
    }

    /// What is done with a path the app's own chooser gave.
    pub(super) fn finish_pick(&mut self, purpose: Purpose, path: PathBuf) {
        match purpose {
            Purpose::NotebooksFolder => self.open_notebooks_at(path),
            Purpose::ImportFolder => self.import_officeimo_source(path, true),
            Purpose::ImportFile => self.import_officeimo_source(path, false),
            Purpose::ImportPdf => self.import_pdf_from(&path),
            Purpose::Printout(dir) => self.add_file(&dir, &path, FileChoice::Printout),
            Purpose::AttachFile(dir) => self.dialog = Some(Dialog::FileOptions { dir, files: vec![path] }),
        }
    }

    /// The File screen: notebooks and what is done to a whole notebook.
    pub(super) fn backstage_screen(&mut self, ui: &mut Ui) {
        let page = self.backstage.unwrap_or("Info");
        let tokens = self.tokens;
        let folder = self.shown(self.session.root());
        let recent = self.recent_notebooks();
        let (root, redact) = (self.session.root().to_path_buf(), self.redact_paths);
        let show_path = move |p: &Path| shown_in(&root, p, redact);
        let mut acts = Vec::new();
        egui::CentralPanel::default().show(ui, |ui| {
            let telemetry_root = self.session.settings.path().parent().unwrap_or(std::path::Path::new(".")).join("telemetry");
            let telemetry_level = self.session.settings.get("telemetry_level").unwrap_or_else(|| "none".into());
            let session_id = self.session.settings.get("telemetry_session_id");
            acts = backstage::show(
                ui,
                self.session.tree(),
                &recent,
                &folder,
                page,
                &show_path,
                &telemetry_root,
                &telemetry_level,
                session_id.as_deref(),
                &mut self.sync,
                self.show_perf,
                self.fps_limit,
                self.compact_ui,
                self.recycle_days,
                &tokens,
            );
        });
        for a in acts {
            match a {
                backstage::Act::Close => self.backstage = None,
                backstage::Act::Page(name) => self.backstage = Some(name),
                backstage::Act::NewNotebook => {
                    self.backstage = None;
                    self.dialog = Some(Dialog::NewNotebook { name: String::new(), format: None });
                }
                backstage::Act::CreateNotebook(name, colour) => {
                    if let Some(Output::Path(dir)) = self.run("notebook.create", &params(&[("name", name.as_str())])) {
                        if let Some(c) = colour {
                            self.run("node.color", &params(&[("path", &path_str(&dir)), ("color", c.id())]));
                        }
                        self.session.refresh();
                        self.say("Notebook created");
                        self.open_notebook_from_path(&dir);
                    }
                }
                backstage::Act::OpenFolder(p) => self.reveal(&p),
                backstage::Act::ChooseFolder => self.choose_notebooks_folder(),
                backstage::Act::ImportNotebookFolder => self.import_notebook_folder(),
                backstage::Act::ImportFile => self.import_file(),
                backstage::Act::ImportPdf => self.import_pdf(),
                backstage::Act::ImportCloud => {
                    self.status = "Cloud import is not implemented yet.".to_string();
                    self.status_is_error = false;
                }
                backstage::Act::OpenNotebooksFolder => {
                    let root = self.session.root().to_path_buf();
                    self.reveal(&root);
                }
                backstage::Act::Open(p) => {
                    self.open_notebook_from_path(&p);
                }
                backstage::Act::OpenRecent(p) => {
                    self.open_notebook_from_path(&p);
                }
                backstage::Act::RemoveRecent(p) => self.remove_recent_notebook(&p),
                backstage::Act::DeleteRecent(p, title) => self.dialog = Some(Dialog::DeleteNotebook { path: p, title }),
                backstage::Act::SetTelemetryLevel(level) => {
                    let _ = self.session.settings.set("telemetry_level", &level);
                }
                backstage::Act::OpenTelemetryFolder(path) => {
                    // The folder is made on first use: it does not exist until a session has
                    // written to it, and Explorer opens Documents for a folder it cannot find.
                    match std::fs::create_dir_all(&path) {
                        Ok(()) => self.reveal(&path),
                        Err(e) => {
                            self.status = format!("The diagnostics folder could not be created: {e}");
                            self.status_is_error = true;
                        }
                    }
                }
                backstage::Act::ClearTelemetry(path) => self.dialog = Some(Dialog::ClearTelemetry { path }),
                backstage::Act::ResetPreferences => self.dialog = Some(Dialog::ResetPreferences),
                backstage::Act::ExportTelemetry(root) => {
                    if let Some(session) = std::fs::read_dir(&root)
                        .ok()
                        .and_then(|r| r.filter_map(Result::ok).find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)))
                        && let Some(destination) = rfd::FileDialog::new()
                            .set_title("Export telemetry ZIP")
                            .set_file_name(format!("notecraft-{}-telemetry.zip", session.file_name().to_string_lossy()))
                            .save_file()
                    {
                        let _ = crate::telemetry_export::export(&session.path(), &destination);
                    }
                }
                backstage::Act::SyncSave(now) => match self.sync.save(&self.session.settings) {
                    Ok(()) if now => self.sync_now(ui.ctx()),
                    Ok(()) => self.sync.status = "Saved.".into(),
                    Err(e) => {
                        self.sync.status = e;
                        self.sync.failed = true;
                    }
                },
                backstage::Act::SetRecycleDays(days) => {
                    self.recycle_days = days;
                    let _ = self.session.settings.set("recycle_days", &days.to_string());
                    self.bins_swept = None;
                }
                backstage::Act::SetLanguage(code) => {
                    crate::i18n::set_language(&code);
                    let _ = self.session.settings.set("language", &code);
                }
                backstage::Act::SetCompactUi(on) => {
                    self.compact_ui = on;
                    let _ = self.session.settings.set("layout", if on { "compact" } else { "classic" });
                }
                backstage::Act::SetFpsLimit(n) => {
                    self.fps_limit = n;
                    let _ = self.session.settings.set("fps_limit", &n.to_string());
                }
                backstage::Act::SetShowPerf(on) => {
                    self.show_perf = on;
                    let _ = self.session.settings.set("show_perf", if on { "true" } else { "false" });
                }
                backstage::Act::ExportNotebook(path) => {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "notebook".into());
                    if let Some(destination) =
                        rfd::FileDialog::new().set_title("Export notebook as ZIP").set_file_name(format!("{name}.zip")).save_file()
                    {
                        match crate::export::notebook_zip(&path, &destination) {
                            Ok(files) => self.say(format!("Exported {files} files of {name} to {}", destination.display())),
                            Err(e) => self.complain(&format!("Cannot export {name}: {e}")),
                        }
                    }
                }
                backstage::Act::AddWelcome => {
                    self.backstage = None;
                    self.add_welcome();
                }
                backstage::Act::NotBuilt(name) => {
                    self.status = format!("{name} is not built yet. See the roadmap for what is coming.");
                    self.status_is_error = false;
                }
            }
        }
    }

    pub(super) fn recent_notebooks(&self) -> Vec<Node> {
        let mut paths = Vec::new();
        if let Some(path) = self.editor.as_ref().and_then(|editor| notebook_for(&editor.dir)) {
            paths.push(path);
        }
        paths.extend(self.session.opened_elsewhere().iter().rev().cloned());
        if let Some(saved) = self.session.settings.get("recent_notebooks")
            && let Ok(saved) = serde_json::from_str::<Vec<String>>(&saved)
        {
            paths.extend(saved.into_iter().map(PathBuf::from));
        }
        let mut recent = Vec::new();
        let mut seen = HashSet::new();
        for path in paths {
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            if !seen.insert(path.clone()) {
                continue;
            }
            if let Some(notebook) = notecraft_store::tree::load_tree(&path)
                && notebook.info.kind == NodeKind::Notebook
            {
                recent.push(notebook);
                if recent.len() == 12 {
                    break;
                }
            }
        }
        recent
    }

    pub(super) fn remember_recent_notebook(&mut self, path: &Path) {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mut recent =
            self.session.settings.get("recent_notebooks").and_then(|saved| serde_json::from_str::<Vec<String>>(&saved).ok()).unwrap_or_default();
        let path_text = path.to_string_lossy().into_owned();
        recent.retain(|saved| std::fs::canonicalize(saved).unwrap_or_else(|_| PathBuf::from(saved)) != path);
        recent.insert(0, path_text);
        recent.truncate(12);
        if let Ok(serialized) = serde_json::to_string(&recent) {
            let _ = self.session.settings.set("recent_notebooks", &serialized);
        }
    }

    pub(super) fn remove_recent_notebook(&mut self, path: &Path) {
        let Some(saved) = self.session.settings.get("recent_notebooks") else { return };
        let Ok(mut recent) = serde_json::from_str::<Vec<String>>(&saved) else { return };
        recent.retain(|entry| std::fs::canonicalize(entry).unwrap_or_else(|_| PathBuf::from(entry)) != path);
        if let Ok(serialized) = serde_json::to_string(&recent) {
            let _ = self.session.settings.set("recent_notebooks", &serialized);
        }
        self.say("Removed notebook from the known notebooks list");
    }

    pub(super) fn open_notebook_from_path(&mut self, path: &Path) {
        self.save_now();
        let path_text = path_str(path);
        if let Some(Output::Path(opened)) = self.run("notebook.open", &params(&[("path", &path_text)])) {
            self.remember_recent_notebook(&opened);
            self.open.insert(opened.clone());
            self.selected = Some(opened.clone());
            self.backstage = None;
            if let Some(page) = first_page(self.session.tree(), &opened) {
                self.open_page(&page);
            } else {
                self.editor = None;
                self.pad = None;
                self.say(format!("Opened {}; it has no pages yet", self.shown(&opened)));
            }
        }
    }

    /// Asks the system for a folder and opens the notebooks in it, keeping everything that
    /// is already open. A folder that is itself a notebook opens as one; a folder that
    /// holds notebooks opens all of them. Nothing is closed and nothing is moved.
    pub(super) fn choose_notebooks_folder(&mut self) {
        let title = "Choose a notebook, a folder of notebooks, or an empty folder for a new notebook";
        if let Some(picked) = self.pick(Purpose::NotebooksFolder, title, true, &[], None) {
            self.open_notebooks_at(picked);
        }
    }

    pub(super) fn open_notebooks_at(&mut self, picked: PathBuf) {
        let is_notebook = notecraft_store::tree::read_node(&picked).is_some_and(|i| i.kind == NodeKind::Notebook);
        let wanted: Vec<std::path::PathBuf> =
            if is_notebook { vec![picked.clone()] } else { notecraft_store::tree::load_notebooks(&picked).into_iter().map(|n| n.path).collect() };
        if wanted.is_empty() {
            let p = path_str(&picked);
            if let Some(Output::Path(opened)) = self.run("notebook.open", &params(&[("path", &p)])) {
                self.remember_recent_notebook(&opened);
                self.open.insert(opened.clone());
                self.selected = Some(opened);
                self.backstage = None;
                self.say(format!("{} opened as an empty notebook", self.shown(&picked)));
            }
            return;
        }
        let mut opened_count = 0;
        let mut first = None;
        for path in wanted {
            let p = path_str(&path);
            if let Some(Output::Path(notebook_path)) = self.run("notebook.open", &params(&[("path", &p)])) {
                self.remember_recent_notebook(&notebook_path);
                opened_count += 1;
                first.get_or_insert(path.clone());
                self.open.insert(notebook_path);
            }
        }
        if let Some(path) = first {
            self.selected = Some(path);
        }
        self.backstage = None;
        self.say(match opened_count {
            1 => "Notebook opened".to_string(),
            n => format!("{n} notebooks opened"),
        });
    }

    pub(super) fn reveal(&mut self, path: &Path) {
        match open_folder(path) {
            Ok(()) => self.say(format!("Opened {}", self.shown(path))),
            Err(e) => {
                self.status = format!("The folder could not be opened: {e}");
                self.status_is_error = true;
            }
        }
    }

    /// Adds the Welcome notebook and opens it on its start page; if it is already in the
    /// folder, opens that one instead.
    pub fn add_welcome(&mut self) {
        // Always call install to replace with the current bundled demo version
        // (allows demo updates on every startup)
        let message = match self.run("notebook.welcome", &Params::default()) {
            Some(Output::Path(dir)) => {
                // Refresh the session to pick up the newly replaced content
                self.session.refresh();
                // Now open the updated demo
                self.remember_recent_notebook(&dir);
                self.open.insert(dir.clone());
                self.selected = Some(dir.clone());
                if let Some(first) = first_page_under(self.session.tree(), &dir) {
                    self.open_path(&path_str(&first));
                }
                "Updated the welcome notebook".to_string()
            }
            _ => return,
        };
        // After the page opened, so the message is the one the user reads.
        if !self.status_is_error {
            self.say(message);
        }
    }

    /// Follows a link written in the open page. Another page of the notebooks opens in the
    /// window; a notebook, section group or section opens on its first page; a file opens in
    /// the program the system has for it; a web address opens in the browser. A relative
    /// path can never reach outside the notebooks folder (`notecraft_store::links`).
    /// Puts a link to a page on the clipboard, as OneNote's Copy Link to Page does.
    ///
    /// The link names the page from the notebooks folder down
    /// (`notecraft:/book/section/page`), not relative to anything, because what a relative
    /// link would be relative to is the page it is pasted into, and nobody knows that when
    /// the link is copied. Pasted into a page it reads as an ordinary link and opens the
    /// page it names.
    pub fn copy_link_to(&mut self, dir: &Path) {
        let mut roots = vec![self.session.root().to_path_buf()];
        roots.extend(self.session.opened_elsewhere().iter().cloned());
        let Some(uri) = notecraft_store::links::page_uri(&roots, dir) else {
            self.complain("That page is not in a notebook that is open");
            return;
        };
        let title = notecraft_store::tree::read_node(dir).map(|n| n.title).unwrap_or_else(|| "Page".to_string());
        // Plain text keeps the title and address readable; the matching HTML flavor is an
        // anchor for clients that accept rich content.
        self.pending_copy = Some(format!("{title} ({uri})"));
        self.pending_copy_html = Some(crate::clipboard::html_link(&title, &uri));
        self.say(format!("Link to \"{title}\" copied"));
    }

    pub fn follow_link(&mut self, ctx: &Context, href: &str) {
        use notecraft_store::links::{Link, resolve_in};
        let Some(page) = self.editor.as_ref().map(|e| e.dir.clone()) else { return };
        // Every folder a notebook is open from, not just the notebooks folder: a notebook
        // opened from elsewhere is as much inside as one that lives there, and judging it
        // by the notebooks folder alone made every link in it dead.
        let mut roots = vec![self.session.root().to_path_buf()];
        roots.extend(self.session.opened_elsewhere().iter().cloned());
        match resolve_in(&roots, &page, href) {
            Link::Web(url) => {
                ctx.open_url(egui::OpenUrl::new_tab(url.clone()));
                self.say(format!("Opened {url} in the browser"));
            }
            Link::Node(dir) => {
                if notecraft_store::tree::read_node(&dir).is_some_and(|n| n.kind == NodeKind::Page) {
                    self.open_path(&path_str(&dir));
                } else {
                    self.open.insert(dir.clone());
                    self.selected = Some(dir.clone());
                    match first_page_under(self.session.tree(), &dir) {
                        Some(first) => self.open_path(&path_str(&first)),
                        None => self.say(format!("Opened {}; it has no pages yet", self.shown(&dir))),
                    }
                }
            }
            Link::File(file) => {
                if self.open_attachment_warning_suppressed {
                    match open_file_for_viewer(&file) {
                        Ok(()) => self.say(format!("Opening {}", self.shown(&file))),
                        Err(e) => self.complain(&format!("The file could not be opened: {e}")),
                    }
                } else {
                    self.dialog = Some(Dialog::OpenAttachmentWarning { path: file });
                }
            }
            Link::Missing(p) => {
                self.status = format!("Nothing at {}: the link points to a page or file that is not there", self.shown(&p));
                self.status_is_error = true;
            }
            Link::Outside => {
                self.status = "That link points outside the notebooks folder, so it was not followed".to_string();
                self.status_is_error = true;
            }
        }
    }
}
