mod covers;
mod details;
mod focus;
mod icons;

use covers::{BlurCache, CoverView};
use details::{DetailsView, details_ui, exe_choices};
use dlsite_launcher::browse::{self, Card, Query, Sort};
use dlsite_launcher::extract::{self, Encoding, Progress, Status};
use dlsite_launcher::fetch::HttpFetch;
use dlsite_launcher::fonts;
use dlsite_launcher::info::{GameInfo, cover_path, load};
use dlsite_launcher::jobs::{ExtractJob, JobStatus};
use dlsite_launcher::launch::{Launcher, UmuSpawner, unix_now};
use dlsite_launcher::library::{Game, add_library, library_labels, resolve_library, scan_all};
use dlsite_launcher::nav::{
    self, Command, Context, Control, DEBOUNCE_PRESETS_MS, DEFAULT_DEBOUNCE_MS, Debounce, Input,
    Repeater,
};
use dlsite_launcher::profile::{self, ExeState, Stats};
use dlsite_launcher::proton;
use dlsite_launcher::reload::Reloads;
use dlsite_launcher::settings::{self, Settings};
use dlsite_launcher::window::{self, Restore, RestoreStep};
use eframe::egui;
use focus::{FocusFrame, Item, Target, bounds};
use gilrs::{Axis, Button, EventType, Gilrs};
use icons::{Icon, icon_button};
use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

fn main() -> eframe::Result {
    let override_dir = match parse_library_override() {
        Ok(dir) => dir,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    let exe = std::env::current_exe().expect("cannot determine executable path");
    let library = match resolve_library(&exe, override_dir.as_deref()) {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("cannot locate library: {err}");
            std::process::exit(1);
        }
    };

    let startup = Startup::detect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_fullscreen(startup.fullscreen),
        ..Default::default()
    };
    eframe::run_native(
        "DLsite Launcher",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            // Slightly larger buttons everywhere, which also suits a controller on the Deck.
            cc.egui_ctx.global_style_mut(|style| {
                style.spacing.button_padding = egui::vec2(12.0, 7.0);
                style.spacing.interact_size.y = icons::BUTTON_HEIGHT;
                // Larger text to match the large buttons. The Button style is left alone, so
                // buttons keep their size.
                for (text_style, size) in [
                    (egui::TextStyle::Small, 13.0),
                    (egui::TextStyle::Body, 17.0),
                    (egui::TextStyle::Monospace, 16.0),
                    (egui::TextStyle::Heading, 24.0),
                ] {
                    style
                        .text_styles
                        .insert(text_style, egui::FontId::proportional(size));
                }
            });
            install_japanese_font(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(startup.scale);
            Ok(Box::new(App::new(library, startup)))
        }),
    )
}

const TILE_WIDTH: f32 = 200.0;

const DEBOUNCE_HELP: &str = "Press debounce\n\
Ignores a repeat of the same button within this time. Raise it if one D-pad press moves \
twice (Steam Input can report each press from two devices); Off disables it.";

const SORT_HELP: &str = "Sort\n\
Last played: most recently played first; games never played go last.\n\
Title: alphabetical by title.\n\
Date added: newest folder first.";

const ZIP_ENCODING_HELP: &str = "Zip encoding\n\
How file names inside a zip are read when you press Extract or Re-extract.\n\
Auto: UTF-8 if the zip says so, otherwise cp932 (Shift-JIS).\n\
cp932 / UTF-8: force one. If extracted names come out garbled, pick the other\n\
and press Re-extract.";

/// Padding around each tile. The focus ring is drawn inside this margin, so it can never
/// reach a neighboring tile or the row above.
const RING_PAD: i8 = 6;
const CELL_WIDTH: f32 = TILE_WIDTH + 2.0 * RING_PAD as f32;

/// Per-machine startup state, read from the user's config directory and the environment.
struct Startup {
    settings: Settings,
    settings_path: Option<PathBuf>,
    minimize_on_launch: bool,
    fullscreen: bool,
    scale: f32,
}

impl Startup {
    fn detect() -> Self {
        let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let settings_path = settings::settings_path(xdg.as_deref(), home.as_deref());
        let settings = settings_path
            .as_deref()
            .map(settings::load)
            .unwrap_or_default();
        let deck_like = window::is_deck_like(|key| std::env::var(key).ok());
        let native_wayland = window::is_native_wayland(|key| std::env::var(key).ok());
        Startup {
            fullscreen: window::initial_fullscreen(&settings, deck_like),
            scale: settings
                .ui_scale
                .unwrap_or_else(|| window::default_ui_scale(deck_like)),
            settings,
            settings_path,
            minimize_on_launch: window::should_minimize_on_launch(deck_like, native_wayland),
        }
    }
}

/// egui's built-in fonts have no Japanese glyphs; add a system font as a fallback.
fn install_japanese_font(ctx: &egui::Context) {
    let Some(source) = fonts::locate_japanese_font() else {
        eprintln!("no Japanese font found; Japanese text will show as boxes");
        return;
    };
    let bytes = match std::fs::read(&source.path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("cannot read font {}: {err}", source.path.display());
            return;
        }
    };
    let mut data = egui::FontData::from_owned(bytes);
    data.index = source.index;
    let mut definitions = egui::FontDefinitions::default();
    definitions
        .font_data
        .insert("japanese".to_owned(), Arc::new(data));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        definitions
            .families
            .entry(family)
            .or_default()
            .push("japanese".to_owned());
    }
    ctx.set_fonts(definitions);
}

fn parse_library_override() -> Result<Option<PathBuf>, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = if arg == "--library" {
            args.next().ok_or("--library requires a path")?
        } else if let Some(value) = arg.strip_prefix("--library=") {
            value.to_owned()
        } else {
            continue;
        };
        let path = std::path::absolute(&value).map_err(|e| format!("bad --library path: {e}"))?;
        return Ok(Some(path));
    }
    Ok(None)
}

struct Entry {
    game: Game,
    state: Result<ExeState, String>,
    extract: Result<Status, String>,
    stats: Stats,
    added: Option<u64>,
    candidates: Vec<PathBuf>,
}

enum Screen {
    Grid,
    Details(String),
}

enum View {
    Missing,
    Games(Vec<Entry>),
    Error(String),
}

/// A small list opened from a toolbar dropdown so the controller can choose from it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Picker {
    Sort,
    Encoding,
    Scale,
    Debounce,
    Library,
}

const SORTS: [Sort; 3] = [Sort::LastPlayed, Sort::Title, Sort::DateAdded];
const ENCODINGS: [Encoding; 3] = [Encoding::Auto, Encoding::Cp932, Encoding::Utf8];

/// What a dropdown popup shows: its title, its entries, and which one is current.
struct PickerList {
    title: &'static str,
    labels: Vec<String>,
    current: usize,
}

