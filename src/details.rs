//! The per-game details page and the exe chooser list it shares with the grid.

use crate::covers::CoverView;
use crate::focus::FocusFrame;
use crate::icons::{Icon, chip, chip_rows, icon_text_button};
use dackel_launcher::browse::format_play_time;
use dackel_launcher::extract::{Progress, Status};
use dackel_launcher::info::GameInfo;
use dackel_launcher::library::store_of;
use dackel_launcher::nav::Control;
use dackel_launcher::pickers::AUTOMATIC_PROTON;
use dackel_launcher::profile::{self, Stats};
use dackel_launcher::proton;
use dackel_launcher::steam_art::IconSource;
use dackel_launcher::tuning::{format_env, parse_env, split_args};
use dackel_launcher::winetricks::{COMMON, parse_verbs, toggle_verb};
use eframe::egui;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const COVER_WIDTH: f32 = 420.0;

/// Below this much room the cover goes above the info instead of beside it.
const STACK_BELOW: f32 = 900.0;

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
    /// The game's folder.
    pub dir: PathBuf,
    env_text: String,
    args: String,
    error: Option<String>,
    /// The verbs typed (or picked) to install into the prefix.
    verbs_text: String,
    install_error: Option<String>,
    /// Verbs already installed into this game's prefix.
    pub installed: Vec<String>,
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
            dir: game_dir.to_path_buf(),
            env_text: format_env(&tuning.env),
            args: tuning.args,
            error: None,
            verbs_text: String::new(),
            install_error: None,
            installed: profile::read_winetricks(game_dir).unwrap_or_default(),
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

    /// Whether the form is kept while `open` is the game on the details page (`None` when no
    /// details page is open). Anything else leaves unsaved text behind, so it is dropped.
    pub fn keeps(&self, open: Option<&str>) -> bool {
        open == Some(self.key.as_str())
    }

    /// Re-reads the saved env and arguments after a save, so the boxes show what was written.
    /// Everything else on the form is left as it is.
    pub fn refresh(&mut self) {
        let tuning = profile::read_tuning(&self.dir).unwrap_or_default();
        self.env_text = format_env(&tuning.env);
        self.args = tuning.args;
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
    /// Winetricks verbs are being installed into this game's prefix.
    pub installing: bool,
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
    /// Winetricks verbs to install into the prefix.
    pub install: Option<Vec<String>>,
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

/// The cover, `width` points wide, at the height its own aspect ratio gives it.
fn draw_cover(ui: &mut egui::Ui, view: &DetailsView, width: f32) {
    match (&view.cover_view, view.cover.as_ref()) {
        // Blur is on: only ever the blurred copy, or an empty box while it is made.
        (CoverView::Blurred(texture), _) => {
            let aspect = texture
                .as_ref()
                .map_or(0.75, |t| t.size_vec2().y / t.size_vec2().x);
            let size = egui::vec2(width, width * aspect);
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
            ui.add(egui::Image::new(uri).max_width(width).corner_radius(6.0));
        }
        (CoverView::Normal, None) => {
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(width, width * 0.75), egui::Sense::hover());
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
            // Narrow windows stack the cover over the info, so nothing is pushed past the edge.
            let stacked = ui.available_width() < STACK_BELOW;
            let cover_width = if stacked {
                (ui.available_width() * 0.6).min(COVER_WIDTH)
            } else {
                COVER_WIDTH
            };
            let mut column = |ui: &mut egui::Ui| {
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
                        match view.info.map(|i| i.genres.as_slice()) {
                            Some(genres) if !genres.is_empty() => chip_rows(ui, genres),
                            _ => {
                                ui.weak("--");
                            }
                        }
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
                ui.horizontal_wrapped(|ui| {
                    wrapped_rows_fit_buttons(ui);
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
                                let label = if matches!(view.extract, Ok(Status::NotExtracted)) {
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
            };
            if stacked {
                draw_cover(ui, view, cover_width);
                column(ui);
            } else {
                ui.horizontal_top(|ui| {
                    draw_cover(ui, view, cover_width);
                    ui.vertical(|ui| column(ui));
                });
            }

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
            .width(form_width(ui))
            .truncate()
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

/// Rows that wrap start at the height of a button, so a row of buttons does not overlap the
/// next one.
fn wrapped_rows_fit_buttons(ui: &mut egui::Ui) {
    ui.spacing_mut().interact_size.y = crate::icons::BUTTON_HEIGHT;
}

/// Width of the Launch options text boxes on a wide window.
const FORM_WIDTH: f32 = 300.0;

/// A text box's width: the usual one, or whatever room is left on a narrow window.
fn form_width(ui: &egui::Ui) -> f32 {
    FORM_WIDTH.min(ui.available_width())
}

fn form_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(text).min_size(egui::vec2(0.0, crate::icons::BUTTON_HEIGHT)),
    )
}

/// Install winetricks verbs into the game's prefix: common ones as chips, anything else typed.
fn winetricks_section(
    ui: &mut egui::Ui,
    view: &DetailsView,
    frame: &mut FocusFrame,
    edit: &mut TuningEdit,
    action: &mut DetailsAction,
) -> egui::Response {
    ui.weak("Install into this game's prefix (winetricks)");
    ui.horizontal_wrapped(|ui| {
        wrapped_rows_fit_buttons(ui);
        for (i, choice) in COMMON.iter().enumerate() {
            let on = edit.verbs_text.split_whitespace().any(|w| w == choice.verb);
            let done = edit.installed.iter().any(|v| v == choice.verb);
            let label = if done {
                format!("{} (installed)", choice.verb)
            } else {
                choice.verb.to_owned()
            };
            let chip = ui
                .add(egui::Button::new(label).selected(on))
                .on_hover_text(choice.hint);
            if frame.pressed(ui, Control::WinetricksVerb(i), &chip, true) {
                edit.verbs_text = toggle_verb(&edit.verbs_text, choice.verb);
                edit.install_error = None;
            }
        }
    });
    let ready = !view.running && !view.installing && !edit.proton_versions.is_empty();
    let typed = ui
        .horizontal_wrapped(|ui| {
            wrapped_rows_fit_buttons(ui);
            let typed = text_box(
                ui,
                frame,
                Control::WinetricksBox,
                egui::TextEdit::singleline(&mut edit.verbs_text)
                    .hint_text("verbs, e.g. dotnet48 win7")
                    .desired_width(form_width(ui)),
            );
            let install = form_button(ui, "Install", ready);
            if frame.pressed(ui, Control::WinetricksInstall, &install, ready) {
                match parse_verbs(&edit.verbs_text) {
                    Ok(verbs) => {
                        edit.install_error = None;
                        action.install = Some(verbs);
                    }
                    Err(err) => edit.install_error = Some(err),
                }
            }
            typed
        })
        .inner;
    if let Some(err) = &edit.install_error {
        ui.colored_label(egui::Color32::LIGHT_RED, err);
    }
    if view.installing {
        ui.weak("Installing... this can take several minutes");
    } else if view.running {
        ui.weak("Close the game first.");
    }
    if !edit.installed.is_empty() {
        ui.weak(format!("Installed: {}", edit.installed.join(", ")));
    }
    typed
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

/// Per-game env and arguments, the Proton choice, winetricks installs and Prefix reset.
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
                    .desired_width(form_width(ui)),
            );
            ui.weak("Extra arguments");
            let args = text_box(
                ui,
                frame,
                Control::ArgsBox,
                egui::TextEdit::singleline(&mut edit.args).desired_width(form_width(ui)),
            );
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
            let verbs = winetricks_section(ui, view, frame, edit, action);
            edit.typing = env.has_focus() || args.has_focus() || verbs.has_focus();
            if std::mem::take(&mut edit.leave) {
                env.surrender_focus();
                args.surrender_focus();
                verbs.surrender_focus();
                edit.typing = false;
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
            installing: false,
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
            Control::WinetricksVerb(1),
            Control::WinetricksBox,
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
        let right = spatial_next(&rects, at(Control::WinetricksBox), Dir::Right);
        assert_eq!(
            items[right].target,
            Target::Control(Control::WinetricksInstall)
        );
        let left = spatial_next(&rects, at(Control::WinetricksInstall), Dir::Left);
        assert_eq!(items[left].target, Target::Control(Control::WinetricksBox));
    }

    /// The text of the whole page drawn `width` points wide with the Launch options open:
    /// where every piece of text landed, and the focusable controls.
    fn draw_page(
        width: f32,
        genres: usize,
        steam: bool,
    ) -> (Vec<(egui::Rect, String)>, Vec<crate::focus::Item>) {
        let dir = std::env::temp_dir();
        let mut edit = TuningEdit::load("k".into(), &dir);
        edit.open = true;
        edit.proton_versions = vec!["GE-Proton11-9-x86_64".to_owned()];
        // A long pinned name that is not installed makes the widest dropdown text.
        edit.pin = Some("GE-Proton11-9-x86_64-with-a-long-custom-suffix".to_owned());
        let extract: Result<Status, String> = Ok(Status::Ready { has_zip: true });
        let info = GameInfo {
            id: "RJ1".into(),
            title: "A fairly long title of a game".into(),
            circle: Some("Some Circle Name".into()),
            genres: (0..genres)
                .map(|i| format!("Genre number {i} long"))
                .collect(),
            description: "A description. ".repeat(40),
            cover: None,
            found: true,
        };
        let exe = PathBuf::from("Game/Game.exe");
        let view = DetailsView {
            id: "RJ1",
            library: Some("Main library".into()),
            info: Some(&info),
            cover: Some(PathBuf::from("/nonexistent/cover.jpg")),
            cover_view: CoverView::Normal,
            stats: Stats::default(),
            exe: Some(exe.as_path()),
            candidates: &[],
            extract: &extract,
            error: None,
            busy: false,
            running: false,
            extracting: None,
            installing: false,
            steam_entry: steam.then_some(true),
        };
        let ctx = egui::Context::default();
        let mut scroll = 0.0;
        let mut texts = Vec::new();
        let mut items = Vec::new();
        for _ in 0..3 {
            let mut frame = FocusFrame::new(None, None, false);
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 4000.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    details_ui(ui, &view, &mut frame, &mut edit, &mut scroll);
                },
            );
            out.textures_delta.clear();
            texts = out
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::Shape::Text(t) => Some((
                        t.galley.rect.translate(t.pos.to_vec2()),
                        t.galley.text().to_owned(),
                    )),
                    _ => None,
                })
                .collect();
            items = std::mem::take(&mut frame.items);
        }
        (texts, items)
    }

    /// Below this the header strip (Back, title) sits; the page proper starts under it.
    const HEADER_HEIGHT: f32 = 40.0;

    /// Every width the size presets give on the Deck's 1280-point-wide screen.
    fn preset_widths() -> Vec<f32> {
        let mut widths: Vec<f32> = dackel_launcher::window::SCALE_PRESETS
            .iter()
            .map(|size| (1280.0 / size).floor())
            .collect();
        // And the widths in between, where the layout switches.
        widths.extend((400..=1800).step_by(50).map(|w| w as f32));
        widths
    }

    #[test]
    fn nothing_on_the_page_runs_past_the_right_edge_at_any_size() {
        for width in preset_widths() {
            for steam in [false, true] {
                let (texts, items) = draw_page(width, 12, steam);
                for rect in texts
                    .iter()
                    .map(|(r, _)| r)
                    .chain(items.iter().map(|i| &i.rect))
                {
                    assert!(
                        rect.right() <= width + 0.5,
                        "{width} wide (steam {steam}): {rect:?} runs past the edge"
                    );
                }
            }
        }
    }

    #[test]
    fn no_two_pieces_of_text_overlap_at_any_size() {
        for width in preset_widths() {
            let (texts, _) = draw_page(width, 30, true);
            // The header's wrapped labels report positions that are not where they are drawn.
            let texts: Vec<_> = texts
                .into_iter()
                .filter(|(r, _)| r.top() > HEADER_HEIGHT)
                .collect();
            for (i, (a, _)) in texts.iter().enumerate() {
                for (b, _) in &texts[i + 1..] {
                    let overlap = a.intersect(*b);
                    assert!(
                        overlap.width() < 1.0 || overlap.height() < 1.0,
                        "{width} wide: {a:?} overlaps {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_tags_sit_right_under_the_circle_with_no_big_gap() {
        for width in preset_widths() {
            let (texts, _) = draw_page(width, 30, false);
            let find = |what: &str| {
                texts
                    .iter()
                    .find(|(_, text)| text.contains(what))
                    .unwrap_or_else(|| panic!("{width}: no text {what}"))
                    .0
            };
            let circle = find("Some Circle Name");
            let first_tag = find("Genre number 0 ");
            let gap = first_tag.top() - circle.bottom();
            assert!(
                (0.0..40.0).contains(&gap),
                "{width} wide: the first tag is {gap} below the circle"
            );
        }
    }

    #[test]
    fn the_pad_can_reach_every_control_at_every_size() {
        use dackel_launcher::nav::{Dir, spatial_next};
        for width in preset_widths() {
            let (_, items) = draw_page(width, 12, true);
            let rects: Vec<_> = items.iter().map(|i| crate::focus::bounds(i.rect)).collect();
            let mut seen = vec![false; items.len()];
            let mut stack = vec![0];
            seen[0] = true;
            while let Some(at) = stack.pop() {
                for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
                    let next = spatial_next(&rects, at, dir);
                    if !seen[next] {
                        seen[next] = true;
                        stack.push(next);
                    }
                }
            }
            let missed: Vec<_> = items
                .iter()
                .zip(&seen)
                .filter(|(_, seen)| !**seen)
                .map(|(i, _)| format!("{:?}", i.target))
                .collect();
            assert!(
                missed.is_empty(),
                "{width} wide: the pad cannot reach {missed:?}"
            );
        }
    }

    #[test]
    fn a_save_then_refresh_shows_what_was_written() {
        let game = tempfile::tempdir().unwrap();
        let mut edit = TuningEdit::load("k".into(), game.path());
        edit.env_text = "A=1\n  B = \"two\"".to_owned();
        edit.args = "  -windowed   -fps 60 ".to_owned();
        edit.verbs_text = "dotnet48".to_owned();
        edit.open = true;

        let env = parse_env(&edit.env_text).unwrap();
        profile::set_tuning(game.path(), env, &edit.args).unwrap();
        edit.refresh();

        assert_eq!(edit.env_text, "A=1\nB=two\n");
        assert_eq!(edit.args, "  -windowed   -fps 60 ");
        assert_eq!(edit.verbs_text, "dotnet48");
        assert!(edit.open);
    }

    #[test]
    fn a_failed_save_keeps_the_typed_text() {
        let game = tempfile::tempdir().unwrap();
        // A file where the game folder should be, so the save cannot write its profile.
        let blocked = game.path().join("blocked");
        std::fs::write(&blocked, "").unwrap();
        let mut edit = TuningEdit::load("k".into(), game.path());
        edit.env_text = "A=1".to_owned();
        edit.args = "-windowed".to_owned();

        let env = parse_env(&edit.env_text).unwrap();
        assert!(profile::set_tuning(&blocked, env, &edit.args).is_err());

        assert_eq!(edit.env_text, "A=1");
        assert_eq!(edit.args, "-windowed");
    }

    #[test]
    fn a_fresh_load_shows_the_stored_values_not_the_unsaved_text() {
        let game = tempfile::tempdir().unwrap();
        let env = parse_env("A=1").unwrap();
        profile::set_tuning(game.path(), env, "-windowed").unwrap();

        let mut edit = TuningEdit::load("k".into(), game.path());
        edit.env_text = "UNSAVED=yes".to_owned();
        edit.args = "-unsaved".to_owned();

        let edit = TuningEdit::load("k".into(), game.path());
        assert_eq!(edit.env_text, "A=1\n");
        assert_eq!(edit.args, "-windowed");
    }

    #[test]
    fn the_form_is_kept_only_while_its_own_game_is_open() {
        let edit = TuningEdit::load("RJ1".into(), &std::env::temp_dir());
        assert!(edit.keeps(Some("RJ1")));
        assert!(!edit.keeps(Some("RJ2")));
        assert!(!edit.keeps(None));
    }
}
