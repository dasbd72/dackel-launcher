mod covers;
mod details;
mod focus;
mod icons;

use covers::{BlurCache, CoverView};
use dackel_launcher::actions::{Actions, Exclusive};
use dackel_launcher::browse::{self, Card, Query, Sort};
use dackel_launcher::cursor::{Cursor, Page, Slot, Target};
use dackel_launcher::extract::{self, Encoding, Existing, Progress, Status};
use dackel_launcher::fetch::HttpFetch;
use dackel_launcher::fonts;
use dackel_launcher::info::{GameInfo, cover_path, load};
use dackel_launcher::jobs::{Cancelled, ExtractJob, JobStatus, Jobs, OverwriteCheck};
use dackel_launcher::launch::{Launcher, UmuSpawner, unix_now};
use dackel_launcher::library::{Game, Store, library_labels, resolve_library, scan_all, store_of};
use dackel_launcher::library_list::LibraryList;
use dackel_launcher::nav::{
    self, Command, Context, Control, DEBOUNCE_PRESETS_MS, DEFAULT_DEBOUNCE_MS, Input,
};
use dackel_launcher::pad::Pad;
use dackel_launcher::pickers::{
    self, Choice, ENCODINGS, Picker, PickerList, SORTS, Selection, debounce_label, encoding_label,
    sort_label,
};
use dackel_launcher::popup::{OVERWRITE_CHOICES, OverwritePrompt, PickTarget, Popup, ProtonPrompt};
use dackel_launcher::profile::{self, ExeState, Stats};
use dackel_launcher::proton::{self, ProtonChoice};
use dackel_launcher::reload::Infos;
use dackel_launcher::settings::{self, Settings};
use dackel_launcher::tuning::reset_prefix;
use dackel_launcher::window::{self, RestoreStep, WindowCommand, WindowState};
use details::{DetailsView, TuningEdit, details_ui, exe_choices};
use eframe::egui;
use focus::{FocusFrame, Item, bounds};
use icons::{Icon, icon_button};
use std::collections::HashSet;
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
            .with_app_id(APP_ID)
            .with_inner_size([1280.0, 720.0])
            .with_fullscreen(startup.fullscreen),
        ..Default::default()
    };
    eframe::run_native(
        "Dackel Launcher",
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

/// Points per second the details page scrolls with the stick pushed all the way.
const PAD_SCROLL_SPEED: f32 = 500.0;
/// Matches the desktop entry's `StartupWMClass`, so the menu entry and the window group together.
const APP_ID: &str = "io.github.dasbd72.DackelLauncher";
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

/// A launch waiting for the user to decide what to do about a missing Proton version.
#[derive(Clone)]
enum PromptOption {
    Newest,
    Another,
    Version(PathBuf),
}

struct App {
    /// The libraries in use, which could not be read, the filter and the "add a folder" form.
    libraries: LibraryList,
    /// The popup that is open, if any.
    popup: Option<Popup>,
    /// How far down the details page is scrolled, in points.
    details_scroll: f32,
    /// The details page's Launch options form.
    tuning_edit: Option<TuningEdit>,
    view: View,
    encoding: Encoding,
    message: Option<String>,
    /// Store info for each game, and the reloads of it running in the background.
    infos: Infos,
    launcher: Launcher<UmuSpawner>,
    screen: Screen,
    query: Query,
    /// The gamepad: its inputs, the stick's scroll request, the press debounce.
    pad: Pad,
    /// Controller focus, and the selected row of the open popup list.
    cursor: Cursor,
    settings: Settings,
    settings_path: Option<PathBuf>,
    /// Fullscreen, UI scale, and bringing the window back after a game; its changes are sent
    /// to egui at the end of the frame.
    window: WindowState,
    /// Extractions and overwrite checks running in the background, by game folder.
    jobs: Jobs,
    blur_cache: BlurCache,
    /// Lets background work ask the window to redraw; set on the first frame.
    repaint: Option<egui::Context>,
    started: std::time::Instant,
}

impl App {
    fn new(library: PathBuf, startup: Startup) -> Self {
        // A folder listed twice (say, the one beside the program added again) shows once.
        let libraries = LibraryList::new(library, &startup.settings.libraries);
        let mut startup = startup;
        let added = libraries.added().to_vec();
        let tidy = startup.settings.libraries != added;
        startup.settings.libraries = added;
        let mut app = App {
            libraries,
            popup: None,
            details_scroll: 0.0,
            tuning_edit: None,
            view: View::Missing,
            encoding: Encoding::Auto,
            message: None,
            infos: Infos::new(Arc::new(HttpFetch)),
            launcher: Launcher::new(UmuSpawner),
            screen: Screen::Grid,
            query: Query {
                text: String::new(),
                favorites_only: false,
                sort: Sort::Title,
            },
            pad: Pad::new(startup.settings.debounce_ms.unwrap_or(DEFAULT_DEBOUNCE_MS)),
            cursor: Cursor::new(),
            settings: startup.settings,
            settings_path: startup.settings_path,
            window: WindowState::new(
                startup.fullscreen,
                startup.scale,
                startup.minimize_on_launch,
            ),
            jobs: Jobs::new(),
            blur_cache: BlurCache::default(),
            repaint: None,
            started: std::time::Instant::now(),
        };
        if tidy {
            app.save_settings();
        }
        app.rescan();
        app
    }

    fn rescan(&mut self) {
        self.close_chooser();
        let extracting: HashSet<String> = self.jobs.keys().cloned().collect();
        let scans = scan_all(self.libraries.roots());
        self.libraries.set_offline(
            scans
                .iter()
                .filter(|s| s.games.is_err())
                .map(|s| s.root.clone())
                .collect(),
        );
        let primary_error = scans
            .first()
            .and_then(|s| s.games.as_ref().err())
            .map(|err| (err.kind(), err.to_string()));
        let games: Vec<Game> = scans
            .into_iter()
            .filter_map(|s| s.games.ok())
            .flatten()
            .collect();
        let nothing_readable = games.is_empty() && self.libraries.all_offline();
        self.view = match primary_error {
            // Nothing readable and nothing configured: the empty state, with a way to add one.
            Some((ErrorKind::NotFound, _)) if nothing_readable && self.libraries.count() == 1 => {
                View::Missing
            }
            // A configured folder is missing: name it, since the system error does not.
            Some((ErrorKind::NotFound, _)) if nothing_readable => {
                let missing: Vec<String> = self
                    .libraries
                    .offline()
                    .iter()
                    .map(|root| root.display().to_string())
                    .collect();
                View::Error(format!("Library not found: {}", missing.join(", ")))
            }
            Some((_, msg)) if nothing_readable => View::Error(msg),
            _ => View::Games(
                games
                    .into_iter()
                    .map(|game| {
                        // A folder being extracted is half built: do not detect (and pin) an
                        // exe from it yet. The rescan after the job finishes does that.
                        let state = if extracting.contains(&game.dir_key()) {
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
        if !self.libraries.offline().is_empty()
            && self.message.is_none()
            && matches!(self.view, View::Games(_))
        {
            let names: Vec<String> = self
                .libraries
                .offline()
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
                Some(info) => self.infos.set(game.key(), info),
                None => self.infos.reload(game),
            }
        }
    }

    fn collect_reloads(&mut self) {
        // A reload may have replaced the cover, so any blurred copy of it is stale.
        if self.infos.collect() > 0 {
            self.blur_cache.clear();
        }
    }

    /// Looks for files the extraction would overwrite; the extraction starts at once when
    /// there are none, otherwise the user is asked first.
    fn extract_game(&mut self, dir: &std::path::Path) {
        let id = dir.to_string_lossy().into_owned();
        if self.jobs.in_progress(&id) {
            return;
        }
        let Some(ctx) = self.repaint.clone() else {
            return;
        };
        let check = OverwriteCheck::start(dir.to_path_buf(), self.encoding, move || {
            ctx.request_repaint()
        });
        self.jobs.start_check(id.clone(), check);
        self.message = Some(format!("Checking {}...", file_name_of(&id)));
    }

    /// Starts extracting in the background; progress shows on the tile and details page.
    fn begin_extraction(&mut self, dir: &std::path::Path, existing: Existing) {
        let id = dir.to_string_lossy().into_owned();
        if self.jobs.is_extracting(&id) {
            return;
        }
        let Some(ctx) = self.repaint.clone() else {
            return;
        };
        let job = ExtractJob::start_with(dir.to_path_buf(), self.encoding, existing, move || {
            ctx.request_repaint()
        });
        self.jobs.start_extraction(id.clone(), job);
        self.message = Some(format!("Extracting {}...", file_name_of(&id)));
    }

    /// Asks a running extraction (or the check before it) to stop; an extraction removes
    /// what it created once it has.
    fn cancel_extraction(&mut self, id: &str) {
        match self.jobs.cancel(id) {
            Cancelled::Check => {
                self.message = Some(format!("Cancelled extracting {}", file_name_of(id)));
            }
            Cancelled::Extraction => {
                self.message = Some(format!("Cancelling {}...", file_name_of(id)));
            }
            Cancelled::Nothing => {}
        }
    }

    /// Acts on finished overwrite checks: extract at once, or ask first.
    fn poll_checks(&mut self) {
        for (id, result) in self.jobs.finished_checks() {
            let dir = PathBuf::from(&id);
            match result {
                Ok(files) if files.is_empty() => self.begin_extraction(&dir, Existing::Overwrite),
                Ok(files) => {
                    self.popup = Some(Popup::Overwrite(OverwritePrompt {
                        dir,
                        files,
                        search: String::new(),
                    }));
                    // Cancel is the first choice, so a stray A press overwrites nothing.
                    self.cursor.reset_list();
                    self.message = None;
                }
                Err(err) => {
                    self.message = Some(format!("Extract failed ({}): {err}", file_name_of(&id)));
                }
            }
        }
    }

    fn answer_overwrite(&mut self, choice: usize) {
        let Some(prompt) = self.take_overwrite_prompt() else {
            return;
        };
        match choice {
            1 => self.begin_extraction(&prompt.dir, Existing::Overwrite),
            2 => self.begin_extraction(&prompt.dir, Existing::Skip),
            _ => {}
        }
    }

    /// Collects finished extractions. Runs from `logic`, so it works while minimized.
    fn poll_extractions(&mut self) {
        self.poll_checks();
        let finished = self.jobs.finished_extractions();
        if finished.is_empty() {
            return;
        }
        for (id, progress, status) in finished {
            self.message = Some(match status {
                JobStatus::Finished(Ok(())) if progress.files_skipped > 0 => format!(
                    "Extracted {id}: {} new, {} skipped",
                    progress.files_done, progress.files_skipped
                ),
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
        if entry.state.is_err() {
            return;
        }
        let dir = entry.game.dir.clone();
        let pinned = profile::read_tuning(&dir).ok().and_then(|t| t.proton);
        match proton::resolve_for_game_default(pinned.as_deref()) {
            Err(err) => self.message = Some(format!("Launch failed: {err}")),
            Ok(ProtonChoice::Use(path)) => {
                self.start_game(&dir, path.as_deref());
            }
            Ok(ProtonChoice::Mismatch { pinned, newest }) => {
                self.cursor.reset_list();
                // Shown when a game's pinned Proton is not installed on this machine.
                self.popup = Some(Popup::Proton(ProtonPrompt {
                    dir,
                    pinned,
                    newest,
                    choosing: false,
                }));
            }
        }
    }

    /// Spawns the game with the given Proton (`None`: umu's own choice).
    fn start_game(&mut self, dir: &Path, proton: Option<&Path>) -> bool {
        let View::Games(entries) = &self.view else {
            return false;
        };
        let Some(entry) = entries.iter().find(|e| e.game.dir == dir) else {
            return false;
        };
        let Ok(state) = &entry.state else {
            return false;
        };
        match self
            .launcher
            .start(&entry.game.dir, state, proton, unix_now())
        {
            Ok(()) => {
                // Hand the screen to the game; minimize rather than close.
                self.window.game_started();
                self.message = Some(format!("Launched {}", entry.game.id));
                true
            }
            Err(err) => {
                self.message = Some(format!("Launch failed: {err}"));
                false
            }
        }
    }

    /// The user's answer to the missing-Proton prompt: launch with `proton`, and once the
    /// game has started, pin that version instead of the missing one.
    fn launch_with_other_proton(&mut self, proton: Option<PathBuf>) {
        let Some(prompt) = self.take_proton_prompt() else {
            return;
        };
        if self.start_game(&prompt.dir, proton.as_deref()) {
            let name = proton
                .as_deref()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned());
            if let Err(err) = profile::set_proton_pin(&prompt.dir, name.as_deref()) {
                self.message = Some(format!("Launched, but the Proton pin was not saved: {err}"));
            }
        }
    }

    /// The choices the missing-Proton prompt offers right now.
    fn prompt_options(&self) -> Vec<(PromptOption, String)> {
        let Some(prompt) = self.popup.as_ref().and_then(Popup::as_proton) else {
            return Vec::new();
        };
        let name = |p: &Path| {
            p.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        };
        if prompt.choosing {
            return proton::installed(&proton::compat_dir())
                .unwrap_or_default()
                .into_iter()
                .map(|p| {
                    let label = name(&p);
                    (PromptOption::Version(p), label)
                })
                .collect();
        }
        let newest = match &prompt.newest {
            Some(path) => format!("Use newest anyway ({})", name(path)),
            None => "Launch with umu's default anyway".to_owned(),
        };
        vec![
            (PromptOption::Newest, newest),
            (PromptOption::Another, "Pick another...".to_owned()),
        ]
    }

    fn answer_prompt(&mut self, option: PromptOption) {
        match option {
            PromptOption::Newest => {
                let newest = self
                    .popup
                    .as_ref()
                    .and_then(Popup::as_proton)
                    .and_then(|p| p.newest.clone());
                self.launch_with_other_proton(newest);
            }
            PromptOption::Another => {
                if let Some(prompt) = self.popup.as_mut().and_then(Popup::as_proton_mut) {
                    prompt.choosing = true;
                }
                self.cursor.reset_list();
            }
            PromptOption::Version(path) => self.launch_with_other_proton(Some(path)),
        }
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
            .filter(|&i| self.libraries.shows_slot(entries[i].game.slot))
            .collect()
    }

    /// Carries out what the toolbar asked for, before the rest of the frame is drawn.
    fn apply_toolbar(&mut self, actions: &Actions) {
        if actions.fullscreen {
            self.toggle_fullscreen();
        }
        if actions.toggle_blur {
            self.toggle_blur();
        }
        if let Some(ms) = actions.debounce {
            self.set_debounce(ms);
        }
        if let Some(scale) = actions.scale {
            self.set_ui_scale(scale);
        }
    }

    /// Carries out what the frame asked for, once everything has been drawn: the popups'
    /// answers, the clicks on tiles, the details page, then the one action that changes the
    /// game list. Drawing never changes the app itself; this is the only place that does.
    fn apply_actions(&mut self, mut actions: Actions) {
        if let Some((picker, index)) = actions.pick.take() {
            self.close_picker();
            self.pick(picker, index);
        }
        if actions.cancel_picker {
            self.close_picker();
        }
        if let Some(id) = actions.select.take() {
            self.cursor.select_game(id);
        }
        for id in std::mem::take(&mut actions.cancel_extract) {
            self.cancel_extraction(&id);
        }
        if let Some(i) = actions.launch.take() {
            self.launch_game(i);
        }
        if actions.close_chooser {
            self.close_chooser();
        }
        if let Some(id) = actions.open_details.take() {
            self.open_details_page(id);
        }
        if actions.back {
            self.close_details_page();
        }
        if let Some(id) = actions.favorite.take() {
            self.toggle_favorite(&id);
        }
        if let Some(game) = actions.reload.take() {
            self.infos.reload(game);
        }
        match actions.take_exclusive() {
            Some(Exclusive::ChooseExe { dir, exe }) => match profile::choose(&dir, &exe) {
                Ok(()) => self.rescan(),
                Err(err) => self.view = View::Error(err.to_string()),
            },
            Some(Exclusive::Extract(dir)) => self.extract_game(&dir),
            // Applied here and not while drawing: the central panel borrows the view.
            Some(Exclusive::OpenLibraries) => self.popup = Some(Popup::Libraries),
            Some(Exclusive::CreateLibrary) => self.create_library(),
            Some(Exclusive::Rescan) => self.rescan(),
            None => {}
        }
    }

    /// The name to show beside a game's library; `None` with a single library.
    fn library_label(&self, game: &Game) -> Option<String> {
        self.libraries.label_of(game.slot)
    }

    /// Keeps the cursor on a game that is on screen, so focus is never lost.
    fn normalize_focus(&mut self, visible: &[usize]) {
        let View::Games(entries) = &self.view else {
            return;
        };
        let keys: Vec<String> = visible.iter().map(|&i| entries[i].game.key()).collect();
        self.cursor.keep_on_game(&keys);
    }

    /// The entry controller commands act on: the open details page, else the cursor.
    fn target_index(&self) -> Option<usize> {
        let View::Games(entries) = &self.view else {
            return None;
        };
        let id = match &self.screen {
            Screen::Details(id) => Some(id.as_str()),
            Screen::Grid => self.cursor.game(),
        }?;
        entries.iter().position(|e| e.game.key() == id)
    }

    fn context(&self) -> Context {
        if self.popup.is_some() {
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
        self.settings.fullscreen = Some(self.window.toggle_fullscreen());
        self.save_settings();
    }

    fn set_ui_scale(&mut self, scale: f32) {
        self.window.set_scale(scale);
        self.settings.ui_scale = Some(scale);
        self.save_settings();
    }

    /// Saves the Launch options form, unpins Proton, or resets the prefix.
    fn apply_tuning(&mut self, result: TuningResult) {
        let TuningResult {
            dir,
            id,
            save,
            unpin,
            reset_prefix: reset,
        } = result;
        if let Some((env, args)) = save {
            self.message = Some(match profile::set_tuning(&dir, env, &args) {
                Ok(()) => format!("Saved launch options for {id}"),
                Err(err) => format!("Could not save launch options: {err}"),
            });
        }
        if unpin {
            self.message = Some(match profile::set_proton_pin(&dir, None) {
                Ok(()) => format!("{id} will pin the Proton it launches with next"),
                Err(err) => format!("Could not unpin Proton: {err}"),
            });
        }
        if reset {
            self.message = Some(if self.launcher.is_running(&dir) {
                "Close the game before resetting its prefix".to_owned()
            } else {
                match reset_prefix(&dir) {
                    Ok(()) => format!("Reset the Wine prefix of {id}"),
                    Err(err) => format!("Could not reset the prefix: {err}"),
                }
            });
        }
    }

    /// Asks what to do when a game's pinned Proton is not installed here.
    fn proton_prompt_window(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.popup.as_ref().and_then(Popup::as_proton) else {
            return;
        };
        let options = self.prompt_options();
        let mut answer: Option<PromptOption> = None;
        let mut cancel = false;
        egui::Window::new("Proton version missing")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let game = prompt
                    .dir
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                ui.label(format!(
                    "{game} was set up with {}, which is not installed on this machine.",
                    prompt.pinned
                ));
                if options.is_empty() {
                    ui.weak("No GE-Proton versions are installed.");
                }
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    for (i, (option, label)) in options.iter().enumerate() {
                        if ui
                            .selectable_label(i == self.cursor.list_sel(), label)
                            .clicked()
                        {
                            answer = Some(option.clone());
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if let Some(option) = answer {
            self.answer_prompt(option);
        } else if cancel {
            self.popup = None;
        }
    }

    /// Lists the files an extraction would overwrite and asks what to do about them.
    fn overwrite_prompt_window(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.popup.as_mut().and_then(Popup::as_overwrite_mut) else {
            return;
        };
        let needle = prompt.search.to_lowercase();
        let shown: Vec<&PathBuf> = prompt
            .files
            .iter()
            .filter(|f| needle.is_empty() || f.to_string_lossy().to_lowercase().contains(&needle))
            .collect();
        let game = prompt
            .dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let total = prompt.files.len();
        let stick = self.pad.scroll() * PAD_SCROLL_SPEED * ctx.input(|i| i.stable_dt).min(0.1);
        let mut answer = None;
        egui::Window::new("Overwrite files?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(520.0_f32.min(ctx.content_rect().width() - 64.0));
                ui.label(format!(
                    "Extracting {game} would replace {total} existing file{}. Files not in the zip are not touched.",
                    if total == 1 { "" } else { "s" }
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut prompt.search)
                        .desired_width(f32::INFINITY)
                        .hint_text("Search paths"),
                );
                if !needle.is_empty() {
                    ui.weak(format!("{} of {total} files", shown.len()));
                }
                let row = ui.text_style_height(&egui::TextStyle::Body);
                egui::ScrollArea::vertical()
                    .max_height(row * 12.0)
                    .auto_shrink([false, true])
                    .show_rows(ui, row, shown.len(), |ui, range| {
                        ui.scroll_with_delta(egui::vec2(0.0, stick));
                        for path in &shown[range] {
                            ui.add(egui::Label::new(path.display().to_string()).truncate());
                        }
                    });
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    for (i, label) in OVERWRITE_CHOICES.iter().enumerate() {
                        if ui.selectable_label(i == self.cursor.list_sel(), *label).clicked() {
                            answer = Some(i);
                        }
                    }
                });
            });
        if let Some(choice) = answer {
            self.answer_overwrite(choice);
        }
    }

    /// The manager for the added libraries: list them, add a typed path, remove one.
    fn libraries_window(&mut self, ctx: &egui::Context) {
        let mut remove: Option<usize> = None;
        let mut add = false;
        let mut close = false;
        let rows: Vec<(usize, PathBuf, bool)> = self
            .libraries
            .listed()
            .into_iter()
            .map(|(i, root)| (i, root.clone(), self.libraries.is_offline(root)))
            .collect();
        let error = self.libraries.error().map(str::to_owned);
        egui::Window::new("Libraries")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                // The library beside the program is listed only when it was found, and it
                // cannot be removed. Added libraries are listed even when offline.
                if rows.is_empty() {
                    ui.weak("No libraries found.");
                }
                for (i, root, offline) in &rows {
                    let i = *i;
                    ui.horizontal(|ui| {
                        ui.label(root.display().to_string());
                        if *offline {
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
                        egui::TextEdit::singleline(self.libraries.input_mut())
                            .desired_width(420.0)
                            .hint_text("/path/to/games"),
                    );
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Add").clicked() || enter {
                        add = true;
                    }
                });
                if let Some(err) = &error {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if add && let Ok(path) = self.libraries.add_typed() {
            self.settings.libraries.push(path);
            self.save_settings();
            self.rescan();
        }
        if let Some(removed) = remove.and_then(|i| self.libraries.remove(i)) {
            let gone = removed.root;
            self.settings.libraries.retain(|l| *l != gone);
            // Later slots move up, so keys, the filter and the open page would point at the
            // wrong games: go back to the grid with no filter (the list cleared the filter).
            self.close_details_page();
            // Stop extracting into a folder that is no longer listed anywhere.
            if !removed.still_listed {
                for key in self.jobs.keys_in(&gone) {
                    self.cancel_extraction(&key);
                }
            }
            self.save_settings();
            self.rescan();
        }
        if close {
            if matches!(self.popup, Some(Popup::Libraries)) {
                self.popup = None;
            }
            self.libraries.clear_error();
        }
    }

    fn set_debounce(&mut self, ms: u64) {
        self.pad.set_debounce_window(ms);
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
        self.details_scroll = 0.0;
        self.cursor.open_details();
    }

    /// Back to the library; focus returns to the game that was selected.
    fn close_details_page(&mut self) {
        self.screen = Screen::Grid;
        self.cursor.close_details();
    }

    /// The values the toolbar dropdowns choose between, as they are now.
    fn selection(&self) -> Selection<'_> {
        Selection {
            sort: self.query.sort,
            encoding: self.encoding,
            scale: self.window.scale(),
            debounce_ms: self.pad.debounce_window(),
            libraries: self.libraries.roots(),
            library_filter: self.libraries.filter(),
        }
    }

    fn picker_list(&self, picker: Picker) -> PickerList {
        pickers::list(picker, &self.selection())
    }

    /// The dropdown that is open, if it is the popup that is open.
    fn picker_open(&self) -> Option<Picker> {
        match self.popup {
            Some(Popup::Picker(picker)) => Some(picker),
            _ => None,
        }
    }

    fn close_picker(&mut self) {
        if self.picker_open().is_some() {
            self.popup = None;
        }
    }

    fn overwrite_prompt_open(&self) -> bool {
        self.popup.as_ref().and_then(Popup::as_overwrite).is_some()
    }

    /// Takes the overwrite prompt, closing it. Any other popup is left open.
    fn take_overwrite_prompt(&mut self) -> Option<OverwritePrompt> {
        match self.popup.take() {
            Some(Popup::Overwrite(prompt)) => Some(prompt),
            other => {
                self.popup = other;
                None
            }
        }
    }

    fn proton_prompt_open(&self) -> bool {
        self.popup.as_ref().and_then(Popup::as_proton).is_some()
    }

    /// Takes the missing-Proton prompt, closing it. Any other popup is left open.
    fn take_proton_prompt(&mut self) -> Option<ProtonPrompt> {
        match self.popup.take() {
            Some(Popup::Proton(prompt)) => Some(prompt),
            other => {
                self.popup = other;
                None
            }
        }
    }

    fn open_picker(&mut self, picker: Picker) {
        self.cursor.set_list(self.picker_list(picker).current);
        self.popup = Some(Popup::Picker(picker));
    }

    fn pick(&mut self, picker: Picker, index: usize) {
        match pickers::pick(picker, index, self.libraries.count()) {
            Some(Choice::Sort(sort)) => self.query.sort = sort,
            Some(Choice::Encoding(encoding)) => self.encoding = encoding,
            Some(Choice::LibraryFilter(slot)) => self.libraries.set_filter(slot),
            Some(Choice::Debounce(ms)) => self.set_debounce(ms),
            Some(Choice::Scale(scale)) => self.set_ui_scale(scale),
            None => {}
        }
    }

    /// The game the exe chooser is open for, if it is the popup that is open.
    fn chooser_index(&self) -> Option<usize> {
        match self.popup {
            Some(Popup::Chooser(index)) => Some(index),
            _ => None,
        }
    }

    fn close_chooser(&mut self) {
        if self.chooser_index().is_some() {
            self.popup = None;
        }
    }

    fn open_chooser(&mut self, index: usize) {
        self.popup = Some(Popup::Chooser(index));
        self.cursor.reset_list();
    }

    fn apply(&mut self, command: Command) {
        match command {
            Command::Move(dir) => self.cursor.move_toward(dir),
            Command::Primary => {
                // A control with focus is pressed; the library's dropdowns open a list.
                if let Some(control) = self.cursor.control() {
                    match Picker::for_control(control) {
                        Some(picker) => self.open_picker(picker),
                        None => self.cursor.press(control),
                    }
                    return;
                }
                let Some(i) = self.target_index() else { return };
                let View::Games(entries) = &self.view else {
                    return;
                };
                let entry = &entries[i];
                let dir = entry.game.dir.clone();
                if self.launcher.is_running(&dir) || self.jobs.in_progress(&entry.game.dir_key()) {
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
                if matches!(self.screen, Screen::Grid) && self.cursor.control().is_some() {
                    return;
                }
                if let (Some(i), View::Games(entries)) = (self.target_index(), &self.view) {
                    let game = entries[i].game.clone();
                    // While a game is extracting, this button cancels that instead.
                    if self.jobs.in_progress(&game.dir_key()) {
                        self.cancel_extraction(&game.dir_key());
                    } else {
                        self.infos.reload(game);
                    }
                }
            }
            Command::OpenDetails => {
                if self.cursor.control().is_none()
                    && let Some(id) = self.cursor.game().map(str::to_owned)
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
                let len = self.popup.as_ref().map_or(0, |popup| {
                    popup.list_len(
                        |game| match &self.view {
                            View::Games(entries) => {
                                entries.get(game).map_or(0, |e| e.candidates.len())
                            }
                            _ => 0,
                        },
                        |picker| self.picker_list(picker).labels.len(),
                        |_| self.prompt_options().len(),
                    )
                });
                self.cursor.move_list(delta, len);
            }
            Command::ChooserPick => {
                let Some(target) = self
                    .popup
                    .as_ref()
                    .map(|popup| popup.pick(self.cursor.list_sel()))
                else {
                    return;
                };
                match target {
                    PickTarget::Overwrite(choice) => self.answer_overwrite(choice),
                    PickTarget::Proton(index) => {
                        if let Some((option, _)) = self.prompt_options().into_iter().nth(index) {
                            self.answer_prompt(option);
                        }
                    }
                    PickTarget::Picker(picker, index) => {
                        self.popup = None;
                        self.pick(picker, index);
                    }
                    PickTarget::Exe { chooser, candidate } => {
                        let View::Games(entries) = &self.view else {
                            return;
                        };
                        let picked = entries.get(chooser).and_then(|e| {
                            Some((e.game.dir.clone(), e.candidates.get(candidate)?.clone()))
                        });
                        if let Some((dir, exe)) = picked {
                            match profile::choose(&dir, &exe) {
                                Ok(()) => self.rescan(),
                                Err(err) => self.view = View::Error(err.to_string()),
                            }
                        }
                    }
                    PickTarget::Nothing => {}
                }
            }
            Command::ChooserCancel => {
                self.popup = None;
            }
            Command::ToggleFullscreen => self.toggle_fullscreen(),
            Command::ToggleBlur => self.toggle_blur(),
        }
    }

    /// Gamepad and keyboard presses since the last frame.
    fn poll_inputs(&mut self, ctx: &egui::Context) -> Vec<Input> {
        let mut inputs = Vec::new();
        let on_details = self.context() == Context::Details;
        let now_ms = (ctx.input(|i| i.time) * 1000.0) as u64;
        // Ignore the pad while another window (a running game) has focus.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        // On the details page, pushing the stick up or down scrolls instead of moving focus.
        let scrolls = on_details || self.popup.as_ref().is_some_and(Popup::scrolls_with_stick);
        if let Some(pad_inputs) = self.pad.poll(now_ms, focused, scrolls) {
            inputs.extend(pad_inputs);
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        if !ctx.egui_wants_keyboard_input() {
            ctx.input(|i| {
                for (key, input) in KEYBOARD {
                    if i.key_pressed(*key) {
                        inputs.push(*input);
                        if matches!(input, Input::Up | Input::Down | Input::Left | Input::Right)
                            && let Some(hint) = self.pad.arrow_pressed()
                        {
                            self.message = Some(hint.to_owned());
                        }
                    }
                }
            });
        }
        let now_ms = self.started.elapsed().as_millis() as u64;
        self.pad.debounced(now_ms, inputs)
    }

    /// Only reached from the "Create it" button, which is shown only when it is allowed.
    fn create_library(&mut self) {
        match std::fs::create_dir_all(self.libraries.beside()) {
            Ok(()) => self.rescan(),
            Err(err) => self.view = View::Error(err.to_string()),
        }
    }
}

impl eframe::App for App {
    /// Tab would move egui's own widget focus, which fights the controller focus ring.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.events.retain(|event| {
            !matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Tab,
                    ..
                }
            )
        });
    }

    /// Runs before every frame and, unlike `ui`, also while the window is minimized.
    /// Watching for a game's exit has to happen here, or play time would only be saved
    /// once the launcher is brought back on screen.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.repaint.is_none() {
            self.repaint = Some(ctx.clone());
        }
        self.poll_extractions();
        if self.jobs.any() {
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
        }
        // Bring the launcher back once the game is gone. Retried, because the window
        // manager may refuse while the game's window is still the active one.
        let step = self
            .window
            .after_game_poll(now_ms, was_running, running, || {
                ctx.input(|i| i.viewport().minimized) == Some(true)
            });
        if let Some(step) = step {
            if step == RestoreStep::Request {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
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
        self.cursor.keep_on_page(match self.screen {
            Screen::Grid => Page::Grid,
            Screen::Details(_) => Page::Details,
        });
        let mut frame = FocusFrame::new(
            self.cursor.control(),
            self.cursor.take_press(),
            self.cursor.scroll_requested(),
        );
        // Apply a requested zoom; otherwise notice egui's own Ctrl +/- zoom and remember it.
        if let Some(zoom) = self.window.take_zoom_request() {
            ctx.set_zoom_factor(zoom);
        } else if self.window.zoom_differs(ctx.zoom_factor()) {
            self.set_ui_scale(ctx.zoom_factor());
        }

        let mut actions = Actions::default();
        let mut tuning_action: Option<TuningResult> = None;
        // The pin is re-read each frame the page is open, so the first launch shows it at once.
        let tuning_pin = match (&self.screen, &self.view) {
            (Screen::Details(key), View::Games(entries)) => entries
                .iter()
                .find(|e| e.game.key() == *key)
                .and_then(|e| profile::read_tuning(&e.game.dir).ok())
                .and_then(|t| t.proton),
            _ => None,
        };
        self.collect_reloads();
        if self.infos.any_busy() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        let current_scale = self.window.scale();
        let system_scale = ui.ctx().native_pixels_per_point().unwrap_or(1.0);
        let mut blur_on = self.settings.blur_covers;
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                let response = ui.button("Rescan");
                if frame.pressed(ui, Control::Rescan, &response, true) {
                    actions.rescan = true;
                }
                let response = ui.button("Libraries");
                if frame.pressed(ui, Control::Libraries, &response, true) {
                    self.popup = Some(Popup::Libraries);
                }
                let response = ui.button("Fullscreen");
                actions.fullscreen = frame.pressed(ui, Control::Fullscreen, &response, true);
                let response = ui
                    .toggle_value(&mut blur_on, "Blur")
                    .on_hover_text("Blur all images (key: B)");
                frame.register(ui, Control::Blur, &response);
                if frame.activated(Control::Blur) {
                    blur_on = !blur_on;
                }
                let response = egui::ComboBox::from_id_salt("ui-scale")
                    .selected_text(window::scale_label(current_scale, system_scale))
                    .show_ui(ui, |ui| {
                        for preset in window::SCALE_PRESETS {
                            let on = (current_scale - preset).abs() < 1e-3;
                            if ui.selectable_label(on, format!("{preset:.2}")).clicked() {
                                actions.scale = Some(preset);
                            }
                        }
                    })
                    .response;
                frame.register(ui, Control::Scale, &response);
                let response = egui::ComboBox::from_id_salt("debounce")
                    .selected_text(format!(
                        "Debounce {}",
                        debounce_label(self.pad.debounce_window())
                    ))
                    .show_ui(ui, |ui| {
                        for ms in DEBOUNCE_PRESETS_MS {
                            let on = ms == self.pad.debounce_window();
                            if ui.selectable_label(on, debounce_label(ms)).clicked() {
                                actions.debounce = Some(ms);
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
                let response = ui.button("Quit");
                if frame.pressed(ui, Control::Quit, &response, true) {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
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
                    if self.libraries.count() > 1 {
                        let labels = library_labels(self.libraries.roots());
                        let shown = self
                            .libraries
                            .filter()
                            .and_then(|slot| labels.get(slot))
                            .map_or("All libraries", String::as_str);
                        let response = egui::ComboBox::from_id_salt("library-filter")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        self.libraries.filter().is_none(),
                                        "All libraries",
                                    )
                                    .clicked()
                                {
                                    self.libraries.set_filter(None);
                                }
                                for (slot, label) in labels.iter().enumerate() {
                                    let on = self.libraries.filter() == Some(slot);
                                    if ui.selectable_label(on, label).clicked() {
                                        self.libraries.set_filter(Some(slot));
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

        actions.toggle_blur = blur_on != self.settings.blur_covers;
        self.apply_toolbar(&actions);

        let hint = match self.context() {
            Context::Grid => {
                "D-pad Move   A Open / Press   B Reset filters   X Reload / Cancel extract   Y Details   Start Rescan   Select Favorites only"
            }
            Context::Details => {
                "D-pad Move   Left stick Scroll   A Press   B Back   X Reload / Cancel extract   Y Favorite   Start Rescan"
            }
            Context::Chooser => "Up/Down Select   A Pick   B Cancel",
        };
        egui::Panel::bottom("hints").show(ui, |ui| {
            ui.weak(hint);
        });

        egui::CentralPanel::default().show(ui, |ui| match (&self.screen, &self.view) {
            (_, View::Missing) => {
                if self.libraries.can_create() {
                    ui.heading("Library not found");
                    ui.label(format!(
                        "{} does not exist.",
                        self.libraries.beside().display()
                    ));
                    let response = ui.button("Create it");
                    if frame.pressed(ui, Control::CreateLibrary, &response, true) {
                        actions.create_library = true;
                    }
                } else {
                    ui.heading("No library yet");
                    ui.label("Add a folder that contains your game folders to see them here.");
                }
                let response = ui.button("Add a library folder");
                if frame.pressed(ui, Control::AddLibrary, &response, true) {
                    actions.open_libraries = true;
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
                    error: self.infos.error(id),
                    busy: self.infos.is_busy(id),
                    running: self.launcher.is_running(&entry.game.dir),
                    extracting: self.jobs.progress(&entry.game.dir_key()),
                    dir: &entry.game.dir,
                    proton_pin: tuning_pin.as_deref(),
                };
                if self.tuning_edit.as_ref().is_none_or(|e| e.key != *id) {
                    self.tuning_edit = Some(TuningEdit::load(id.clone(), &entry.game.dir));
                }
                let Some(edit) = self.tuning_edit.as_mut() else {
                    return;
                };
                // Left stick: scroll at up to SCROLL_SPEED points per second.
                let dt = ui.ctx().input(|i| i.stable_dt).min(0.1);
                self.details_scroll =
                    (self.details_scroll - self.pad.scroll() * PAD_SCROLL_SPEED * dt).max(0.0);
                let action = details_ui(ui, &view, &mut frame, edit, &mut self.details_scroll);
                tuning_action = Some(TuningResult {
                    dir: entry.game.dir.clone(),
                    id: entry.game.id.clone(),
                    save: action.save_tuning,
                    unpin: action.unpin_proton,
                    reset_prefix: action.reset_prefix,
                });
                actions.back = action.back;
                if action.reload {
                    actions.reload = Some(entry.game.clone());
                }
                if action.extract {
                    actions.details_extract = Some(entry.game.dir.clone());
                }
                if action.cancel_extract {
                    actions.cancel_extract.push(entry.game.dir_key());
                }
                if action.launch {
                    actions.launch = entries.iter().position(|e| &e.game.key() == id);
                }
                if action.toggle_favorite {
                    actions.favorite = Some(id.clone());
                }
                if let Some(exe) = action.choose_exe {
                    actions.details_choice = Some((entry.game.dir.clone(), exe));
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
                                        error: self.infos.error(&entry.game.key()),
                                        busy: self.infos.is_busy(&entry.game.key()),
                                        running: self.launcher.is_running(&entry.game.dir),
                                        extracting: self.jobs.progress(&entry.game.dir_key()),
                                        focused: self.cursor.control().is_none()
                                            && self.cursor.game()
                                                == Some(entry.game.key().as_str()),
                                        cover_file,
                                        cover_view,
                                    };
                                    let action = tile_ui(ui, &tile);
                                    frame.items.push(Item {
                                        target: Target::Game(entry.game.key().clone()),
                                        rect: action.rect,
                                    });
                                    if tile.focused && self.cursor.scroll_requested() {
                                        ui.scroll_to_rect(action.rect.expand(8.0), None);
                                    }
                                    if action.clicked
                                        || action.launch
                                        || action.details
                                        || action.favorite
                                        || action.extract.is_some()
                                    {
                                        actions.select = Some(entry.game.key().clone());
                                    }
                                    if let Some(dir) = action.extract {
                                        actions.extract = Some(dir);
                                    }
                                    if action.launch {
                                        actions.launch = Some(i);
                                    }
                                    if action.cancel_extract {
                                        actions.cancel_extract.push(entry.game.dir_key());
                                    }
                                    if action.favorite {
                                        actions.favorite = Some(entry.game.key().clone());
                                    }
                                    if action.details {
                                        actions.open_details = Some(entry.game.key().clone());
                                    }
                                    // One click only moves the focus (done above); two open the
                                    // details page, which works for any game, extracted or not.
                                    if action.double_clicked {
                                        actions.open_details = Some(entry.game.key().clone());
                                    }
                                }
                            });
                        }
                    });
            }
        });

        if let (Some(i), View::Games(entries)) = (self.chooser_index(), &self.view)
            && let Some(entry) = entries.get(i)
        {
            egui::Window::new(format!("Choose exe for {}", entry.game.id))
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    if entry.candidates.is_empty() {
                        ui.label("No exe found in this folder.");
                    }
                    if let Some(candidate) =
                        exe_choices(ui, &entry.candidates, Some(self.cursor.list_sel()), None)
                    {
                        actions.choose_exe = Some((entry.game.dir.clone(), candidate));
                    }
                    if ui.button("Cancel").clicked() {
                        actions.close_chooser = true;
                    }
                });
        }

        if let Some(picker) = self.picker_open() {
            let list = self.picker_list(picker);
            match picker_window(ui.ctx(), &list, self.cursor.list_sel()) {
                Some(PickerEvent::Pick(i)) => actions.pick = Some((picker, i)),
                Some(PickerEvent::Cancel) => actions.cancel_picker = true,
                None => {}
            }
        }

        if matches!(self.popup, Some(Popup::Libraries)) {
            self.libraries_window(ui.ctx());
        }
        if self.proton_prompt_open() {
            self.proton_prompt_window(ui.ctx());
        }
        if self.overwrite_prompt_open() {
            self.overwrite_prompt_window(ui.ctx());
        }
        if let Some(result) = tuning_action {
            self.apply_tuning(result);
        }

        self.cursor.end_frame(
            std::mem::take(&mut frame.items)
                .into_iter()
                .map(|item| Slot {
                    target: item.target,
                    bounds: bounds(item.rect),
                })
                .collect(),
        );
        self.apply_actions(actions);
        for command in self.window.drain_commands() {
            ctx.send_viewport_cmd(match command {
                WindowCommand::Fullscreen(on) => egui::ViewportCommand::Fullscreen(on),
                WindowCommand::Minimize => egui::ViewportCommand::Minimized(true),
            });
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
                            // Ready games show their store here; the row is blank otherwise.
                            let store = store_of(&game.id).map_or(" ", Store::label);
                            ui.weak(store);
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

/// What the details page's Launch options asked for in one frame.
struct TuningResult {
    dir: PathBuf,
    id: String,
    save: Option<(std::collections::BTreeMap<String, String>, String)>,
    unpin: bool,
    reset_prefix: bool,
}