struct App {
    /// The library beside the binary (or `--library`); always first in `libraries`.
    library: PathBuf,
    /// Every library in use: `library`, then the ones added on this machine.
    libraries: Vec<PathBuf>,
    /// Libraries that could not be read at the last scan (drive unplugged, folder gone).
    offline: Vec<PathBuf>,
    /// Show only games from this library.
    library_filter: Option<PathBuf>,
    libraries_open: bool,
    library_input: String,
    library_error: Option<String>,
    view: View,
    chooser: Option<usize>,
    encoding: Encoding,
    message: Option<String>,
    infos: HashMap<String, GameInfo>,
    errors: HashMap<String, String>,
    busy: HashSet<String>,
    reloads: Reloads,
    launcher: Launcher<UmuSpawner>,
    screen: Screen,
    query: Query,
    gilrs: Option<Gilrs>,
    stick: Repeater,
    /// Drops a press that a second device (Steam's virtual pad) reports again.
    debounce: Debounce,
    /// The game the controller cursor is on (by id, so it survives rescans and re-sorts).
    focus_id: Option<String>,
    /// True until the user moves the cursor: it then tracks the first game as the list re-sorts.
    focus_auto: bool,
    list_sel: usize,
    /// Set when focus is on a toolbar or details-page control instead of a game tile.
    control_focus: Option<Control>,
    /// Where everything focusable was on the last frame; D-pad moves use it.
    nav_items: Vec<Item>,
    /// A control the controller pressed with A, consumed by that control next frame.
    pending_press: Option<Control>,
    picker: Option<Picker>,
    scroll_to_focus: bool,
    settings: Settings,
    settings_path: Option<PathBuf>,
    minimize_on_launch: bool,
    fullscreen: bool,
    scale: f32,
    /// Window changes decided in `apply`, sent to egui at the end of the frame.
    window_cmds: Vec<egui::ViewportCommand>,
    zoom_request: Option<f32>,
    /// Set when a game launch minimized the window, so only then do we bring it back.
    /// Extractions running in the background, by game id.
    extracting: HashMap<String, ExtractJob>,
    blur_cache: BlurCache,
    /// Lets background work ask the window to redraw; set on the first frame.
    repaint: Option<egui::Context>,
    minimized_by_us: bool,
    restore: Restore,
    started: std::time::Instant,
}

impl App {
    fn new(library: PathBuf, startup: Startup) -> Self {
        let libraries = std::iter::once(library.clone())
            .chain(startup.settings.libraries.iter().cloned())
            .collect();
        let mut app = App {
            library,
            libraries,
            offline: Vec::new(),
            library_filter: None,
            libraries_open: false,
            library_input: String::new(),
            library_error: None,
            view: View::Missing,
            chooser: None,
            encoding: Encoding::Auto,
            message: None,
            infos: HashMap::new(),
            errors: HashMap::new(),
            busy: HashSet::new(),
            reloads: Reloads::new(Arc::new(HttpFetch)),
            launcher: Launcher::new(UmuSpawner),
            screen: Screen::Grid,
            query: Query {
                text: String::new(),
                favorites_only: false,
                sort: Sort::Title,
            },
            gilrs: Gilrs::new()
                .map_err(|err| eprintln!("gamepad support unavailable: {err}"))
                .ok(),
            stick: Repeater::default(),
            debounce: Debounce::new(startup.settings.debounce_ms.unwrap_or(DEFAULT_DEBOUNCE_MS)),
            focus_id: None,
            focus_auto: true,
            list_sel: 0,
            control_focus: None,
            nav_items: Vec::new(),
            pending_press: None,
            picker: None,
            scroll_to_focus: false,
            settings: startup.settings,
            settings_path: startup.settings_path,
            minimize_on_launch: startup.minimize_on_launch,
            fullscreen: startup.fullscreen,
            scale: startup.scale,
            window_cmds: Vec::new(),
            zoom_request: None,
            extracting: HashMap::new(),
            blur_cache: BlurCache::default(),
            repaint: None,
            minimized_by_us: false,
            restore: Restore::default(),
            started: std::time::Instant::now(),
        };
        app.rescan();
        app
    }

    fn rescan(&mut self) {
        self.chooser = None;
        let extracting: HashSet<String> = self.extracting.keys().cloned().collect();
        let scans = scan_all(&self.libraries);
        self.offline = scans
            .iter()
            .filter(|s| s.games.is_err())
            .map(|s| s.root.clone())
            .collect();
        let primary_error = scans
            .first()
            .and_then(|s| s.games.as_ref().err())
            .map(|err| (err.kind(), err.to_string()));
        let games: Vec<Game> = scans
            .into_iter()
            .filter_map(|s| s.games.ok())
            .flatten()
            .collect();
        self.view = match primary_error {
            // Nothing readable at all: the built-in library's own problem is the one to show.
            Some((kind, _))
                if games.is_empty()
                    && kind == ErrorKind::NotFound
                    && self.offline.len() == self.libraries.len() =>
            {
                View::Missing
            }
            Some((_, msg)) if games.is_empty() && self.offline.len() == self.libraries.len() => {
                View::Error(msg)
            }
            _ => View::Games(
                games
                    .into_iter()
                    .map(|game| {
                        // A folder being extracted is half built: do not detect (and pin) an
                        // exe from it yet. The rescan after the job finishes does that.
                        let state = if extracting.contains(&game.key()) {
                            Ok(ExeState::NeedsExeChoice {
                                candidates: Vec::new(),
                            })
                        } else {
                            profile::ensure(&game.dir).map_err(|e| e.to_string())
                        };
                        let extract = extract::status(&game.dir).map_err(|e| e.to_string());
                        let stats = profile::read_stats(&game.dir).unwrap_or_default();
                        let added = added_secs(&game.dir);
                        let candidates = profile::candidates(&game.dir).unwrap_or_default();
                        Entry {
                            game,
                            state,
                            extract,
                            stats,
                            added,
                            candidates,
                        }
                    })
                    .collect(),
            ),
        };
        // Drop a stale "not found" note; never replace some other message (e.g. "Extracted").
        if self
            .message
            .as_deref()
            .is_some_and(|m| m.starts_with("Library not found"))
        {
            self.message = None;
        }
        if !self.offline.is_empty() && self.message.is_none() && matches!(self.view, View::Games(_))
        {
            let names: Vec<String> = self
                .offline
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            self.message = Some(format!("Library not found: {}", names.join(", ")));
        }
        let games: Vec<Game> = match &self.view {
            View::Games(entries) => entries.iter().map(|e| e.game.clone()).collect(),
            _ => return,
        };
        for game in games {
            match load(&game) {
                Some(info) => {
                    self.infos.insert(game.key(), info);
                }
                None => self.reload(game),
            }
        }
    }

    fn reload(&mut self, game: Game) {
        let key = game.key();
        if self.busy.insert(key.clone()) {
            self.errors.remove(&key);
            self.reloads.start(game);
        }
    }

    fn collect_reloads(&mut self) {
        while let Some(done) = self.reloads.try_next() {
            self.busy.remove(&done.key);
            // A reload may have replaced the cover, so any blurred copy of it is stale.
            self.blur_cache.clear();
            match done.result {
                Ok(info) => {
                    self.errors.remove(&done.key);
                    self.infos.insert(done.key, info);
                }
                Err(msg) => {
                    self.errors.insert(done.key, msg);
                }
            }
        }
    }

    /// Starts extracting in the background; progress shows on the tile and details page.
    fn extract_game(&mut self, dir: &std::path::Path) {
        let id = dir.to_string_lossy().into_owned();
        let name = dir
            .file_name()
            .map_or_else(|| id.clone(), |n| n.to_string_lossy().into_owned());
        if self.extracting.contains_key(&id) {
            return;
        }
        let Some(ctx) = self.repaint.clone() else {
            return;
        };
        let job = ExtractJob::start(dir.to_path_buf(), self.encoding, move || {
            ctx.request_repaint()
        });
        self.extracting.insert(id.clone(), job);
        self.message = Some(format!("Extracting {name}..."));
    }

