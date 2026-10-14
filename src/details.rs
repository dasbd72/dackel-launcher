//! The per-game details page and the exe chooser list it shares with the grid.

use crate::covers::CoverView;
use crate::focus::FocusFrame;
use crate::icons::{CHIP_PAD_Y, Icon, chip, icon_text_button};
use dlsite_launcher::browse::format_play_time;
use dlsite_launcher::extract::{Progress, Status};
use dlsite_launcher::info::GameInfo;
use dlsite_launcher::nav::Control;
use dlsite_launcher::profile::{self, Stats};
use dlsite_launcher::tuning::{format_env, parse_env, split_args, winetricks_command};
use eframe::egui;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const COVER_WIDTH: f32 = 420.0;

/// The label of an info row. Rows are top-aligned; `pad_top` lines the label's text up
/// with a value that sits inside a padded chip.
fn row_label(ui: &mut egui::Ui, text: &str, pad_top: f32) {
    ui.vertical(|ui| {
        ui.add_space(pad_top);
        // Never wrap: "Play time" and "Choose exe" must stay on one line.
        ui.add(
            egui::Label::new(egui::RichText::new(text).weak())
                .wrap_mode(egui::TextWrapMode::Extend),
        );
    });
}

/// The "Launch options" form of the open details page: what is typed, before it is saved.
pub struct TuningEdit {
    /// `Game::key` of the game the text belongs to.
    pub key: String,
    env_text: String,
    args: String,
    error: Option<String>,
    confirm_reset: bool,
}

impl TuningEdit {
    pub fn load(key: String, game_dir: &Path) -> Self {
        let tuning = profile::read_tuning(game_dir).unwrap_or_default();
        TuningEdit {
            key,
            env_text: format_env(&tuning.env),
            args: tuning.args,
            error: None,
            confirm_reset: false,
        }
    }
}

pub struct DetailsView<'a> {
    pub id: &'a str,
    /// Which library the game is in; only given when there is more than one.
    pub library: Option<String>,
    pub info: Option<&'a GameInfo>,
    pub cover: Option<PathBuf>,
    pub cover_view: CoverView,
    pub stats: Stats,
    pub exe: Option<&'a Path>,
    pub candidates: &'a [PathBuf],
    pub extract: &'a Result<Status, String>,
    pub error: Option<&'a str>,
    pub busy: bool,
    pub running: bool,
    /// Set while this game is being extracted in the background.
    pub extracting: Option<Progress>,
    /// The game's folder, for the winetricks command.
    pub dir: &'a Path,
    /// The Proton version pinned on the first launch.
    pub proton_pin: Option<&'a str>,
}

#[derive(Default)]
pub struct DetailsAction {
    pub back: bool,
    pub reload: bool,
    pub extract: bool,
    pub cancel_extract: bool,
    pub toggle_favorite: bool,
    pub launch: bool,
    pub choose_exe: Option<PathBuf>,
    /// Env and arguments to save.
    pub save_tuning: Option<(BTreeMap<String, String>, String)>,
    pub unpin_proton: bool,
    pub reset_prefix: bool,
}

/// Lists exe candidates as buttons; returns the one clicked, if any.
/// `selected` marks the entry a controller cursor is on in a modal list. With a `frame`
/// the buttons are focusable themselves (the details page).
pub fn exe_choices(
    ui: &mut egui::Ui,
    candidates: &[PathBuf],
    selected: Option<usize>,
    mut frame: Option<&mut FocusFrame>,
) -> Option<PathBuf> {
    let mut chosen = None;
    for (i, candidate) in candidates.iter().enumerate() {
        let button =
            egui::Button::new(candidate.display().to_string()).selected(selected == Some(i));
        let response = ui.add(button);
        let pressed = match frame.as_deref_mut() {
            Some(frame) => frame.pressed(ui, Control::ExeChoice(i), &response, true),
            None => response.clicked(),
        };
        if pressed {
            chosen = Some(candidate.clone());
        }
    }
    chosen
}

