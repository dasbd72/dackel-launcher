//! The per-game details page and the exe chooser list it shares with the grid.

use crate::covers::CoverView;
use crate::focus::FocusFrame;
use crate::icons::{Icon, chip, icon_text_button};
use dackel_launcher::browse::format_play_time;
use dackel_launcher::extract::{Progress, Status};
use dackel_launcher::info::GameInfo;
use dackel_launcher::library::store_of;
use dackel_launcher::nav::Control;
use dackel_launcher::pickers::AUTOMATIC_PROTON;
use dackel_launcher::profile::{self, Stats};
use dackel_launcher::proton;
use dackel_launcher::steam_art::IconSource;
use dackel_launcher::tuning::{format_env, parse_env, split_args, winetricks_command};
use eframe::egui;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const COVER_WIDTH: f32 = 420.0;

/// The label of an info row. The grid centers each cell vertically, so the label is centered
/// too and lines up with its value, whether that is text or a chip.
fn row_label(ui: &mut egui::Ui, text: &str) {
    // Never wrap: "Play time" and "Choose exe" must stay on one line.
    ui.add(
        egui::Label::new(egui::RichText::new(text).weak()).wrap_mode(egui::TextWrapMode::Extend),
    );
}

/// The "Launch options" form of the open details page: what is typed, before it is saved.
pub struct TuningEdit {
    /// `Game::key` of the game the text belongs to.
    pub key: String,
    env_text: String,
    args: String,
    error: Option<String>,
    confirm_reset: bool,
    /// Proton versions installed here, for the per-game choice.
    pub proton_versions: Vec<String>,
    /// The Proton version chosen for this game; `None` is automatic.
    pub pin: Option<String>,
    /// Where the game's Steam icon comes from.
    pub steam_icon: IconSource,
    /// The section is unfolded.
    open: bool,
    /// A text box has keyboard focus, so the pad's B and up/down leave it instead of acting
    /// on the page.
    pub typing: bool,
    /// Leave the text box at the next draw.
    pub leave: bool,
    /// The control that should get focus once it is drawn (after the reset prompt changes).
    focus: Option<Control>,
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
            proton_versions: proton::selectable(&proton::compat_dir()).unwrap_or_default(),
            pin: tuning.proton,
            steam_icon: profile::read_steam_icon(game_dir).unwrap_or_default(),
            open: false,
            typing: false,
            leave: false,
            focus: None,
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
    /// Whether the game has an entry in Steam; `None` when Steam was not found.
    pub steam_entry: Option<bool>,
}