    /// Asks a running extraction to stop; it removes what it created once it has.
    fn cancel_extraction(&mut self, id: &str) {
        if let Some(job) = self.extracting.get(id) {
            job.cancel();
            self.message = Some(format!("Cancelling {}...", file_name_of(id)));
        }
    }

    /// Collects finished extractions. Runs from `logic`, so it works while minimized.
    fn poll_extractions(&mut self) {
        let mut finished = Vec::new();
        for (id, job) in &mut self.extracting {
            match job.poll() {
                JobStatus::Running(_) => {}
                done => finished.push((id.clone(), done)),
            }
        }
        if finished.is_empty() {
            return;
        }
        for (id, status) in finished {
            self.extracting.remove(&id);
            self.message = Some(match status {
                JobStatus::Finished(Ok(())) => {
                    format!("Extracted {id}")
                }
                JobStatus::Finished(Err(err)) => {
                    format!("Extract failed ({}): {err}", file_name_of(&id))
                }
                JobStatus::Cancelled => {
                    format!(
                        "Cancelled extracting {}; removed what it had created",
                        file_name_of(&id)
                    )
                }
                JobStatus::Running(_) => continue,
            });
        }
        // Rescan only now: doing it mid-extraction could pin an exe from a half-built folder.
        self.rescan();
    }

    fn launch_game(&mut self, index: usize) {
        let View::Games(entries) = &self.view else {
            return;
        };
        let Some(entry) = entries.get(index) else {
            return;
        };
        let Ok(state) = &entry.state else {
            return;
        };
        self.message = Some(match proton::resolve_default() {
            Err(err) => format!("Launch failed: {err}"),
            Ok(proton) => {
                match self
                    .launcher
                    .start(&entry.game.dir, state, proton.as_deref(), unix_now())
                {
                    Ok(()) => {
                        if self.minimize_on_launch {
                            // Hand the screen to the game; minimize rather than close.
                            self.window_cmds
                                .push(egui::ViewportCommand::Minimized(true));
                            self.minimized_by_us = true;
                        }
                        format!("Launched {}", entry.game.id)
                    }
                    Err(err) => format!("Launch failed: {err}"),
                }
            }
        });
    }

    fn toggle_favorite(&mut self, id: &str) {
        let View::Games(entries) = &mut self.view else {
            return;
        };
        let Some(entry) = entries.iter_mut().find(|e| e.game.key() == id) else {
            return;
        };
        let favorite = !entry.stats.favorite;
        match profile::set_favorite(&entry.game.dir, favorite) {
            Ok(()) => entry.stats.favorite = favorite,
            Err(err) => self.message = Some(format!("Could not save favorite: {err}")),
        }
    }

    /// Entry indices in the order the grid shows them.
    fn visible(&self) -> Vec<usize> {
        let View::Games(entries) = &self.view else {
            return Vec::new();
        };
        let cards: Vec<Card> = entries
            .iter()
            .map(|e| card_of(e, self.infos.get(&e.game.key())))
            .collect();
        browse::browse(&cards, &self.query)
            .into_iter()
            .filter(|&i| {
                self.library_filter
                    .as_deref()
                    .is_none_or(|root| entries[i].game.dir.parent() == Some(root))
            })
            .collect()
    }

    /// The name to show beside a game's library; `None` with a single library.
    fn library_label(&self, game: &Game) -> Option<String> {
        if self.libraries.len() < 2 {
            return None;
        }
        let at = self
            .libraries
            .iter()
            .position(|root| game.dir.parent() == Some(root.as_path()))?;
        library_labels(&self.libraries).into_iter().nth(at)
    }

    /// Keeps the cursor on a game that is on screen, so focus is never lost.
    fn normalize_focus(&mut self, visible: &[usize]) {
        let View::Games(entries) = &self.view else {
            return;
        };
        let on_screen = |id: &String| visible.iter().any(|&i| &entries[i].game.key() == id);
        if self.focus_auto || !self.focus_id.as_ref().is_some_and(on_screen) {
            self.focus_id = visible.first().map(|&i| entries[i].game.key().clone());
        }
    }

    /// The entry controller commands act on: the open details page, else the cursor.
    fn target_index(&self) -> Option<usize> {
        let View::Games(entries) = &self.view else {
            return None;
        };
        let id = match &self.screen {
            Screen::Details(id) => Some(id),
            Screen::Grid => self.focus_id.as_ref(),
        }?;
        entries.iter().position(|e| &e.game.key() == id)
    }

    fn context(&self) -> Context {
        if self.chooser.is_some() || self.picker.is_some() || self.libraries_open {
            Context::Chooser
        } else if matches!(self.screen, Screen::Details(_)) {
            Context::Details
        } else {
            Context::Grid
        }
    }

    /// Re-reads play time and last-played from the profiles, after a game exits.
    fn refresh_stats(&mut self) {
        if let View::Games(entries) = &mut self.view {
            for entry in entries {
                entry.stats = profile::read_stats(&entry.game.dir).unwrap_or(entry.stats);
            }
        }
    }

    fn save_settings(&mut self) {
        let Some(path) = &self.settings_path else {
            return;
        };
        if let Err(err) = settings::save(path, &self.settings) {
            self.message = Some(format!("Could not save settings: {err}"));
        }
    }

    fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.settings.fullscreen = Some(self.fullscreen);
        self.window_cmds
            .push(egui::ViewportCommand::Fullscreen(self.fullscreen));
        self.save_settings();
    }

    fn set_ui_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.settings.ui_scale = Some(scale);
        self.zoom_request = Some(scale);
        self.save_settings();
    }

    /// The manager for the extra libraries: list them, add a typed path, remove one.
    fn libraries_window(&mut self, ctx: &egui::Context) {
        let mut remove: Option<usize> = None;
        let mut add = false;
        let mut close = false;
        egui::Window::new("Libraries")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                for (i, root) in self.libraries.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(root.display().to_string());
                        if self.offline.contains(root) {
                            ui.colored_label(egui::Color32::YELLOW, "not found");
                        }
                        if i == 0 {
                            ui.weak("(beside the program)");
                        } else if ui.button("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                ui.separator();
                ui.label("Add a folder that contains RJ game folders:");
                ui.horizontal(|ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.library_input)
                            .desired_width(420.0)
                            .hint_text("/path/to/DLSiteLibrary"),
                    );
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Add").clicked() || enter {
                        add = true;
                    }
                });
                if let Some(err) = &self.library_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if add {
            match add_library(&self.libraries, &self.library_input) {
                Ok(path) => {
                    self.settings.libraries.push(path.clone());
                    self.libraries.push(path);
                    self.library_input.clear();
                    self.library_error = None;
                    self.save_settings();
                    self.rescan();
                }
                Err(err) => self.library_error = Some(err.to_string()),
            }
        }
        if let Some(i) = remove {
            let gone = self.libraries.remove(i);
            self.settings.libraries.retain(|l| *l != gone);
            if self.library_filter.as_ref() == Some(&gone) {
                self.library_filter = None;
            }
            // Games of the removed library: stop extracting them and leave their page.
            let inside = |key: &str| std::path::Path::new(key).parent() == Some(gone.as_path());
            let running: Vec<String> = self
                .extracting
                .keys()
                .filter(|k| inside(k))
                .cloned()
                .collect();
            for key in running {
                self.cancel_extraction(&key);
            }
            if matches!(&self.screen, Screen::Details(key) if inside(key)) {
                self.close_details_page();
            }
            self.save_settings();
            self.rescan();
        }
        if close {
            self.libraries_open = false;
            self.library_error = None;
        }
    }

    fn set_debounce(&mut self, ms: u64) {
        self.debounce.set_window(ms);
        self.settings.debounce_ms = Some(ms);
        self.save_settings();
    }

    fn toggle_blur(&mut self) {
        self.settings.blur_covers = !self.settings.blur_covers;
        self.save_settings();
    }

    /// Opens the details page for a game; focus starts on its Launch button.
    fn open_details_page(&mut self, id: String) {
        self.screen = Screen::Details(id);
        self.control_focus = Some(Control::Launch);
    }

    /// Back to the library; focus returns to the game that was selected.
    fn close_details_page(&mut self) {
        self.screen = Screen::Grid;
        self.control_focus = None;
        self.scroll_to_focus = true;
    }

    fn picker_list(&self, picker: Picker) -> PickerList {
        match picker {
            Picker::Sort => PickerList {
                title: "Sort",
                labels: SORTS.iter().map(|s| sort_label(*s).to_owned()).collect(),
                current: SORTS
                    .iter()
                    .position(|s| *s == self.query.sort)
                    .unwrap_or(0),
            },
            Picker::Encoding => PickerList {
                title: "Zip encoding",
                labels: ENCODINGS
                    .iter()
                    .map(|e| encoding_label(*e).to_owned())
                    .collect(),
                current: ENCODINGS
                    .iter()
                    .position(|e| *e == self.encoding)
                    .unwrap_or(0),
            },
            Picker::Library => {
                let mut labels = vec!["All libraries".to_owned()];
                labels.extend(library_labels(&self.libraries));
                let current = self
                    .library_filter
                    .as_ref()
                    .and_then(|f| self.libraries.iter().position(|l| l == f))
                    .map_or(0, |i| i + 1);
                PickerList {
                    title: "Library",
                    labels,
                    current,
                }
            }
            Picker::Debounce => PickerList {
                title: "Press debounce",
                labels: DEBOUNCE_PRESETS_MS
                    .iter()
                    .map(|ms| debounce_label(*ms))
                    .collect(),
                current: DEBOUNCE_PRESETS_MS
                    .iter()
                    .position(|ms| *ms == self.debounce.window())
                    .unwrap_or(0),
            },
            Picker::Scale => PickerList {
                title: "UI scale",
                labels: window::SCALE_PRESETS
                    .iter()
                    .map(|p| format!("{p:.2}"))
                    .collect(),
                current: window::SCALE_PRESETS
                    .iter()
                    .position(|p| (self.scale - p).abs() < 1e-3)
                    .unwrap_or(0),
            },
        }
    }

    fn open_picker(&mut self, picker: Picker) {
        self.list_sel = self.picker_list(picker).current;
        self.picker = Some(picker);
    }

    fn pick(&mut self, picker: Picker, index: usize) {
        match picker {
            Picker::Sort => {
                if let Some(sort) = SORTS.get(index) {
                    self.query.sort = *sort;
                }
            }
            Picker::Encoding => {
                if let Some(e) = ENCODINGS.get(index) {
                    self.encoding = *e;
                }
            }
            Picker::Library => {
                self.library_filter = index
                    .checked_sub(1)
                    .and_then(|i| self.libraries.get(i))
                    .cloned();
            }
            Picker::Debounce => {
                if let Some(ms) = DEBOUNCE_PRESETS_MS.get(index) {
                    self.set_debounce(*ms);
                }
            }
            Picker::Scale => {
                if let Some(scale) = window::SCALE_PRESETS.get(index) {
                    self.set_ui_scale(*scale);
                }
            }
        }
    }

    /// Keeps controller focus on something that exists on the current screen.
    fn normalize_control_focus(&mut self) {
        let on_screen =
            |c: Control, items: &[Item]| items.iter().any(|i| i.target == Target::Control(c));
        match (&self.screen, self.control_focus) {
            // `nav_items` is still the previous screen's layout until the details page has
            // been drawn once (its Back button is the first thing it registers).
            (Screen::Details(_), _) if !on_screen(Control::Back, &self.nav_items) => {}
            // The details page always has focus on one of its controls.
            (Screen::Details(_), Some(c)) if on_screen(c, &self.nav_items) => {}
            (Screen::Details(_), _) => self.control_focus = Some(Control::Back),
            // In the library, focus on a control that is gone goes back to the games.
            (Screen::Grid, Some(c)) if !on_screen(c, &self.nav_items) => self.control_focus = None,
            _ => {}
        }
    }

    fn open_chooser(&mut self, index: usize) {
        self.chooser = Some(index);
        self.list_sel = 0;
    }

    fn apply(&mut self, command: Command) {
        match command {
            Command::Move(dir) => self.move_focus(dir),
            Command::Primary => {
                // A control with focus is pressed; the library's dropdowns open a list.
                if let Some(control) = self.control_focus {
                    match control {
                        Control::Sort => self.open_picker(Picker::Sort),
                        Control::Encoding => self.open_picker(Picker::Encoding),
                        Control::Scale => self.open_picker(Picker::Scale),
                        Control::Debounce => self.open_picker(Picker::Debounce),
                        Control::LibraryFilter => self.open_picker(Picker::Library),
                        other => self.pending_press = Some(other),
                    }
                    return;
                }
                let Some(i) = self.target_index() else { return };
                let View::Games(entries) = &self.view else {
                    return;
                };
                let entry = &entries[i];
                let dir = entry.game.dir.clone();
                if self.launcher.is_running(&dir) || self.extracting.contains_key(&entry.game.key())
                {
                    return;
                }
                if matches!(entry.extract, Ok(Status::NotExtracted)) {
                    self.extract_game(&dir);
                } else if matches!(entry.state, Ok(ExeState::Ready { .. })) {
                    self.launch_game(i);
                } else if needs_choice(entry) {
                    self.open_chooser(i);
                }
            }
            Command::Reload => {
                // In the library, X acts on the focused game, not on a toolbar control.
                if matches!(self.screen, Screen::Grid) && self.control_focus.is_some() {
                    return;
                }
                if let (Some(i), View::Games(entries)) = (self.target_index(), &self.view) {
                    let game = entries[i].game.clone();
                    // While a game is extracting, this button cancels that instead.
                    if self.extracting.contains_key(&game.id) {
                        self.cancel_extraction(&game.id);
                    } else {
                        self.reload(game);
                    }
                }
            }
            Command::OpenDetails => {
                if self.control_focus.is_none()
                    && let Some(id) = self.focus_id.clone()
                {
                    self.open_details_page(id);
                }
            }
            Command::Back => self.close_details_page(),
            Command::ToggleFavorite => {
                if let (Some(i), View::Games(entries)) = (self.target_index(), &self.view) {
                    let id = entries[i].game.key().clone();
                    self.toggle_favorite(&id);
                }
            }
            Command::Rescan => self.rescan(),
            Command::ToggleFavoritesOnly => self.query.favorites_only = !self.query.favorites_only,
            Command::ResetFilters => {
                self.query.text.clear();
                self.query.favorites_only = false;
            }
            Command::ChooserMove(delta) => {
                let len = match self.picker {
                    Some(picker) => self.picker_list(picker).labels.len(),
                    None => match &self.view {
                        View::Games(entries) => self
                            .chooser
                            .and_then(|i| entries.get(i))
                            .map_or(0, |e| e.candidates.len()),
                        _ => 0,
                    },
                };
                let last = len.saturating_sub(1) as i64;
                self.list_sel = (self.list_sel as i64 + i64::from(delta)).clamp(0, last) as usize;
            }
            Command::ChooserPick => {
                if let Some(picker) = self.picker.take() {
                    self.pick(picker, self.list_sel);
                    return;
                }
                let View::Games(entries) = &self.view else {
                    return;
                };
                let picked = self.chooser.and_then(|i| entries.get(i)).and_then(|e| {
                    Some((e.game.dir.clone(), e.candidates.get(self.list_sel)?.clone()))
                });
                if let Some((dir, exe)) = picked {
                    match profile::choose(&dir, &exe) {
                        Ok(()) => self.rescan(),
                        Err(err) => self.view = View::Error(err.to_string()),
                    }
                }
            }
            Command::ChooserCancel => {
                self.chooser = None;
                self.picker = None;
                self.libraries_open = false;
            }
            Command::ToggleFullscreen => self.toggle_fullscreen(),
            Command::ToggleBlur => self.toggle_blur(),
        }
    }

    /// Moves focus to the nearest control or tile in `dir`, using where things were drawn.
    fn move_focus(&mut self, dir: nav::Dir) {
        let current = match (&self.control_focus, &self.focus_id) {
            (Some(c), _) => Target::Control(*c),
            (None, Some(id)) => Target::Game(id.clone()),
            (None, None) => return,
        };
        let Some(from) = self.nav_items.iter().position(|i| i.target == current) else {
            return;
        };
        let rects: Vec<nav::Bounds> = self.nav_items.iter().map(|i| bounds(i.rect)).collect();
        let to = nav::spatial_next(&rects, from, dir);
        match self.nav_items[to].target.clone() {
            Target::Game(id) => {
                self.focus_id = Some(id);
                self.control_focus = None;
                self.focus_auto = false;
                self.scroll_to_focus = true;
            }
            Target::Control(control) => self.control_focus = Some(control),
        }
    }

    /// Gamepad and keyboard presses since the last frame.
    fn poll_inputs(&mut self, ctx: &egui::Context) -> Vec<Input> {
        let mut inputs = Vec::new();
        let now_ms = (ctx.input(|i| i.time) * 1000.0) as u64;
        // Ignore the pad while another window (a running game) has focus.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if let Some(gilrs) = &mut self.gilrs {
            while let Some(event) = gilrs.next_event() {
                if let (true, EventType::ButtonPressed(button, _)) = (focused, event.event)
                    && let Some(input) = pad_input(button)
                {
                    inputs.push(input);
                }
            }
            let held = gilrs.gamepads().find_map(|(_, pad)| {
                nav::stick_dir(pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY))
            });
            let fired = self.stick.update(now_ms, held.filter(|_| focused));
            inputs.extend(fired.map(dir_input));
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        if !ctx.egui_wants_keyboard_input() {
            ctx.input(|i| {
                for (key, input) in KEYBOARD {
                    if i.key_pressed(*key) {
                        inputs.push(*input);
                    }
                }
            });
        }
        let now_ms = self.started.elapsed().as_millis() as u64;
        inputs.retain(|input| self.debounce.accept(now_ms, *input));
        inputs
    }

    fn create_library(&mut self) {
        match std::fs::create_dir_all(&self.library) {
            Ok(()) => self.rescan(),
            Err(err) => self.view = View::Error(err.to_string()),
        }
    }
}

