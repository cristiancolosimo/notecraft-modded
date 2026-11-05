//! The windows that ask something.

use super::*;

impl NoteCraftApp {
    pub(super) fn dialogs(&mut self, ctx: &Context) {
        while self.dialog.is_none() {
            let Some(next) = self.queued_alerts.pop_front() else { break };
            if !matches!(&next, Dialog::Alert { key, .. } if self.muted_alerts.contains(key)) {
                self.dialog = Some(next);
            }
        }
        let Some(dialog) = self.dialog.take() else { return };
        // Page loading and native Shell icon generation can finish between the
        // recovery scan and the frame that presents this modal. Re-check the
        // filesystem so a healthy sidecar never produces a stale warning.
        if let Dialog::MissingAttachmentIcon { page, name } = &dialog
            && attachment_icon_healthy(page, name)
        {
            log::debug!("discarding stale missing-icon warning: page={page:?} attachment={name:?}");
            return;
        }
        let mut keep = true;
        let mut chosen_path: Option<(Purpose, PathBuf)> = None;
        let mut next = dialog;
        match &mut next {
            Dialog::ClearTelemetry { path } => {
                let mut clear = false;
                let mut cancel = false;
                egui::Window::new("Clear telemetry")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(440.0);
                        modal_title(ui, &self.tokens, "Clear telemetry");
                        ui.label(crate::i18n::tr("Clear all local telemetry sessions and exported telemetry ZIP files?"));
                        ui.add_space(5.0);
                        ui.colored_label(self.tokens.warn, crate::i18n::tr("This cannot be undone."));
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                cancel = true;
                            }
                            if modal_button(ui, &self.tokens, "Clear telemetry", true).clicked() {
                                clear = true;
                            }
                        });
                    });
                if clear {
                    let _ = std::fs::remove_dir_all(path);
                    keep = false;
                } else if cancel {
                    keep = false;
                }
            }
            Dialog::PrivacyWelcome { level } => {
                let mut accepted = false;
                let mut denied = false;
                modal_backdrop(ctx, "privacy-welcome-modal-backdrop");
                egui::Window::new("Welcome to NoteCraft")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, "Welcome to NoteCraft");
                        ui.set_min_width(520.0);
                        ui.label(crate::i18n::tr("Thank you for trying NoteCraft!"));
                        ui.add_space(6.0);
                        ui.label(
                            "Privacy is our top priority. All logs and telemetry stay on this device. NoteCraft never sends them automatically.",
                        );
                        ui.add_space(6.0);
                        ui.label(crate::i18n::tr("Choose a local collection level. Logging is off by default in release builds."));
                        egui::ComboBox::from_id_salt("privacy-welcome-level").selected_text(level.as_str()).show_ui(ui, |ui| {
                            for choice in ["none", "usage", "warning", "error", "debug"] {
                                ui.selectable_value(level, choice.into(), choice);
                            }
                        });
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Accept", true).clicked() {
                                accepted = true;
                            }
                            if modal_button(ui, &self.tokens, "Deny", false).clicked() {
                                denied = true;
                            }
                        });
                    });
                if accepted {
                    let _ = self.session.settings.set("telemetry_level", level);
                    let _ = self.session.settings.set("privacy_welcome_seen", "true");
                    keep = false;
                } else if denied {
                    let _ = self.session.settings.set("telemetry_level", "none");
                    let _ = self.session.settings.set("privacy_welcome_seen", "true");
                    keep = false;
                }
            }
            Dialog::Alert { key, title, message, suppress, suppress_forever } => {
                let mut ok = false;
                egui::Window::new(title.as_str())
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, title.as_str());
                        ui.set_min_width(420.0);
                        ui.label(crate::i18n::tr_dyn(message.as_str()).into_owned());
                        ui.add_space(8.0);
                        ui.checkbox(suppress, crate::i18n::tr("Don't show this again in this session"));
                        ui.checkbox(suppress_forever, crate::i18n::tr("Don't show this ever"));
                        ui.add_space(12.0);
                        if modal_button(ui, &self.tokens, "OK", true).clicked() {
                            ok = true;
                        }
                    });
                if ok {
                    if *suppress {
                        self.muted_alerts.insert(key.clone());
                    }
                    if *suppress_forever {
                        let _ = self.session.settings.set(&format!("{key}_never"), "true");
                    }
                    keep = false;
                }
            }
            Dialog::ResetPreferences => {
                let mut reset = false;
                let mut cancel = false;
                egui::Window::new("Reset app preferences")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, "Reset app preferences");
                        ui.set_min_width(460.0);
                        ui.label(crate::i18n::tr("Reset NoteCraft's saved preferences to their built-in defaults?"));
                        ui.add_space(6.0);
                        ui.colored_label(self.tokens.warn, crate::i18n::tr("This does not delete notebooks or pages. It cannot be undone."));
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Reset preferences", true).clicked() {
                                reset = true;
                            }
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                cancel = true;
                            }
                        });
                    });
                if reset {
                    match self.session.settings.reset() {
                        Ok(()) => self.say("App preferences reset. Restart NoteCraft to apply every default."),
                        Err(e) => self.complain(&format!("Could not reset app preferences: {e}")),
                    }
                    keep = false;
                } else if cancel {
                    keep = false;
                }
            }
            Dialog::MissingAttachmentIcon { page, name } => {
                let mut repair = false;
                let mut leave = false;
                let mut suppress = self.icon_recovery_suppressed;
                modal_backdrop(ctx, "missing-attachment-icon-backdrop");
                egui::Window::new("Attachment recovery")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, "Attachment icon missing");
                        ui.set_min_width(520.0);
                        ui.label(egui::RichText::new(format!("NoteCraft found an attachment without its generated icon:\n{name}")).strong());
                        ui.add_space(6.0);
                        ui.label(crate::i18n::tr("The original file is still safe. NoteCraft can regenerate the icon beside it."));
                        ui.add_space(8.0);
                        ui.checkbox(&mut suppress, crate::i18n::tr("Do not warn about similar missing icons again this session"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Repair icon", true).clicked() {
                                repair = true;
                            }
                            if modal_button(ui, &self.tokens, "Leave it", false).clicked() {
                                leave = true;
                            }
                        });
                    });
                if repair {
                    self.icon_recovery_suppressed = suppress;
                    match notecraft_store::assets::ensure_icon(page, name).and_then(|()| {
                        let icon_exists = attachment_icon_exists(page, name);
                        if !icon_exists {
                            return Err(notecraft_store::StoreError::invalid("the icon generator produced no sidecar file"));
                        }
                        self.repair_attachment_reference(page, name).map_err(notecraft_store::StoreError::invalid).map(|_| ())
                    }) {
                        Ok(()) => {
                            log::info!("repaired missing attachment icon and reference: page={:?} attachment={name:?}", page);
                            self.say(format!("Repaired the attachment icon for {name}"));
                        }
                        Err(e) => {
                            log::error!("could not repair missing attachment icon: page={:?} attachment={name:?} error={e}", page);
                            self.complain(&format!("Could not repair the icon for {name}: {e}"));
                        }
                    }
                    keep = false;
                } else if leave {
                    self.icon_recovery_suppressed = suppress;
                    log::error!("user declined missing attachment icon repair: page={:?} attachment={name:?}", page);
                    keep = false;
                } else {
                    self.icon_recovery_suppressed = suppress;
                }
            }
            Dialog::UnreferencedAttachment { page, name } => {
                let mut restore = false;
                let mut leave = false;
                let mut suppress = self.icon_recovery_suppressed;
                modal_backdrop(ctx, "unreferenced-attachment-backdrop");
                egui::Window::new("Attachment recovery")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, "Attachment not linked");
                        ui.set_min_width(520.0);
                        ui.label(egui::RichText::new(format!("NoteCraft found an attachment that is not referenced by this page:\n{name}")).strong());
                        ui.add_space(6.0);
                        ui.label(crate::i18n::tr("The file is safe. NoteCraft can append a link to it at the end of the page."));
                        ui.add_space(8.0);
                        ui.checkbox(&mut suppress, crate::i18n::tr("Do not warn about similar attachment issues again this session"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Append link", true).clicked() {
                                restore = true;
                            }
                            if modal_button(ui, &self.tokens, "Leave it", false).clicked() {
                                leave = true;
                            }
                        });
                    });
                if restore {
                    self.icon_recovery_suppressed = suppress;
                    match self.repair_attachment_reference(page, name) {
                        Ok(true) => {
                            log::info!("restored unreferenced attachment link: page={:?} attachment={name:?}", page);
                            self.say(format!("Added a link to {name} at the end of the page"));
                        }
                        Ok(false) => log::debug!("attachment recovery found no change needed: page={:?} attachment={name:?}", page),
                        Err(e) => {
                            log::error!("could not restore unreferenced attachment link: page={:?} attachment={name:?} error={e}", page);
                            self.complain(&format!("Could not restore the link for {name}: {e}"));
                        }
                    }
                    keep = false;
                } else if leave {
                    self.icon_recovery_suppressed = suppress;
                    log::error!("user declined unreferenced attachment recovery: page={:?} attachment={name:?}", page);
                    keep = false;
                } else {
                    self.icon_recovery_suppressed = suppress;
                }
            }
            Dialog::OpenAttachmentWarning { path } => {
                let mut open = false;
                let mut cancel = false;
                let mut suppress = self.open_attachment_warning_suppressed;
                modal_backdrop(ctx, "open-attachment-warning-backdrop");
                egui::Window::new("Open attachment")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, "Open attachment");
                        ui.set_min_width(520.0);
                        ui.label(egui::RichText::new(crate::i18n::tr("This file will be opened by its associated application.")).strong());
                        ui.add_space(6.0);
                        ui.label(self.shown(path).to_string());
                        ui.add_space(6.0);
                        ui.label(crate::i18n::tr("Only open it if you trust the file and the person who created it."));
                        ui.add_space(8.0);
                        ui.checkbox(&mut suppress, crate::i18n::tr("Do not warn again for this file type this session"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Open", true).clicked() {
                                open = true;
                            }
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                cancel = true;
                            }
                        });
                    });
                if open {
                    self.open_attachment_warning_suppressed = suppress;
                    match open_file_for_viewer(path) {
                        Ok(()) => self.say(format!("Opening {}", self.shown(path))),
                        Err(e) => self.complain(&format!("Could not open the file: {e}")),
                    }
                    keep = false;
                } else if cancel {
                    self.open_attachment_warning_suppressed = suppress;
                    log::info!("user cancelled opening attachment: file={:?}", path);
                    keep = false;
                } else {
                    self.open_attachment_warning_suppressed = suppress;
                }
            }
            Dialog::Browse { purpose, browser } => {
                if let Some(access) = self.files.clone() {
                    match browser.show(ctx, &access, &self.tokens, self.dialog_lift) {
                        crate::filebrowser::Outcome::Open => {}
                        crate::filebrowser::Outcome::Cancel => keep = false,
                        crate::filebrowser::Outcome::Chosen(path) => {
                            keep = false;
                            chosen_path = Some((purpose.clone(), path));
                        }
                    }
                } else {
                    keep = false;
                }
            }
            Dialog::PickMedia { dir, picker } => {
                if let Some(access) = self.media.clone() {
                    match picker.show(ctx, &access, &self.tokens) {
                        crate::mediapicker::Outcome::Open => {}
                        crate::mediapicker::Outcome::Cancel => keep = false,
                        crate::mediapicker::Outcome::Chosen(file) => {
                            let dir = dir.clone();
                            next = Dialog::FileOptions { dir, files: vec![file] };
                        }
                    }
                } else {
                    keep = false;
                }
            }
            Dialog::FileOptions { dir, files } => {
                let (dir, file) = (dir.clone(), files.first().cloned().unwrap_or_default());
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let left = files.len().saturating_sub(1);
                let kind = notecraft_store::assets::media_kind(&name);
                let mut chosen: Option<FileChoice> = None;
                let title = if left > 0 { format!("Add {name} ({left} more)") } else { format!("Add {name}") };
                egui::Window::new("file-options")
                    .collapsible(false)
                    .resizable(false)
                    .title_bar(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        modal_title(ui, &self.tokens, &title);
                        ui.label(
                            egui::RichText::new(crate::i18n::tr("What should happen to this file?")).strong().size(16.0).color(self.tokens.text),
                        );
                        ui.add_space(6.0);
                        let copy_label = match kind {
                            notecraft_store::assets::MediaKind::Image => "Insert the picture",
                            notecraft_store::assets::MediaKind::Video => "Insert the video",
                            notecraft_store::assets::MediaKind::Audio => "Insert the recording",
                            notecraft_store::assets::MediaKind::Other => "Attach the file",
                        };
                        if modal_button(ui, &self.tokens, copy_label, true).clicked() {
                            log::debug!("file attach choice clicked: choice=copy name={name:?}");
                            chosen = Some(FileChoice::Copy);
                        }
                        ui.label(
                            egui::RichText::new(crate::i18n::tr("A copy goes into this page's folder, so the notebook carries it everywhere."))
                                .small()
                                .color(self.tokens.muted),
                        );
                        ui.add_space(6.0);
                        let can_printout = printout::supports(&file);
                        if modal_button_enabled(ui, &self.tokens, "Insert printout", false, can_printout).clicked() {
                            log::debug!("file attach choice clicked: choice=printout name={name:?}");
                            chosen = Some(FileChoice::Printout);
                        }
                        ui.label(
                            egui::RichText::new(if can_printout {
                                "Copies the file and inserts one picture for each printed page."
                            } else {
                                "Printout is available for PDF and Office documents."
                            })
                            .small()
                            .color(self.tokens.muted),
                        );
                        ui.add_space(6.0);
                        if modal_button(ui, &self.tokens, "Link to the file where it is", false).clicked() {
                            log::debug!("file attach choice clicked: choice=link name={name:?}");
                            chosen = Some(FileChoice::Link);
                        }
                        ui.label(
                            egui::RichText::new(crate::i18n::tr("Nothing is copied. The link breaks if the file moves."))
                                .small()
                                .color(self.tokens.muted),
                        );
                        ui.add_space(6.0);
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                        });
                    });
                if let Some(how) = chosen {
                    self.add_file(&dir, &file, how);
                    if let Dialog::FileOptions { files, .. } = &mut next {
                        files.remove(0);
                        keep = !files.is_empty();
                    }
                }
            }
            Dialog::NewNotebook { name, format } => {
                let mut go = false;
                egui::Window::new("New notebook")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, "New notebook");
                        ui.set_min_width(420.0);
                        ui.label(crate::i18n::tr("Name"));
                        let r = ui.text_edit_singleline(name);
                        // Read Enter before asking for focus again: a new focus request hides the lost focus.
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if !r.has_focus() {
                            // Once: asking again every frame restarts the on-screen keyboard every frame.
                            r.request_focus();
                        }
                        format_picker(ui, format);
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                            let ready = !name.trim().is_empty();
                            let create = ui.add_enabled_ui(ready, |ui| modal_button(ui, &self.tokens, "Create", true).clicked()).inner;
                            if (create || enter) && ready {
                                go = true;
                            }
                        });
                    });
                if go {
                    let mut p = params(&[("name", name.trim())]);
                    if let Some(f) = format {
                        p.insert("format".into(), f.id().into());
                    }
                    if let Some(Output::Path(dir)) = self.run("notebook.create", &p) {
                        self.open.insert(dir);
                        self.say("Notebook created");
                    }
                    keep = false;
                }
            }
            Dialog::Add { parent, kind: NodeKind::Page, title, format, .. } => {
                let (go, cancel) = self.new_page_modal(ctx, parent, title, format);
                if cancel {
                    keep = false;
                }
                if go {
                    // A page may be left without a title: it is made untitled, to be named later.
                    let mut p = params(&[("parent", &path_str(parent)), ("kind", "page")]);
                    if !title.trim().is_empty() {
                        p.insert("title".into(), title.trim().into());
                    }
                    if let Some(f) = format {
                        p.insert("format".into(), f.id().into());
                    }
                    if let Some(Output::Path(dir)) = self.run("node.create", &p) {
                        self.open.insert(parent.clone());
                        let _ = self.session.settings.set("new_page_sheet", if self.new_page_a4 { "a4" } else { "none" });
                        self.give_sheet(&dir);
                        self.open_page(&dir);
                        self.say("Created");
                    }
                    keep = false;
                }
            }
            Dialog::Add { parent, parent_kind, title, kind, format } => {
                let mut go = false;
                let heading = if *parent_kind == NodeKind::Page {
                    "Add a sub-page".to_string()
                } else {
                    format!("Add inside this {}", parent_kind.label().to_lowercase())
                };
                egui::Window::new(heading.clone())
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, &heading);
                        ui.set_min_width(420.0);
                        let kinds = parent_kind.may_contain();
                        if kinds.len() > 1 {
                            ui.label(crate::i18n::tr("What to add"));
                            ui.horizontal(|ui| {
                                for k in kinds {
                                    ui.selectable_value(kind, *k, k.label());
                                }
                            });
                        }
                        ui.label(crate::i18n::tr("Title"));
                        let r = ui.text_edit_singleline(title);
                        // Read Enter before asking for focus again: a new focus request hides the lost focus.
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if !r.has_focus() {
                            // Once: asking again every frame restarts the on-screen keyboard every frame.
                            r.request_focus();
                        }
                        let what = if *kind == NodeKind::Page { "Format for this page" } else { "Default format for pages below" };
                        ui.label(what);
                        format_picker(ui, format);
                        if *kind == NodeKind::Page {
                            ui.add_space(6.0);
                            ui.label(crate::i18n::tr("Page type"));
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut self.new_page_a4, false, "Infinite scroll")
                                    .on_hover_text(crate::i18n::tr("One page with no end"));
                                ui.selectable_value(&mut self.new_page_a4, true, "A4 sheets")
                                    .on_hover_text(crate::i18n::tr("Sheets that grow as you scroll down"));
                            });
                        }
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                            let ready = !title.trim().is_empty();
                            let create = ui.add_enabled_ui(ready, |ui| modal_button(ui, &self.tokens, "Create", true).clicked()).inner;
                            if (create || enter) && ready {
                                go = true;
                            }
                        });
                    });
                if go {
                    let mut p = params(&[("parent", &path_str(parent)), ("kind", kind.id()), ("title", title.trim())]);
                    if let Some(f) = format {
                        p.insert("format".into(), f.id().into());
                    }
                    if let Some(Output::Path(dir)) = self.run("node.create", &p) {
                        self.open.insert(parent.clone());
                        if *kind == NodeKind::Page {
                            let _ = self.session.settings.set("new_page_sheet", if self.new_page_a4 { "a4" } else { "none" });
                            self.give_sheet(&dir);
                            self.open_page(&dir);
                        }
                        self.say("Created");
                    }
                    keep = false;
                }
            }
            Dialog::Rename { path, title } => {
                let mut go = false;
                egui::Window::new("Rename").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift)).show(
                    ctx,
                    |ui| {
                        ui.label(crate::i18n::tr("Title (the folder name does not change)"));
                        let r = ui.text_edit_singleline(title);
                        // Read Enter before asking for focus again: a new focus request hides the lost focus.
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if !r.has_focus() {
                            // Once: asking again every frame restarts the on-screen keyboard every frame.
                            r.request_focus();
                        }
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                            if (modal_button(ui, &self.tokens, "Rename", true).clicked() || enter) && !title.trim().is_empty() {
                                go = true;
                            }
                        });
                    },
                );
                if go {
                    if self.run("node.title", &params(&[("path", &path_str(path)), ("title", title.trim())])).is_some() {
                        if self.editor.as_ref().map(|e| e.dir == *path).unwrap_or(false) {
                            self.open_page(path);
                        }
                        self.say("Renamed");
                    }
                    keep = false;
                }
            }
            Dialog::DeleteFile { page, name } => {
                let mut go = false;
                let mut cancel = false;
                modal_backdrop(ctx, "delete-file-modal-backdrop");
                egui::Window::new("Delete file")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, "Delete file");
                        ui.set_min_width(420.0);
                        ui.label(format!("Delete \"assets/{name}\" from this page's folder?"));
                        ui.add_space(6.0);
                        ui.colored_label(self.tokens.warn, crate::i18n::tr("The file is removed from disk. Undo does not bring a file back."));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, "Delete", true).clicked() {
                                go = true;
                            }
                        });
                    });
                if ctx.input(|input| input.key_pressed(Key::Escape)) {
                    cancel = true;
                } else if ctx.input(|input| input.key_pressed(Key::Enter)) {
                    go = true;
                }
                if cancel {
                    keep = false;
                }
                if go {
                    let page = page.clone();
                    if self.run("page.detach", &params(&[("path", &path_str(&page)), ("name", name)])).is_some() {
                        if let Some(ed) = self.editor.as_mut() {
                            ed.attachments = notecraft_store::assets::list(&page);
                        }
                        self.say("The file is gone");
                        // The page may have named the drawing it no longer has.
                        self.sync_ink_reference();
                    }
                    keep = false;
                }
            }
            Dialog::DeleteNotebook { path, title } => {
                let mut go = false;
                egui::Window::new("Delete notebook")
                    .collapsible(false)
                    .resizable(false)
                    .title_bar(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(440.0);
                        modal_title(ui, &self.tokens, "Delete notebook");
                        ui.label(crate::i18n::tr_dyn(&format!("Delete notebook \"{title}\" and everything inside it?")));
                        ui.add_space(4.0);
                        ui.colored_label(self.tokens.warn, crate::i18n::tr("This removes the notebook folder from disk and cannot be undone."));
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, "Delete notebook", true).clicked() {
                                go = true;
                            }
                        });
                    });
                if go {
                    let path = path.clone();
                    let was_open = self.editor.as_ref().and_then(|editor| notebook_for(&editor.dir)).is_some_and(|notebook| notebook == path);
                    log::info!("DeleteNotebook dialog: deleting {:?}", path);
                    match notecraft_store::ops::remove_tree(&path) {
                        Ok(()) => {
                            log::info!("notebook deleted successfully: {:?}", path);
                            self.remove_recent_notebook(&path);
                            if was_open {
                                self.editor = None;
                                self.pad = None;
                                self.selected = None;
                                self.backstage = Some("Open");
                            }
                            let _ = self.run("tree.refresh", &Params::new());
                            self.say(format!("Deleted notebook {}", self.shown(&path)));
                        }
                        Err(error) => {
                            log::error!("notebook delete failed: {:?}: {}", path, error);
                            self.alert("delete-failed", "Delete notebook", &format!("The notebook could not be deleted: {error}"));
                        }
                    }
                    keep = false;
                }
            }
            Dialog::Delete { path, title, count } => {
                let mut go = false;
                let is_notebook = notecraft_store::tree::read_node(path).is_some_and(|node| node.kind == NodeKind::Notebook);
                egui::Window::new("Delete")
                    .collapsible(false)
                    .resizable(false)
                    .title_bar(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(420.0);
                        modal_title(ui, &self.tokens, "Delete notebook");
                        if is_notebook {
                            ui.label(crate::i18n::tr_dyn(&format!("Delete notebook \"{title}\" and everything inside it?")));
                            ui.colored_label(self.tokens.warn, crate::i18n::tr("This removes the notebook folder from disk and cannot be undone."));
                        } else {
                            ui.label(crate::i18n::tr_dyn(&format!(
                                "Move \"{title}\" and everything inside it ({count} item{}) to this notebook's recycle bin?",
                                if *count == 1 { "" } else { "s" }
                            )));
                            ui.label(crate::i18n::tr("You can restore it from History."));
                        }
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, if is_notebook { "Delete notebook" } else { "Move to recycle bin" }, true).clicked() {
                                go = true;
                            }
                        });
                    });
                if go {
                    if is_notebook {
                        let path = path.clone();
                        log::info!("deleting notebook: {:?}", path);
                        match notecraft_store::ops::remove_tree(&path) {
                            Ok(()) => {
                                log::info!("notebook deleted successfully: {:?}", path);
                                self.remove_recent_notebook(&path);
                                self.editor = None;
                                self.pad = None;
                                self.selected = None;
                                self.open.remove(&path);
                                let _ = self.run("tree.refresh", &Params::new());
                                self.say("Deleted notebook");
                            }
                            Err(error) => {
                                log::error!("notebook delete failed: {:?}: {}", path, error);
                                self.alert("delete-failed", "Delete notebook", &format!("The notebook could not be deleted: {error}"));
                            }
                        }
                    } else {
                        let was_open = self.editor.as_ref().map(|e| e.dir.starts_with(&*path)).unwrap_or(false);
                        log::info!("deleting node: {:?}", path);
                        match self.run("node.delete", &params(&[("path", &path_str(path))])) {
                            Some(_) => {
                                log::info!("node deleted successfully: {:?}", path);
                                if was_open {
                                    self.editor = None;
                                }
                                self.selected = None;
                                self.say("Moved to the notebook recycle bin");
                            }
                            None => {
                                log::error!("node.delete command failed: {:?}", path);
                                self.alert("delete-failed", "Delete", "This could not be moved to the recycle bin. Nothing was deleted.");
                            }
                        }
                    }
                    keep = false;
                }
            }
            Dialog::PageVersions { page, versions } => {
                let mut restore: Option<String> = None;
                egui::Window::new("Page versions")
                    .collapsible(false)
                    .resizable(true)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .show(ctx, |ui| {
                        ui.set_min_width(420.0);
                        ui.label(crate::i18n::tr("Saved copies of this page"));
                        ui.add_space(6.0);
                        if versions.is_empty() {
                            ui.label(crate::i18n::tr("No saved versions yet. NoteCraft keeps a copy when the page changes."));
                        } else {
                            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                for version in versions.iter() {
                                    ui.horizontal(|ui| {
                                        ui.label(&version.created);
                                        ui.label(
                                            egui::RichText::new(format!("{} KB", version.bytes.div_ceil(1024))).small().color(self.tokens.muted),
                                        );
                                        if modal_button(ui, &self.tokens, "Restore", true).clicked() {
                                            restore = Some(version.id.clone());
                                        }
                                    });
                                }
                            });
                        }
                        ui.add_space(8.0);
                        if modal_button(ui, &self.tokens, "Close", false).clicked() {
                            keep = false;
                        }
                    });
                if let Some(id) = restore {
                    if self.run("page.version.restore", &params(&[("path", &path_str(page)), ("id", &id)])).is_some() {
                        self.open_page(page);
                        self.say("Restored the saved page version");
                    }
                    keep = false;
                }
            }
            Dialog::RecycleBin { notebook, items } => {
                let mut restore: Option<String> = None;
                let mut empty = false;
                let mut purge: Option<(String, String)> = None;
                egui::Window::new("Notebook recycle bin")
                    .collapsible(false)
                    .resizable(true)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .show(ctx, |ui| {
                        ui.set_min_width(460.0);
                        if items.is_empty() {
                            ui.label(crate::i18n::tr("This notebook's recycle bin is empty."));
                        } else {
                            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                                for item in items.iter() {
                                    ui.horizontal(|ui| {
                                        ui.label(&item.title);
                                        ui.label(
                                            egui::RichText::new(format!("{} item(s) · {}", item.count, item.deleted))
                                                .small()
                                                .color(self.tokens.muted),
                                        );
                                        if modal_button(ui, &self.tokens, "Restore", true).clicked() {
                                            restore = Some(item.id.clone());
                                        }
                                        if modal_button(ui, &self.tokens, "Delete permanently", true).clicked() {
                                            purge = Some((item.id.clone(), item.title.clone()));
                                        }
                                    });
                                }
                            });
                        }
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Close", false).clicked() {
                                keep = false;
                            }
                            if !items.is_empty() && modal_button(ui, &self.tokens, "Empty the bin", true).clicked() {
                                empty = true;
                            }
                        });
                    });
                if empty {
                    next = Dialog::EmptyRecycleBin { notebook: notebook.clone(), count: items.len() };
                } else if let Some(id) = restore {
                    if self.run("recycle.restore", &params(&[("notebook", &path_str(notebook)), ("id", &id)])).is_some() {
                        self.say("Restored to the original location");
                        keep = false;
                    }
                } else if let Some((id, title)) = purge {
                    next = Dialog::PurgeRecycleItem { notebook: notebook.clone(), id, title };
                }
            }
            Dialog::EmptyRecycleBin { notebook, count } => {
                let mut go = false;
                egui::Window::new("Empty the bin")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .show(ctx, |ui| {
                        ui.label(crate::i18n::tr_dyn(&format!("Permanently delete all {count} items in the recycle bin?")));
                        ui.colored_label(self.tokens.warn, crate::i18n::tr("This cannot be undone."));
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, "Delete permanently", true).clicked() {
                                go = true;
                            }
                        });
                    });
                if go && self.run("recycle.empty", &params(&[("notebook", &path_str(notebook))])).is_some() {
                    self.say("Emptied the recycle bin");
                    keep = false;
                }
            }
            Dialog::PurgeRecycleItem { notebook, id, title } => {
                let mut purge = false;
                egui::Window::new("Permanently delete?")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, self.dialog_lift))
                    .show(ctx, |ui| {
                        ui.label(crate::i18n::tr_dyn(&format!("Permanently delete \"{title}\" and everything inside it?")));
                        ui.colored_label(self.tokens.warn, crate::i18n::tr("This cannot be undone."));
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, "Cancel", false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, "Delete permanently", true).clicked() {
                                purge = true;
                            }
                        });
                    });
                if purge && self.run("recycle.purge", &params(&[("notebook", &path_str(notebook)), ("id", id)])).is_some() {
                    self.say("Permanently deleted");
                    keep = false;
                }
            }
            Dialog::Symbols => match crate::symboldialog::show(ctx, &self.tokens, self.dialog_lift) {
                crate::symboldialog::Outcome::Open => {}
                crate::symboldialog::Outcome::Close => {
                    crate::symboldialog::reset(ctx);
                    keep = false;
                }
                crate::symboldialog::Outcome::Insert(symbol) => {
                    crate::symboldialog::reset(ctx);
                    self.insert_in_page(&symbol);
                    keep = false;
                }
            },
        }
        if keep {
            self.dialog = Some(next);
        }
        if let Some((purpose, path)) = chosen_path {
            self.finish_pick(purpose, path);
        }
    }
}