/// What the details page asks to do with the game's Steam entry.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SteamChange {
    Add,
    Remove,
    /// Write the cover into Steam again.
    Images,
    /// Switch the icon between the cover and the exe's, then write the images again.
    ToggleIcon,
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
    /// A Proton choice to save: `Some(None)` is Automatic.
    pub set_proton: Option<Option<String>>,
    /// Move controller focus here (the reset prompt came or went).
    pub focus: Option<Control>,
    pub reset_prefix: bool,
    pub steam: Option<SteamChange>,
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

    // Wraps, so a long title pushes the id and store onto the next line instead of past the edge.
    ui.horizontal_wrapped(|ui| {
        let response = ui.button("Back");
        if frame.pressed(ui, Control::Back, &response, true) {
            action.back = true;
        }
        ui.heading(title);
        // Without scraped info the heading already is the RJ code.
        if title != view.id {
            ui.label(egui::RichText::new(view.id).monospace().weak());
        }
        if let Some(store) = store_of(view.id) {
            ui.weak(store.label());
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
                            if let Some(library) = &view.library {
                                row_label(ui, "Library");
                                ui.label(library);
                                ui.end_row();
                            }

                            row_label(ui, "Circle");
                            match view.info.and_then(|i| i.circle.as_deref()) {
                                Some(circle) => chip(ui, circle),
                                None => {
                                    ui.weak("--");
                                }
                            }
                            ui.end_row();

                            row_label(ui, "Tags");
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

                            row_label(ui, "Play time");
                            ui.label(format_play_time(view.stats.play_time_secs));
                            ui.end_row();

                            row_label(ui, "Exe");
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
                                row_label(ui, "Choose exe");
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
                        if let Some(has_entry) = view.steam_entry {
                            let label = if has_entry {
                                "Remove from Steam"
                            } else {
                                "Add to Steam"
                            };
                            let response = ui.button(label);
                            if frame.pressed(ui, Control::SteamEntry, &response, true) {
                                action.steam = Some(if has_entry {
                                    SteamChange::Remove
                                } else {
                                    SteamChange::Add
                                });
                            }
                            if has_entry && view.cover.is_some() {
                                let response = ui.button("Update Steam image");
                                if frame.pressed(ui, Control::SteamImages, &response, true) {
                                    action.steam = Some(SteamChange::Images);
                                }
                                let label = match edit.steam_icon {
                                    IconSource::Cover => "Icon: cover",
                                    IconSource::Exe => "Icon: exe",
                                };
                                let response = ui
                                    .button(label)
                                    .on_hover_text("Where the Steam icon comes from");
                                if frame.pressed(ui, Control::SteamIconSource, &response, true) {
                                    action.steam = Some(SteamChange::ToggleIcon);
                                }
                            }
                        }
                    });
                    if let Some(err) = view.error {
                        ui.colored_label(egui::Color32::LIGHT_RED, err);
                    }
                    launch_options(ui, view, frame, edit, &mut action);
                });
            });

            ui.separator();
            match view.info {
                None => {
                    ui.label("No info yet. Press Reload info to fetch it.");
                }
                Some(info) => {
                    if !info.found && dackel_launcher::library::is_dlsite_id(&info.id) {
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

/// The Proton dropdown: Automatic or one installed version, saved as soon as it is picked.
/// A pin that is not installed here is still listed, marked as such, so it stays visible.
/// With the controller, A on it opens the same list as a pad popup.
fn proton_choice(
    ui: &mut egui::Ui,
    frame: &mut FocusFrame,
    pin: Option<&str>,
    installed: &[String],
    action: &mut DetailsAction,
) {
    let current = match pin {
        None => AUTOMATIC_PROTON.to_owned(),
        Some(pin) if installed.iter().any(|v| v == pin) => pin.to_owned(),
        Some(pin) => format!("{pin} (not installed)"),
    };
    // Label above, dropdown at the left edge, so up/down from Save reaches it.
    ui.label("Proton");
    {
        let combo = egui::ComboBox::from_id_salt("proton-choice")
            .selected_text(current)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(pin.is_none(), AUTOMATIC_PROTON)
                    .clicked()
                    && pin.is_some()
                {
                    action.set_proton = Some(None);
                }
                for version in installed {
                    let on = pin == Some(version.as_str());
                    if ui.selectable_label(on, version).clicked() && !on {
                        action.set_proton = Some(Some(version.clone()));
                    }
                }
            });
        frame.register(ui, Control::ProtonChoice, &combo.response);
    }
    if installed.is_empty() {
        ui.weak("No Proton found in compatibilitytools.d");
    }
}

/// Width of the Launch options text boxes.
const FORM_WIDTH: f32 = 300.0;

fn form_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(text).min_size(egui::vec2(0.0, crate::icons::BUTTON_HEIGHT)),
    )
}

/// A text box the controller can focus: A starts typing in it, and the page's B or
/// up/down leave it again (see `TuningEdit::leave`).
pub fn text_box(
    ui: &mut egui::Ui,
    frame: &mut FocusFrame,
    id: Control,
    edit: egui::TextEdit<'_>,
) -> egui::Response {
    let response = ui.add(edit);
    frame.register(ui, id, &response);
    if frame.activated(id) {
        response.request_focus();
    }
    response
}