impl eframe::App for App {
    /// Runs before every frame and, unlike `ui`, also while the window is minimized.
    /// Watching for a game's exit has to happen here, or play time would only be saved
    /// once the launcher is brought back on screen.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.repaint.is_none() {
            self.repaint = Some(ctx.clone());
        }
        self.poll_extractions();
        if !self.extracting.is_empty() {
            // Backstop in case a wake-up is missed.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        let was_running = self.launcher.is_running_any();
        if let Err(err) = self.launcher.poll(unix_now()) {
            self.message = Some(format!("Play time not saved: {err}"));
        }
        let running = self.launcher.is_running_any();
        if running {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        let now_ms = self.started.elapsed().as_millis() as u64;
        if was_running && !running {
            self.refresh_stats();
            if self.minimized_by_us {
                self.restore.arm(now_ms);
            }
        }
        if self.minimized_by_us && !running {
            // Bring the launcher back once the game is gone. Retried, because the window
            // manager may refuse while the game's window is still the active one.
            let minimized = ctx.input(|i| i.viewport().minimized) == Some(true);
            match self.restore.poll(now_ms, minimized) {
                RestoreStep::Request => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                RestoreStep::Done => self.minimized_by_us = false,
                RestoreStep::Idle => {}
            }
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.blur_cache.new_frame();
        let inputs = self.poll_inputs(&ctx);
        let visible = self.visible();
        self.normalize_focus(&visible);
        for input in inputs {
            if let Some(command) = nav::command(self.context(), input) {
                self.apply(command);
            }
        }
        // Commands can rescan or re-sort; make sure the cursor is still on a visible game.
        let visible = self.visible();
        self.normalize_focus(&visible);
        self.normalize_control_focus();
        let mut frame = FocusFrame::new(self.control_focus, self.pending_press.take());
        // Apply a requested zoom; otherwise notice egui's own Ctrl +/- zoom and remember it.
        if let Some(zoom) = self.zoom_request.take() {
            ctx.set_zoom_factor(zoom);
        } else if (ctx.zoom_factor() - self.scale).abs() > 1e-3 {
            self.set_ui_scale(ctx.zoom_factor());
        }

        let mut rescan = false;
        let mut extract_dir: Option<PathBuf> = None;
        let mut create = false;
        let mut choice = None;
        let mut close_chooser = false;
        let mut picker_pick: Option<(Picker, usize)> = None;
        let mut picker_cancel = false;
        let mut launch = None;
        let mut clicked_focus: Option<String> = None;
        let mut cancel_ids: Vec<String> = Vec::new();
        let mut open_details: Option<String> = None;
        let mut favorite_id: Option<String> = None;
        let mut back = false;
        let mut details_reload: Option<Game> = None;
        let mut details_extract: Option<PathBuf> = None;
        let mut details_choice: Option<(PathBuf, PathBuf)> = None;
        self.collect_reloads();
        if !self.busy.is_empty() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        let current_scale = self.scale;
        let mut picked_scale: Option<f32> = None;
        let mut picked_debounce: Option<u64> = None;
        let mut fullscreen_click = false;
        let mut blur_on = self.settings.blur_covers;
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                let response = ui.button("Rescan");
                if frame.pressed(ui, Control::Rescan, &response, true) {
                    rescan = true;
                }
                let response = ui.button("Libraries");
                if frame.pressed(ui, Control::Libraries, &response, true) {
                    self.libraries_open = true;
                }
                let response = ui.button("Fullscreen");
                fullscreen_click = frame.pressed(ui, Control::Fullscreen, &response, true);
                let response = ui
                    .toggle_value(&mut blur_on, "Blur")
                    .on_hover_text("Blur all images (key: B)");
                frame.register(ui, Control::Blur, &response);
                if frame.activated(Control::Blur) {
                    blur_on = !blur_on;
                }
                let response = egui::ComboBox::from_id_salt("ui-scale")
                    .selected_text(format!("UI x{current_scale:.2}"))
                    .show_ui(ui, |ui| {
                        for preset in window::SCALE_PRESETS {
                            let on = (current_scale - preset).abs() < 1e-3;
                            if ui.selectable_label(on, format!("{preset:.2}")).clicked() {
                                picked_scale = Some(preset);
                            }
                        }
                    })
                    .response;
                frame.register(ui, Control::Scale, &response);
                let response = egui::ComboBox::from_id_salt("debounce")
                    .selected_text(format!(
                        "Debounce {}",
                        debounce_label(self.debounce.window())
                    ))
                    .show_ui(ui, |ui| {
                        for ms in DEBOUNCE_PRESETS_MS {
                            let on = ms == self.debounce.window();
                            if ui.selectable_label(on, debounce_label(ms)).clicked() {
                                picked_debounce = Some(ms);
                            }
                        }
                    })
                    .response
                    .on_hover_text(DEBOUNCE_HELP);
                frame.register(ui, Control::Debounce, &response);
                let response = egui::ComboBox::from_id_salt("zip-encoding")
                    .selected_text(encoding_label(self.encoding))
                    .show_ui(ui, |ui| {
                        for choice in ENCODINGS {
                            ui.selectable_value(&mut self.encoding, choice, encoding_label(choice));
                        }
                    })
                    .response
                    .on_hover_text(ZIP_ENCODING_HELP);
                frame.register(ui, Control::Encoding, &response);
                ui.label(self.library.display().to_string());
            });
            if matches!(self.screen, Screen::Grid) {
                ui.horizontal(|ui| {
                    let response = egui::ComboBox::from_id_salt("sort")
                        .selected_text(sort_label(self.query.sort))
                        .show_ui(ui, |ui| {
                            for sort in SORTS {
                                ui.selectable_value(&mut self.query.sort, sort, sort_label(sort));
                            }
                        })
                        .response
                        .on_hover_text(SORT_HELP);
                    frame.register(ui, Control::Sort, &response);
                    if self.libraries.len() > 1 {
                        let labels = library_labels(&self.libraries);
                        let shown = self
                            .library_filter
                            .as_ref()
                            .and_then(|f| self.libraries.iter().position(|l| l == f))
                            .map_or("All libraries", |i| labels[i].as_str());
                        let response = egui::ComboBox::from_id_salt("library-filter")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        self.library_filter.is_none(),
                                        "All libraries",
                                    )
                                    .clicked()
                                {
                                    self.library_filter = None;
                                }
                                for (root, label) in self.libraries.iter().zip(&labels) {
                                    let on = self.library_filter.as_ref() == Some(root);
                                    if ui.selectable_label(on, label).clicked() {
                                        self.library_filter = Some(root.clone());
                                    }
                                }
                            })
                            .response
                            .on_hover_text("Show games from one library");
                        frame.register(ui, Control::LibraryFilter, &response);
                    }
                    ui.label("Search");
                    ui.text_edit_singleline(&mut self.query.text);
                    let response = ui.checkbox(&mut self.query.favorites_only, "Favorites only");
                    frame.register(ui, Control::FavoritesOnly, &response);
                    if frame.activated(Control::FavoritesOnly) {
                        self.query.favorites_only = !self.query.favorites_only;
                    }
                });
            }
            if let Some(message) = &self.message {
                // Status line ("Extracted ...", "Launched ..."): small and muted.
                ui.label(egui::RichText::new(message).small().weak());
            }
        });

        if fullscreen_click {
            self.toggle_fullscreen();
        }
        if blur_on != self.settings.blur_covers {
            self.toggle_blur();
        }
        if let Some(ms) = picked_debounce {
            self.set_debounce(ms);
        }
        if let Some(scale) = picked_scale {
            self.set_ui_scale(scale);
        }

        let hint = match self.context() {
            Context::Grid => {
                "D-pad Move   A Open / Press   B Reset filters   X Reload / Cancel extract   Y Details   Start Rescan   Select Favorites only"
            }
            Context::Details => {
                "D-pad Move   A Press   B Back   X Reload / Cancel extract   Y Favorite   Start Rescan"
            }
            Context::Chooser => "Up/Down Select   A Pick   B Cancel",
        };
        egui::Panel::bottom("hints").show(ui, |ui| {
            ui.weak(hint);
        });

        egui::CentralPanel::default().show(ui, |ui| match (&self.screen, &self.view) {
            (_, View::Missing) => {
                ui.heading("Library not found");
                ui.label(format!("{} does not exist.", self.library.display()));
                if ui.button("Create it").clicked() {
                    create = true;
                }
            }
            (_, View::Error(msg)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, msg);
            }
            (Screen::Details(id), View::Games(entries))
                if entries.iter().any(|e| &e.game.key() == id) =>
            {
                let entry = entries.iter().find(|e| &e.game.key() == id).unwrap();
                let info = self.infos.get(id);
                let cover = info
                    .and_then(|info| cover_path(&entry.game, info))
                    .filter(|path| path.exists());
                let cover_view =
                    self.blur_cache
                        .cover_view(&ctx, self.settings.blur_covers, cover.as_deref());
                let view = DetailsView {
                    id: &entry.game.id,
                    library: self.library_label(&entry.game),
                    info,
                    cover,
                    cover_view,
                    stats: entry.stats,
                    exe: match &entry.state {
                        Ok(ExeState::Ready { exe }) => Some(exe.as_path()),
                        _ => None,
                    },
                    candidates: &entry.candidates,
                    extract: &entry.extract,
                    error: self.errors.get(id).map(String::as_str),
                    busy: self.busy.contains(id),
                    running: self.launcher.is_running(&entry.game.dir),
                    extracting: self.extracting.get(id).map(ExtractJob::progress),
                };
                let action = details_ui(ui, &view, &mut frame);
                back = action.back;
                if action.reload {
                    details_reload = Some(entry.game.clone());
                }
                if action.extract {
                    details_extract = Some(entry.game.dir.clone());
                }
                if action.cancel_extract {
                    cancel_ids.push(id.clone());
                }
                if action.launch {
                    launch = entries.iter().position(|e| &e.game.key() == id);
                }
                if action.toggle_favorite {
                    favorite_id = Some(id.clone());
                }
                if let Some(exe) = action.choose_exe {
                    details_choice = Some((entry.game.dir.clone(), exe));
                }
            }
            (_, View::Games(entries)) if entries.is_empty() => {
                ui.label("No games yet. Put RJ folders in the library and press Rescan.");
            }
            (_, View::Games(entries)) => {
                // The same list the controller moves through, library filter included.
                let order = self.visible();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let spacing = ui.spacing().item_spacing.x;
                        let columns = ((ui.available_width() + spacing) / (CELL_WIDTH + spacing))
                            .floor()
                            .max(1.0) as usize;
                        for row in order.chunks(columns) {
                            ui.horizontal_top(|ui| {
                                for &i in row {
                                    let entry = &entries[i];
                                    let cover_file = self
                                        .infos
                                        .get(&entry.game.key())
                                        .and_then(|info| cover_path(&entry.game, info))
                                        .filter(|path| path.exists());
                                    let cover_view = self.blur_cache.cover_view(
                                        &ctx,
                                        self.settings.blur_covers,
                                        cover_file.as_deref(),
                                    );
                                    let tile = Tile {
                                        entry,
                                        info: self.infos.get(&entry.game.key()),
                                        error: self
                                            .errors
                                            .get(&entry.game.key())
                                            .map(String::as_str),
                                        busy: self.busy.contains(&entry.game.key()),
                                        running: self.launcher.is_running(&entry.game.dir),
                                        extracting: self
                                            .extracting
                                            .get(&entry.game.key())
                                            .map(ExtractJob::progress),
                                        focused: self.control_focus.is_none()
                                            && self.focus_id.as_ref() == Some(&entry.game.key()),
                                        cover_file,
                                        cover_view,
                                    };
                                    let action = tile_ui(ui, &tile);
                                    frame.items.push(Item {
                                        target: Target::Game(entry.game.key().clone()),
                                        rect: action.rect,
                                    });
                                    if tile.focused && self.scroll_to_focus {
                                        ui.scroll_to_rect(action.rect.expand(8.0), None);
                                    }
                                    if action.clicked
                                        || action.launch
                                        || action.details
                                        || action.favorite
                                        || action.extract.is_some()
                                    {
                                        clicked_focus = Some(entry.game.key().clone());
                                    }
                                    if let Some(dir) = action.extract {
                                        extract_dir = Some(dir);
                                    }
                                    if action.launch {
                                        launch = Some(i);
                                    }
                                    if action.cancel_extract {
                                        cancel_ids.push(entry.game.key().clone());
                                    }
                                    if action.favorite {
                                        favorite_id = Some(entry.game.key().clone());
                                    }
                                    if action.details {
                                        open_details = Some(entry.game.key().clone());
                                    }
                                    // One click only moves the focus (done above); two open the
                                    // details page, which works for any game, extracted or not.
                                    if action.double_clicked {
                                        open_details = Some(entry.game.key().clone());
                                    }
                                }
                            });
                        }
                    });
            }
        });

        if let (Some(i), View::Games(entries)) = (self.chooser, &self.view)
            && let Some(entry) = entries.get(i)
        {
            egui::Window::new(format!("Choose exe for {}", entry.game.id))
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    if entry.candidates.is_empty() {
                        ui.label("No exe found in this folder.");
                    }
                    if let Some(candidate) =
                        exe_choices(ui, &entry.candidates, Some(self.list_sel), None)
                    {
                        choice = Some((entry.game.dir.clone(), candidate));
                    }
                    if ui.button("Cancel").clicked() {
                        close_chooser = true;
                    }
                });
        }

        if let Some(picker) = self.picker {
            let list = self.picker_list(picker);
            match picker_window(ui.ctx(), &list, self.list_sel) {
                Some(PickerEvent::Pick(i)) => picker_pick = Some((picker, i)),
                Some(PickerEvent::Cancel) => picker_cancel = true,
                None => {}
            }
        }

        if self.libraries_open {
            self.libraries_window(ui.ctx());
        }

        self.nav_items = std::mem::take(&mut frame.items);
        self.scroll_to_focus = false;
        if let Some((picker, index)) = picker_pick {
            self.picker = None;
            self.pick(picker, index);
        }
        if picker_cancel {
            self.picker = None;
        }
        if let Some(id) = clicked_focus {
            self.focus_id = Some(id);
            self.control_focus = None;
            self.focus_auto = false;
        }
        for id in cancel_ids {
            self.cancel_extraction(&id);
        }
        if let Some(i) = launch {
            self.launch_game(i);
        }
        if close_chooser {
            self.chooser = None;
        }
        if let Some(id) = open_details {
            self.open_details_page(id);
        }
        if back {
            self.close_details_page();
        }
        if let Some(id) = favorite_id {
            self.toggle_favorite(&id);
        }
        if let Some(game) = details_reload {
            self.reload(game);
        }
        if let Some((dir, exe)) = details_choice.or(choice) {
            match profile::choose(&dir, &exe) {
                Ok(()) => self.rescan(),
                Err(err) => self.view = View::Error(err.to_string()),
            }
        } else if let Some(dir) = extract_dir.or(details_extract) {
            self.extract_game(&dir);
        } else if create {
            self.create_library();
        } else if rescan {
            self.rescan();
        }
        for command in self.window_cmds.drain(..) {
            ctx.send_viewport_cmd(command);
        }
    }
}

