//! The per-game details page and the exe chooser list it shares with the grid.

use dlsite_launcher::browse::format_play_time;
use dlsite_launcher::extract::Status;
use dlsite_launcher::info::GameInfo;
use dlsite_launcher::profile::Stats;
use eframe::egui;
use std::path::{Path, PathBuf};

pub struct DetailsView<'a> {
    pub id: &'a str,
    pub info: Option<&'a GameInfo>,
    pub cover: Option<PathBuf>,
    pub stats: Stats,
    pub exe: Option<&'a Path>,
    pub candidates: &'a [PathBuf],
    pub extract: &'a Result<Status, String>,
    pub error: Option<&'a str>,
    pub busy: bool,
    pub running: bool,
}

#[derive(Default)]
pub struct DetailsAction {
    pub back: bool,
    pub reload: bool,
    pub extract: bool,
    pub toggle_favorite: bool,
    pub launch: bool,
    pub choose_exe: Option<PathBuf>,
}

/// Lists exe candidates as buttons; returns the one clicked, if any.
pub fn exe_choices(ui: &mut egui::Ui, candidates: &[PathBuf]) -> Option<PathBuf> {
    let mut chosen = None;
    for candidate in candidates {
        if ui.button(candidate.display().to_string()).clicked() {
            chosen = Some(candidate.clone());
        }
    }
    chosen
}

pub fn details_ui(ui: &mut egui::Ui, view: &DetailsView) -> DetailsAction {
    let mut action = DetailsAction::default();
    let title = view.info.map_or(view.id, |info| info.title.as_str());

    ui.horizontal(|ui| {
        if ui.button("Back").clicked() {
            action.back = true;
        }
        ui.heading(title);
    });
    ui.separator();

    ui.horizontal_top(|ui| {
        let size = egui::vec2(240.0, 340.0);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
        match view.cover.as_ref().filter(|p| p.exists()) {
            Some(path) => {
                let uri = format!("file://{}", path.display());
                ui.put(rect, egui::Image::new(uri).max_size(size));
            }
            None => {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No cover",
                    egui::FontId::proportional(16.0),
                    ui.visuals().weak_text_color(),
                );
            }
        }

        ui.vertical(|ui| {
            ui.label(format!(
                "Circle: {}",
                view.info
                    .and_then(|info| info.circle.as_deref())
                    .unwrap_or("--")
            ));
            ui.horizontal_wrapped(|ui| {
                ui.label("Tags:");
                let genres = view.info.map(|info| info.genres.as_slice()).unwrap_or(&[]);
                if genres.is_empty() {
                    ui.weak("--");
                }
                for genre in genres {
                    ui.label(format!("[{genre}]"));
                }
            });
            ui.label(format!(
                "Play time: {}",
                format_play_time(view.stats.play_time_secs)
            ));

            let star = if view.stats.favorite {
                "Unfavorite"
            } else {
                "Favorite"
            };
            if ui.button(star).clicked() {
                action.toggle_favorite = true;
            }

            ui.separator();
            match view.exe {
                Some(exe) => ui.label(format!("Exe: {}", exe.display())),
                None => ui.label("Exe: not chosen"),
            };
            if !view.candidates.is_empty() {
                ui.label("Choose exe:");
                action.choose_exe = exe_choices(ui, view.candidates);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if view.running {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "Running");
                } else if ui
                    .add_enabled(view.exe.is_some(), egui::Button::new("Launch"))
                    .clicked()
                {
                    action.launch = true;
                }
                let label = if view.busy {
                    "Reloading..."
                } else {
                    "Reload info"
                };
                if ui
                    .add_enabled(!view.busy, egui::Button::new(label))
                    .clicked()
                {
                    action.reload = true;
                }
                match view.extract {
                    Err(msg) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, msg);
                    }
                    Ok(Status::NotExtracted) => {
                        if ui.button("Extract").clicked() {
                            action.extract = true;
                        }
                    }
                    Ok(Status::ExtractManually(_)) => {
                        ui.colored_label(egui::Color32::YELLOW, "extract manually");
                    }
                    Ok(Status::Ready { has_zip: true }) => {
                        if ui.button("Re-extract").clicked() {
                            action.extract = true;
                        }
                    }
                    Ok(Status::Ready { has_zip: false }) => {}
                }
            });
            if let Some(err) = view.error {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }
        });
    });

    ui.separator();
    match view.info {
        None => {
            ui.label("No info yet. Press Reload info to fetch it from DLsite.");
        }
        Some(info) => {
            if !info.found {
                ui.weak("No DLsite page was found for this work; showing the folder name.");
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(&info.description);
            });
        }
    }
    action
}