/// Per-game env and arguments, the Proton choice, the winetricks command and Prefix reset.
/// Everything in it can be focused with the controller.
fn launch_options(
    ui: &mut egui::Ui,
    view: &DetailsView,
    frame: &mut FocusFrame,
    edit: &mut TuningEdit,
    action: &mut DetailsAction,
) {
    let shown = egui::CollapsingHeader::new("Launch options")
        .open(Some(edit.open))
        .show(ui, |ui| {
            ui.weak("Environment variables, one KEY=value per line");
            let env = text_box(
                ui,
                frame,
                Control::EnvBox,
                egui::TextEdit::multiline(&mut edit.env_text)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(3)
                    .desired_width(FORM_WIDTH),
            );
            ui.weak("Extra arguments");
            let args = text_box(
                ui,
                frame,
                Control::ArgsBox,
                egui::TextEdit::singleline(&mut edit.args).desired_width(FORM_WIDTH),
            );
            edit.typing = env.has_focus() || args.has_focus();
            if std::mem::take(&mut edit.leave) {
                env.surrender_focus();
                args.surrender_focus();
                edit.typing = false;
            }
            ui.horizontal(|ui| {
                let save = form_button(ui, "Save", true);
                if frame.pressed(ui, Control::SaveTuning, &save, true) {
                    let checked = parse_env(&edit.env_text).and_then(|env| {
                        if env.contains_key("WINEPREFIX") {
                            return Err("WINEPREFIX is managed by the launcher (see Reset prefix)"
                                .to_owned());
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
            proton_choice(
                ui,
                frame,
                edit.pin.as_deref(),
                &edit.proton_versions,
                action,
            );

            ui.separator();
            ui.weak("Winetricks for this game's prefix");
            let command = winetricks_command(view.dir);
            ui.label(egui::RichText::new(&command).monospace());
            let copy = form_button(ui, "Copy", true);
            if frame.pressed(ui, Control::CopyWinetricks, &copy, true) {
                ui.ctx().copy_text(command.clone());
            }

            ui.separator();
            if edit.confirm_reset {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Delete this game's Wine prefix? Anything stored inside it, such as \
                     installed components or saves, is lost. The game's own files are kept.",
                );
                ui.horizontal(|ui| {
                    // Focus starts on the safe answer.
                    if edit.focus.take() == Some(Control::KeepPrefix) {
                        action.focus = Some(Control::KeepPrefix);
                    }
                    let delete = ui.add_enabled(!view.running, egui::Button::new("Delete prefix"));
                    if frame.pressed(ui, Control::DeletePrefix, &delete, !view.running) {
                        action.reset_prefix = true;
                        edit.confirm_reset = false;
                        edit.focus = Some(Control::ResetPrefix);
                    }
                    let keep = form_button(ui, "Keep it", true);
                    if frame.pressed(ui, Control::KeepPrefix, &keep, true) {
                        edit.confirm_reset = false;
                        edit.focus = Some(Control::ResetPrefix);
                    }
                });
                if view.running {
                    ui.weak("Close the game first.");
                }
            } else {
                if edit.focus.take() == Some(Control::ResetPrefix) {
                    action.focus = Some(Control::ResetPrefix);
                }
                let reset = form_button(ui, "Reset prefix...", true);
                if frame.pressed(ui, Control::ResetPrefix, &reset, true) {
                    edit.confirm_reset = true;
                    edit.focus = Some(Control::KeepPrefix);
                }
            }
        });
    if frame.pressed(ui, Control::LaunchOptions, &shown.header_response, true) {
        edit.open = !edit.open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dackel_launcher::cursor::Target;
    use dackel_launcher::nav::{Dir, spatial_next};

    #[test]
    fn the_dpad_walks_the_launch_options_top_to_bottom_and_back() {
        let dir = std::env::temp_dir();
        let mut edit = TuningEdit::load("k".into(), &dir);
        edit.open = true;
        let extract: Result<Status, String> = Ok(Status::NotExtracted);
        let view = DetailsView {
            id: "RJ1",
            library: None,
            info: None,
            cover: None,
            cover_view: CoverView::Normal,
            stats: Stats::default(),
            exe: None,
            candidates: &[],
            extract: &extract,
            error: None,
            busy: false,
            running: false,
            extracting: None,
            dir: &dir,
            steam_entry: None,
        };
        let ctx = egui::Context::default();
        let mut items = Vec::new();
        for _ in 0..3 {
            let mut frame = FocusFrame::new(None, None, false);
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let mut action = DetailsAction::default();
                    launch_options(ui, &view, &mut frame, &mut edit, &mut action);
                },
            );
            out.textures_delta.clear();
            items = std::mem::take(&mut frame.items);
        }
        let rects: Vec<_> = items.iter().map(|i| crate::focus::bounds(i.rect)).collect();
        let at = |c: Control| {
            items
                .iter()
                .position(|i| i.target == Target::Control(c))
                .unwrap()
        };
        let chain = [
            Control::LaunchOptions,
            Control::EnvBox,
            Control::ArgsBox,
            Control::SaveTuning,
            Control::ProtonChoice,
            Control::CopyWinetricks,
            Control::ResetPrefix,
        ];
        for pair in chain.windows(2) {
            let down = spatial_next(&rects, at(pair[0]), Dir::Down);
            assert_eq!(
                items[down].target,
                Target::Control(pair[1]),
                "down from {:?}",
                pair[0]
            );
            let up = spatial_next(&rects, at(pair[1]), Dir::Up);
            assert_eq!(
                items[up].target,
                Target::Control(pair[0]),
                "up from {:?}",
                pair[1]
            );
        }
    }
}