fn card_of(entry: &Entry, info: Option<&GameInfo>) -> Card {
    Card {
        id: entry.game.id.clone(),
        title: info.map_or_else(|| entry.game.id.clone(), |info| info.title.clone()),
        circle: info.and_then(|info| info.circle.clone()),
        favorite: entry.stats.favorite,
        last_played: entry.stats.last_played,
        added: entry.added,
    }
}

fn added_secs(dir: &Path) -> Option<u64> {
    let meta = std::fs::metadata(dir).ok()?;
    let time = meta.created().or_else(|_| meta.modified()).ok()?;
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

fn sort_label(sort: Sort) -> &'static str {
    match sort {
        Sort::LastPlayed => "Last played",
        Sort::Title => "Title",
        Sort::DateAdded => "Date added",
    }
}

fn needs_choice(entry: &Entry) -> bool {
    matches!(entry.state, Ok(ExeState::NeedsExeChoice { .. }))
}

struct Tile<'a> {
    entry: &'a Entry,
    info: Option<&'a GameInfo>,
    error: Option<&'a str>,
    busy: bool,
    running: bool,
    extracting: Option<Progress>,
    focused: bool,
    cover_file: Option<PathBuf>,
    cover_view: CoverView,
}

struct TileAction {
    launch: bool,
    clicked: bool,
    double_clicked: bool,
    extract: Option<PathBuf>,
    favorite: bool,
    details: bool,
    cancel_extract: bool,
    rect: egui::Rect,
}

