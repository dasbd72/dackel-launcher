//! The "Add zip" flow: the archive picker, the add dialog that names each game folder and
//! picks the library, and the one conflict prompt. The moving is `dackel_launcher::import`.

use dackel_launcher::import::{self, Conflict, Outcome, Policy, Stat};
use dackel_launcher::library::browser_start;
use eframe::egui;
use egui_file_dialog::{FileDialog, Filter, OpeningMode};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ARCHIVES: &str = "Archives (zip, tar.gz, 7z, rar)";

/// The archive picker: many files at once, starting in Downloads, or home when there is no
/// Downloads folder.
pub fn picker(home: Option<&Path>) -> FileDialog {
    let downloads = home
        .map(|home| home.join("Downloads"))
        .filter(|d| d.is_dir());
    FileDialog::new()
        .title("Add archives")
        .opening_mode(OpeningMode::AlwaysInitialDir)
        .initial_directory(downloads.unwrap_or_else(|| browser_start(home)))
        .add_file_filter(ARCHIVES, Filter::new(|p: &Path| import::is_archive(p)))
        .default_file_filter(ARCHIVES)
        .show_new_folder_button(false)
}

/// One archive in the add dialog: where it is, and the game folder it goes into.
struct Row {
    source: PathBuf,
    name: String,
}

/// The add dialog, open while the user names the folders.
pub struct AddZip {
    rows: Vec<Row>,
    /// Index into the libraries: the one the folders go into.
    library: usize,
    /// Set after Confirm when some archive would replace a file; the prompt lists them.
    conflicts: Option<Vec<Conflict>>,
}

/// How the dialog ends.
pub enum Done {
    Cancel,
    Import {
        library: PathBuf,
        items: Vec<(PathBuf, String)>,
        policy: Policy,
    },
}

impl AddZip {
    pub fn new(files: Vec<PathBuf>) -> Self {
        let rows = files
            .into_iter()
            .map(|source| {
                let name = source
                    .file_name()
                    .map(|n| import::default_folder_name(&n.to_string_lossy()))
                    .unwrap_or_default();
                Row { source, name }
            })
            .collect();
        AddZip {
            rows,
            library: 0,
            conflicts: None,
        }
    }

    /// Draws the dialog, and its conflict prompt when one is up. Some when it is finished.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        libraries: &[PathBuf],
        labels: &[String],
    ) -> Option<Done> {
        let mut done = None;
        egui::Window::new("Add zip")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(520.0_f32.min(ctx.content_rect().width() - 64.0));
                self.contents(ui, libraries, labels, &mut done);
            });
        if self.conflicts.is_some() {
            self.conflict_prompt(ctx, libraries, &mut done);
        }
        done
    }

    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        libraries: &[PathBuf],
        labels: &[String],
        done: &mut Option<Done>,
    ) {
        if libraries.len() > 1 {
            ui.horizontal(|ui| {
                ui.label("Library:");
                egui::ComboBox::from_id_salt("add-zip-library")
                    .selected_text(labels.get(self.library).map_or("", String::as_str))
                    .show_ui(ui, |ui| {
                        for (i, label) in labels.iter().enumerate() {
                            ui.selectable_value(&mut self.library, i, label);
                        }
                    });
            });
        } else if let Some(root) = libraries.first() {
            ui.label(format!("Library: {}", root.display()));
        }
        ui.separator();
        ui.label("Game folder for each archive:");
        for row in &mut self.rows {
            let file = row
                .source
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            ui.label(file);
            ui.add(egui::TextEdit::singleline(&mut row.name).desired_width(f32::INFINITY));
            if let Err(err) = import::check_folder_name(row.name.trim()) {
                ui.colored_label(egui::Color32::LIGHT_RED, err.to_string());
            }
            ui.add_space(4.0);
        }
        let valid = self
            .rows
            .iter()
            .all(|row| import::check_folder_name(row.name.trim()).is_ok());
        ui.separator();
        ui.horizontal(|ui| {
            let confirm = ui.add_enabled(valid, egui::Button::new("Confirm"));
            if confirm.clicked()
                && let Some(root) = libraries.get(self.library)
            {
                let items = self.items();
                let conflicts = import::find_conflicts(root, &items);
                if conflicts.is_empty() {
                    *done = Some(Done::Import {
                        library: root.clone(),
                        items,
                        policy: Policy::Skip,
                    });
                } else {
                    self.conflicts = Some(conflicts);
                }
            }
            if ui.button("Cancel").clicked() {
                *done = Some(Done::Cancel);
            }
        });
    }

    /// The (source, folder name) pairs for the import, names trimmed.
    fn items(&self) -> Vec<(PathBuf, String)> {
        self.rows
            .iter()
            .map(|row| (row.source.clone(), row.name.trim().to_owned()))
            .collect()
    }

    /// Lists every archive that would replace a file, with both copies' sizes and times.
    fn conflict_prompt(
        &mut self,
        ctx: &egui::Context,
        libraries: &[PathBuf],
        done: &mut Option<Done>,
    ) {
        let Some(conflicts) = &self.conflicts else {
            return;
        };
        let mut answer = None;
        let mut cancel = false;
        egui::Window::new("Replace files?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(520.0_f32.min(ctx.content_rect().width() - 64.0));
                ui.label(format!(
                    "{} file{} would replace an existing file of the same name:",
                    conflicts.len(),
                    if conflicts.len() == 1 { "" } else { "s" }
                ));
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for conflict in conflicts {
                            ui.strong(conflict.dest.display().to_string());
                            ui.label(format!("  Existing: {}", describe(&conflict.existing)));
                            ui.label(format!("  New: {}", describe(&conflict.incoming)));
                            ui.add_space(4.0);
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button("Overwrite all").clicked() {
                        answer = Some(Policy::Overwrite);
                    }
                    if ui.button("Skip those").clicked() {
                        answer = Some(Policy::Skip);
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            self.conflicts = None;
        }
        if let (Some(policy), Some(root)) = (answer, libraries.get(self.library)) {
            *done = Some(Done::Import {
                library: root.clone(),
                items: self.items(),
                policy,
            });
        }
    }
}

/// One line for the per-file results: where a file went, or why it did not.
pub fn result_text(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Moved {
            dest,
            replaced: false,
        } => format!("moved to {}", dest.display()),
        Outcome::Moved {
            dest,
            replaced: true,
        } => format!("replaced {}", dest.display()),
        Outcome::Skipped => "skipped: a file with that name is already there".to_owned(),
        Outcome::Failed(err) => format!("failed: {err}"),
    }
}

fn describe(stat: &Stat) -> String {
    let when = stat
        .modified
        .map_or_else(|| "time unknown".to_owned(), utc_stamp);
    format!("{}, modified {when}", size_text(stat.size))
}

fn size_text(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let size = bytes as f64;
    if size >= KB * KB * KB {
        format!("{:.1} GB", size / (KB * KB * KB))
    } else if size >= KB * KB {
        format!("{:.1} MB", size / (KB * KB))
    } else {
        format!("{:.0} KB", (size / KB).ceil())
    }
}

/// The time as "YYYY-MM-DD HH:MM UTC". The standard library has no local time zone.
fn utc_stamp(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rest / 3600,
        rest % 3600 / 60
    )
}