pub fn details_ui(
    ui: &mut egui::Ui,
    view: &DetailsView,
    frame: &mut FocusFrame,
    edit: &mut TuningEdit,
    scroll: &mut f32,
) -> DetailsAction {
    let mut action = DetailsAction::default();
    let title = view.info.map_or(view.id, |info| info.title.as_str());

    ui.horizontal(|ui| {
        let response = ui.button("Back");
        if frame.pressed(ui, Control::Back, &response, true) {
            action.back = true;
        }
        ui.heading(title);
        // Without scraped info the heading already is the RJ code.
        if title != view.id {
            ui.label(egui::RichText::new(view.id).monospace().weak());
        }
    });
    ui.separator();

    // Everything below the header scrolls as one page, so a tall form never hides the rest.
    // The offset lives with the caller, so the left stick can scroll the page too.
    let scrolled = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .vertical_scroll_offset(*scroll)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // The cover takes the height its own aspect ratio gives it: no empty bands.
                match (&view.cover_view, view.cover.as_ref()) {
                    // Blur is on: only ever the blurred copy, or an empty box while it is made.
                    (CoverView::Blurred(texture), _) => {
                        let aspect = texture
                            .as_ref()
                            .map_or(0.75, |t| t.size_vec2().y / t.size_vec2().x);
                        let size = egui::vec2(COVER_WIDTH, COVER_WIDTH * aspect);
                        match texture {
                            Some(texture) => {
                                ui.add(
                                    egui::Image::new(texture)
                                        .fit_to_exact_size(size)
                                        .corner_radius(6.0),
                                );
                            }
                            None => {
                                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                ui.painter()
                                    .rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
                            }
                        }
                    }
                    (CoverView::Normal, Some(path)) => {
                        let uri = format!("file://{}", path.display());
                        ui.add(
                            egui::Image::new(uri)
                                .max_width(COVER_WIDTH)
                                .corner_radius(6.0),
                        );
                    }
                    (CoverView::Normal, None) => {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(COVER_WIDTH, COVER_WIDTH * 0.75),
                            egui::Sense::hover(),
                        );
                        ui.painter()
                            .rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
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
                    // Two columns, so every label lines up and so does every value.
                    egui::Grid::new("details-info")
                        .num_columns(2)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            let chip_pad = f32::from(CHIP_PAD_Y);

                            if let Some(library) = &view.library {
                                row_label(ui, "Library", 0.0);
                                ui.label(library);
                                ui.end_row();
                            }

                            row_label(ui, "Circle", chip_pad);
                            match view.info.and_then(|i| i.circle.as_deref()) {
                                Some(circle) => chip(ui, circle),
                                None => {
                                    ui.weak("--");
                                }
                            }
                            ui.end_row();

                            row_label(ui, "Tags", chip_pad);
                            ui.horizontal_wrapped(|ui| {
                                let genres = view.info.map(|i| i.genres.as_slice()).unwrap_or(&[]);
                                if genres.is_empty() {
                                    ui.weak("--");
                                }
                                for genre in genres {
                                    chip(ui, genre);
                                }
                            });
                            ui.end_row();

                            row_label(ui, "Play time", 0.0);
                            ui.label(format_play_time(view.stats.play_time_secs));
                            ui.end_row();

                            row_label(ui, "Exe", 0.0);
                            match view.exe {
                                Some(exe) => {
                                    ui.label(exe.display().to_string());
                                }
                                None => {
                                    ui.weak("not chosen");
                                }
                            }
                            ui.end_row();

                            if !view.candidates.is_empty() {
                                row_label(ui, "Choose exe", 0.0);
                                ui.vertical(|ui| {
                                    action.choose_exe =
                                        exe_choices(ui, view.candidates, None, Some(&mut *frame));
                                });
                                ui.end_row();
                            }
                        });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if let Some(progress) = view.extracting {
                            let pct = progress.fraction() * 100.0;
                            ui.add(
                                egui::ProgressBar::new(progress.fraction())
                                    .desired_width(340.0)
                                    .desired_height(crate::icons::BUTTON_HEIGHT)
                                    .text(format!(
                                        "Extracting {pct:.0}%  ({} / {} files)",
                                        progress.files_done, progress.files_total
                                    )),
                            );
                            let response = icon_text_button(ui, Icon::Close, "Cancel", true);
                            if frame.pressed(ui, Control::CancelExtract, &response, true) {
                                action.cancel_extract = true;
                            }
                        } else {
                            if view.running {
                                ui.colored_label(egui::Color32::LIGHT_GREEN, "Running");
                            } else {
                                let ready = view.exe.is_some();
                                let response = icon_text_button(ui, Icon::Play, "Launch", ready);
                                if frame.pressed(ui, Control::Launch, &response, ready) {
                                    action.launch = true;
                                }
                            }
                            let label = if view.busy {
                                "Reloading..."
                            } else {
                                "Reload info"
                            };
                            let response = ui.add_enabled(!view.busy, egui::Button::new(label));
                            if frame.pressed(ui, Control::ReloadInfo, &response, !view.busy) {
                                action.reload = true;
                            }
                            match view.extract {
                                Err(msg) => {
                                    ui.colored_label(egui::Color32::LIGHT_RED, msg);
                                }
                                Ok(Status::NotExtracted) | Ok(Status::Ready { has_zip: true }) => {
                                    let label = if matches!(view.extract, Ok(Status::NotExtracted))
                                    {
                                        "Extract"
                                    } else {
                                        "Re-extract"
                                    };
                                    let response = ui.button(label);
                                    if frame.pressed(ui, Control::Extract, &response, true) {
                                        action.extract = true;
                                    }
                                }
                                Ok(Status::ExtractManually(_)) => {
                                    ui.colored_label(egui::Color32::YELLOW, "extract manually");
                                }
                                Ok(Status::Ready { has_zip: false }) => {}
                            }
                        }
                        let favorite = view.stats.favorite;
                        let label = if favorite { "Unfavorite" } else { "Favorite" };
                        let response =
                            icon_text_button(ui, Icon::Heart { filled: favorite }, label, true);
                        if frame.pressed(ui, Control::Favorite, &response, true) {
                            action.toggle_favorite = true;
                        }
                    });
                    if let Some(err) = view.error {
                        ui.colored_label(egui::Color32::LIGHT_RED, err);
                    }
                    launch_options(ui, view, edit, &mut action);
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
                    // A box as wide as the window; the text wraps to it.
                    egui::Frame::new()
                        .fill(ui.visuals().extreme_bg_color)
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(&info.description);
                        });
                }
            }
        });
    *scroll = scrolled.state.offset.y;
    action
}