/// Draws one game tile: cover, title, status and the Launch, Details and Favorite buttons.
fn tile_ui(ui: &mut egui::Ui, tile: &Tile) -> TileAction {
    let game = &tile.entry.game;
    let mut action = TileAction {
        launch: false,
        clicked: false,
        double_clicked: false,
        extract: None,
        favorite: false,
        details: false,
        cancel_extract: false,
        rect: egui::Rect::NOTHING,
    };
    let cell = egui::Frame::new()
        .inner_margin(egui::Margin::same(RING_PAD))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(TILE_WIDTH);
                let size = egui::vec2(TILE_WIDTH, 150.0);
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                action.clicked = response.clicked();
                action.double_clicked = response.double_clicked();
                ui.painter()
                    .rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
                match (&tile.cover_view, &tile.cover_file) {
                    // Blur is on: only ever the blurred copy, or an empty box while it is made.
                    (CoverView::Blurred(texture), _) => {
                        if let Some(texture) = texture {
                            ui.put(rect, egui::Image::new(texture).fit_to_exact_size(size));
                        }
                    }
                    (CoverView::Normal, Some(path)) => {
                        let uri = format!("file://{}", path.display());
                        ui.put(rect, egui::Image::new(uri).max_size(size));
                    }
                    (CoverView::Normal, None) => {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "No cover",
                            egui::FontId::proportional(16.0),
                            ui.visuals().weak_text_color(),
                        );
                    }
                }
                let title = tile
                    .info
                    .map_or(game.id.as_str(), |info| info.title.as_str());
                // Reserve two title lines so the button rows line up across a grid row.
                ui.vertical(|ui| {
                    ui.set_min_height(2.0 * ui.text_style_height(&egui::TextStyle::Body));
                    ui.add(egui::Label::new(two_line_title(ui, title)));
                });
                if tile.extracting.is_some() {
                    ui.weak("extracting...");
                } else if tile.running {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "Running");
                } else if tile.busy {
                    ui.weak("Reloading...");
                } else {
                    match (&tile.entry.state, &tile.entry.extract) {
                        (_, Err(msg)) | (Err(msg), _) => {
                            ui.colored_label(egui::Color32::LIGHT_RED, msg);
                        }
                        (_, Ok(Status::NotExtracted)) => {
                            ui.weak("not extracted");
                        }
                        (_, Ok(Status::ExtractManually(_))) => {
                            ui.colored_label(egui::Color32::YELLOW, "extract manually");
                        }
                        (Ok(ExeState::NeedsExeChoice { .. }), _) => {
                            ui.weak("needs exe choice");
                        }
                        (Ok(ExeState::Ready { .. }), _) => {
                            ui.weak(" ");
                        }
                    }
                }
                if let Some(err) = tile.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
                ui.horizontal(|ui| {
                    if let Some(progress) = tile.extracting {
                        let bar_width =
                            TILE_WIDTH - icons::ICON_BUTTON.x - ui.spacing().item_spacing.x;
                        ui.add(
                            egui::ProgressBar::new(progress.fraction())
                                .desired_width(bar_width)
                                .desired_height(icons::BUTTON_HEIGHT)
                                .text(format!("Extracting {:.0}%", progress.fraction() * 100.0)),
                        );
                        action.cancel_extract =
                            icon_button(ui, Icon::Close, "Cancel extraction", true).clicked();
                        return;
                    }
                    let not_extracted = matches!(tile.entry.extract, Ok(Status::NotExtracted));
                    let ready = matches!(tile.entry.state, Ok(ExeState::Ready { .. }));
                    if tile.running {
                        icon_button(ui, Icon::Play, "Running", false);
                    } else if not_extracted {
                        if ui.button("Extract").clicked() {
                            action.extract = Some(game.dir.clone());
                        }
                    } else {
                        action.launch = icon_button(ui, Icon::Play, "Launch", ready).clicked();
                    }
                    action.details = icon_button(ui, Icon::Info, "Details", true).clicked();
                    let favorite = tile.entry.stats.favorite;
                    let tip = if favorite {
                        "Remove from favorites"
                    } else {
                        "Add to favorites"
                    };
                    action.favorite =
                        icon_button(ui, Icon::Heart { filled: favorite }, tip, true).clicked();
                });
            });
        });
    action.rect = cell.response.rect;
    if tile.focused {
        ui.painter().rect_stroke(
            action.rect,
            8.0,
            egui::Stroke::new(3.0, egui::Color32::from_rgb(90, 170, 255)),
            egui::StrokeKind::Inside,
        );
    }
    action
}

