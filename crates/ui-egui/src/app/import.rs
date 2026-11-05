//! Bringing notebooks, files and PDFs in.

use super::*;

impl NoteCraftApp {
    pub(super) fn import_notebook_folder(&mut self) {
        let start = notecraft_import::discover::default_source();
        if let Some(source) = self.pick(Purpose::ImportFolder, "Choose a OneNote notebook folder", true, &[], start) {
            self.import_officeimo_source(source, true);
        }
    }

    pub(super) fn import_file(&mut self) {
        let start = notecraft_import::discover::default_source();
        if let Some(source) = self.pick(Purpose::ImportFile, "Choose a file to import", false, &["one"], start) {
            self.import_officeimo_source(source, false);
        }
    }

    pub(super) fn import_pdf(&mut self) {
        let start = notecraft_import::discover::default_source();
        if let Some(source) = self.pick(Purpose::ImportPdf, "Choose a PDF to import", false, &["pdf"], start) {
            self.import_pdf_from(&source);
        }
    }

    /// Where a PDF goes: the notebook or section group the selection is in, else the first notebook.
    pub(super) fn pdf_destination(&self) -> Option<PathBuf> {
        let mut at = self.selected.as_deref();
        while let Some(dir) = at {
            if notecraft_store::tree::read_node(dir).is_some_and(|n| matches!(n.kind, NodeKind::Notebook | NodeKind::SectionGroup)) {
                return Some(dir.to_path_buf());
            }
            at = dir.parent();
        }
        self.session.tree().first().map(|n| n.path.clone())
    }