/// Per-game env and arguments, the pinned Proton, the winetricks command and Prefix reset.
fn launch_options(
    ui: &mut egui::Ui,
    view: &DetailsView,
    edit: &mut TuningEdit,
    action: &mut DetailsAction,
) {
    egui::CollapsingHeader::new("Launch options").show(ui, |ui| {
        ui.weak("Environment variables, one KEY=value per line");
        ui.add(
            egui::TextEdit::multiline(&mut edit.env_text)
                .font(egui::TextStyle::Monospace)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        ui.weak("Extra arguments");
        ui.add(egui::TextEdit::singleline(&mut edit.args).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                let checked = parse_env(&edit.env_text).and_then(|env| {
                    if env.contains_key("WINEPREFIX") {
                        return Err(
                            "WINEPREFIX is managed by the launcher (see Reset prefix)".to_owned()
                        );
                    }
                    split_args(&edit.args).map_err(|e| format!("Extra arguments: {e}"))?;
                    Ok(env)
                });
                match checked {
                    Ok(env) => {
                        edit.error = None;
                        action.save_tuning = Some((env, edit.args.clone()));
                    }
                    Err(err) => edit.error = Some(err),
                }
            }
            if let Some(err) = &edit.error {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }
        });

        ui.separator();
        ui.horizontal(|ui| match view.proton_pin {
            Some(pin) => {
                ui.label(format!("Proton pinned to {pin}"));
                if ui.button("Unpin").clicked() {
                    action.unpin_proton = true;
                }
            }
            None => {
                ui.weak("Proton: pinned on the first launch");
            }
        });

        ui.separator();
        ui.weak("Winetricks for this game's prefix");
        let command = winetricks_command(view.dir);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&command).monospace());
            if ui.button("Copy").clicked() {
                ui.ctx().copy_text(command.clone());
            }
        });

        ui.separator();
        if edit.confirm_reset {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Delete this game's Wine prefix? Anything stored inside it, such as installed \
                 components or saves, is lost. The game's own files are kept.",
            );
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!view.running, egui::Button::new("Delete prefix"))
                    .clicked()
                {
                    action.reset_prefix = true;
                    edit.confirm_reset = false;
                }
                if ui.button("Keep it").clicked() {
                    edit.confirm_reset = false;
                }
            });
            if view.running {
                ui.weak("Close the game first.");
            }
        } else if ui.button("Reset prefix...").clicked() {
            edit.confirm_reset = true;
        }
    });
}