/// A tile title that wraps to at most two lines and ends in an ellipsis, so a long title
/// cannot push the buttons down.
fn two_line_title(ui: &egui::Ui, title: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::single_section(
        title.to_owned(),
        egui::TextFormat {
            font_id: egui::TextStyle::Body.resolve(ui.style()),
            color: ui.visuals().strong_text_color(),
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_rows: 2,
        break_anywhere: true,
        overflow_character: Some('…'),
        ..Default::default()
    };
    job
}

fn encoding_label(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Auto => "Auto",
        Encoding::Cp932 => "cp932",
        Encoding::Utf8 => "UTF-8",
    }
}

const KEYBOARD: &[(egui::Key, Input)] = &[
    (egui::Key::ArrowUp, Input::Up),
    (egui::Key::ArrowDown, Input::Down),
    (egui::Key::ArrowLeft, Input::Left),
    (egui::Key::ArrowRight, Input::Right),
    (egui::Key::Enter, Input::A),
    (egui::Key::Escape, Input::B),
    (egui::Key::Backspace, Input::B),
    (egui::Key::R, Input::X),
    (egui::Key::D, Input::Y),
    (egui::Key::F5, Input::Start),
    (egui::Key::F11, Input::ToggleFullscreen),
    (egui::Key::B, Input::ToggleBlur),
];

fn pad_input(button: Button) -> Option<Input> {
    Some(match button {
        Button::DPadUp => Input::Up,
        Button::DPadDown => Input::Down,
        Button::DPadLeft => Input::Left,
        Button::DPadRight => Input::Right,
        Button::South => Input::A,
        Button::East => Input::B,
        Button::West => Input::X,
        Button::North => Input::Y,
        Button::Start => Input::Start,
        Button::Select => Input::Select,
        _ => return None,
    })
}

fn debounce_label(ms: u64) -> String {
    match ms {
        0 => "Off".to_owned(),
        ms => format!("{ms} ms"),
    }
}

fn dir_input(dir: nav::Dir) -> Input {
    match dir {
        nav::Dir::Up => Input::Up,
        nav::Dir::Down => Input::Down,
        nav::Dir::Left => Input::Left,
        nav::Dir::Right => Input::Right,
    }
}

enum PickerEvent {
    Pick(usize),
    Cancel,
}

/// The dropdown popup. It has no title bar: that bar's width is worked out apart from the
/// content and would not match the entries, so the title is a heading inside the box and
/// the box is as wide as its widest text.
fn picker_window(ctx: &egui::Context, list: &PickerList, selected: usize) -> Option<PickerEvent> {
    let mut event = None;
    egui::Window::new(list.title)
        .title_bar(false)
        .resizable(false)
        .auto_sized()
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let measure = |ui: &egui::Ui, text: &str, style: egui::TextStyle| {
                let font = style.resolve(ui.style());
                ui.painter()
                    .layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE)
                    .size()
                    .x
            };
            let entries = list
                .labels
                .iter()
                .map(String::as_str)
                .chain(["Cancel"])
                .map(|text| measure(ui, text, egui::TextStyle::Button))
                .fold(0.0, f32::max)
                + 2.0 * ui.spacing().button_padding.x;
            ui.set_width(entries.max(measure(ui, list.title, egui::TextStyle::Heading)));
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                ui.heading(list.title);
                ui.separator();
                for (i, label) in list.labels.iter().enumerate() {
                    if ui.selectable_label(i == selected, label).clicked() {
                        event = Some(PickerEvent::Pick(i));
                    }
                }
                if ui.button("Cancel").clicked() {
                    event = Some(PickerEvent::Cancel);
                }
            });
        });
    event
}

/// The last component of a path kept as text, for messages.
fn file_name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned())
}