    /// Brings a PDF into a notebook without touching it: a section named for the file, with a page for each sheet. The sheet
    /// is drawn as a picture at the top of its page, the pen and the words go over and under it, and the original file is kept
    /// whole in the first page's folder. Nothing is ever written back to the PDF.
    pub fn import_pdf_from(&mut self, source: &Path) {
        if !source.extension().and_then(std::ffi::OsStr::to_str).is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            self.complain("Only a PDF can be imported this way");
            return;
        }
        let Some(parent) = self.pdf_destination() else {
            self.complain("Open or create a notebook first");
            return;
        };
        let rendered = match printout::render(source) {
            Ok(r) => r,
            Err(e) => {
                self.complain(&format!("Could not read the PDF: {e}"));
                return;
            }
        };
        let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "document.pdf".into());
        let stem = source.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "PDF".into());
        let Some(Output::Path(section)) = self.run("node.create", &params(&[("parent", &path_str(&parent)), ("kind", "section"), ("title", &stem)]))
        else {
            return;
        };
        let total = rendered.pages().len();
        let mut first = None;
        for (index, png) in rendered.pages().iter().enumerate() {
            let number = (index + 1).to_string();
            let title = format!("Page {number}");
            let Some(Output::Path(page)) =
                self.run("node.create", &params(&[("parent", &path_str(&section)), ("kind", "page"), ("title", &title), ("format", "cmgfm")]))
            else {
                return;
            };
            let assets = page.join("assets");
            let copied = std::fs::create_dir_all(&assets).and_then(|()| std::fs::copy(png, assets.join("pdf-page.png")).map(|_| ())).and_then(|()| {
                if index == 0 {
                    // The file, whole, and its icon beside it: the first sheet, which is also what the page shows.
                    std::fs::copy(source, assets.join(&name))?;
                    std::fs::copy(png, assets.join(format!("{name}.png")))?;
                }
                Ok(())
            });
            if let Err(e) = copied {
                self.complain(&format!("Could not store page {number} of the PDF: {e}"));
                return;
            }
            // As wide as the sheet's column, in the sheet's own proportions.
            let (w, h) = image::image_dimensions(png).unwrap_or((595, 842));
            let height = (760.0 * h as f32 / w.max(1) as f32).round();
            let alt = format!("{stem}, page {number} of {total}").replace('"', "'");
            let mut body = format!("<img src=\"assets/pdf-page.png\" alt=\"{alt}\" width=\"760\" height=\"{height}\">\n");
            if index == 0 {
                body.push_str(&format!("\n[The original PDF: {name}](assets/{name})\n"));
            }
            self.run("page.save", &params(&[("path", &path_str(&page)), ("body", &body)]));
            // The picture is the first block, as wide as the sheet, so the pen lands on the PDF's own lines.
            self.run("page.containers", &params(&[("path", &path_str(&page)), ("boxes", "0,0,760,1")]));
            self.run("page.sheet", &params(&[("path", &path_str(&page)), ("sheet", "a4")]));
            first.get_or_insert(page);
        }
        self.session.refresh();
        self.open.insert(parent);
        self.open.insert(section);
        if let Some(page) = first {
            self.open_page(&page);
        }
        self.say(format!("Imported {name}: {total} page{}. The PDF itself is not changed", if total == 1 { "" } else { "s" }));
    }

    pub(super) fn import_officeimo_source(&mut self, source: PathBuf, folder: bool) {
        let format = self
            .session
            .settings
            .get("page_format")
            .and_then(|v| notecraft_model::PageFormat::parse(&v).ok())
            .unwrap_or(notecraft_model::PageFormat::Html);
        let bridge_format = match format {
            notecraft_model::PageFormat::Html => notecraft_import::officeimo::Format::Html,
            notecraft_model::PageFormat::Cmgfm => notecraft_import::officeimo::Format::Cmgfm,
        };
        let destination = self.unique_import_destination(&source);
        let output = if folder { destination.parent().unwrap_or(self.session.root()) } else { &destination };
        log::info!("OfficeIMO import requested: source={} destination={}", source.display(), destination.display());
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let source_for_worker = source.clone();
        let output_for_worker = output.to_path_buf();
        std::thread::spawn(move || {
            let mut progress = |event| {
                let message = match event {
                    notecraft_import::officeimo::Progress::Found { total } => ImportMessage::Found(total),
                    notecraft_import::officeimo::Progress::Section { index, total, label } => {
                        ImportMessage::Section { index, total, label: label.to_string() }
                    }
                };
                let _ = tx.send(message);
            };
            let result =
                notecraft_import::officeimo::run_with_progress(&source_for_worker, &output_for_worker, bridge_format, &worker_cancel, &mut progress);
            let _ = tx.send(ImportMessage::Finished(result));
        });
        self.import_job =
            Some(ImportJob { rx, keep_alive: None, cancel, source, destination, current: "Starting OfficeIMO…".into(), index: 0, total: 0 });
        self.status = "Importing…".into();
        self.status_is_error = false;
    }

    pub(super) fn poll_import(&mut self, ctx: &Context) {
        let Some(job) = self.import_job.as_mut() else { return };
        let mut finished = None;
        loop {
            match job.rx.try_recv() {
                Ok(ImportMessage::Found(total)) => job.total = total,
                Ok(ImportMessage::Section { index, total, label }) => {
                    job.index = index;
                    job.total = total;
                    job.current = label;
                }
                Ok(ImportMessage::Finished(result)) => {
                    finished = Some(result);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = Some(Err("OfficeIMO import worker stopped unexpectedly".into()));
                    break;
                }
            }
        }
        if let Some(result) = finished {
            let Some(job) = self.import_job.take() else { return };
            match result {
                Ok(message) => {
                    let _ = self.run("tree.refresh", &Params::new());
                    log::info!("OfficeIMO import succeeded: source={} destination={}", job.source.display(), job.destination.display());
                    self.status = message;
                    self.status_is_error = false;
                    self.open_notebook_from_path(&job.destination);
                }
                Err(error) => {
                    log::error!("OfficeIMO import failed: source={} destination={} error={error}", job.source.display(), job.destination.display());
                    self.status = error;
                    self.status_is_error = true;
                }
            }
        } else if job.keep_alive.is_none() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    pub(super) fn import_progress(&mut self, ctx: &Context) {
        let Some(job) = self.import_job.as_mut() else { return };
        let mut cancel = false;
        egui::Window::new("Importing OneNote")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
            .frame(modal_frame(&self.tokens))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                modal_title(ui, &self.tokens, "Importing OneNote");
                ui.label(egui::RichText::new(crate::i18n::tr("You can continue working while the import runs.")).color(self.tokens.muted));
                ui.add_space(12.0);
                ui.label(egui::RichText::new(&job.current).strong());
                let fraction = if job.total == 0 { 0.0 } else { job.index as f32 / job.total as f32 };
                ui.add(egui::ProgressBar::new(fraction).fill(self.tokens.accent).show_percentage());
                if job.total > 0 {
                    ui.label(format!("{} of {} sections", job.index, job.total));
                } else {
                    ui.label(crate::i18n::tr("Discovering sections…"));
                }
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if modal_button(ui, &self.tokens, "Cancel import", true).clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            job.cancel.store(true, Ordering::Relaxed);
            job.current = "Cancelling…".into();
        }
    }

    pub(super) fn import_notebook_name(source: &Path) -> String {
        let stem = source.file_stem().and_then(|name| name.to_str()).unwrap_or("Imported notebook");
        let name = stem.split_once(" (On ").map_or(stem, |(name, _)| name).trim();
        let cleaned: String =
            name.chars().map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '_' } else { c }).collect();
        let cleaned = cleaned.trim().trim_end_matches('.');
        if cleaned.is_empty() { "Imported notebook".to_string() } else { cleaned.to_string() }
    }

    pub(super) fn unique_import_destination(&self, source: &Path) -> PathBuf {
        let base = Self::import_notebook_name(source);
        let root = self.session.root();
        let first = root.join(&base);
        if !first.exists() {
            return first;
        }
        for number in 1..=u32::MAX {
            let candidate = root.join(format!("{base} ({number})"));
            if !candidate.exists() {
                return candidate;
            }
        }
        root.join(format!("{base} ({})", u64::from(u32::MAX) + 1))
    }

    /// Shows a deterministic progress state for documentation screenshots.
    pub fn show_demo_import_progress(&mut self, index: usize, total: usize, current: &str) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.import_job = Some(ImportJob {
            rx,
            keep_alive: Some(tx),
            cancel: Arc::new(AtomicBool::new(false)),
            source: PathBuf::from("demo-source"),
            destination: PathBuf::from("demo-destination"),
            current: current.to_string(),
            index,
            total,
        });
    }
}
