mod details;

use details::{DetailsView, details_ui, exe_choices};
use dlsite_launcher::browse::{self, Card, Query, Sort};
use dlsite_launcher::extract::{self, Encoding, Status};
use dlsite_launcher::fetch::HttpFetch;
use dlsite_launcher::fonts;
use dlsite_launcher::info::{GameInfo, cover_path, load};
use dlsite_launcher::launch::{Launcher, UmuSpawner, unix_now};
use dlsite_launcher::library::{Game, resolve_library, scan};
use dlsite_launcher::nav::{self, Command, Context, Input, Repeater};
use dlsite_launcher::profile::{self, ExeState, Stats};
use dlsite_launcher::proton;
use dlsite_launcher::reload::Reloads;
use dlsite_launcher::settings::{self, Settings};
use dlsite_launcher::window::{self, Restore, RestoreStep};
use eframe::egui;
use gilrs::{Axis, Button, EventType, Gilrs};
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
            install_japanese_font(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(startup.scale);
            Ok(Box::new(App::new(library, startup)))
        }),
    )
}

const TILE_WIDTH: f32 = 200.0;

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

struct App {
    library: PathBuf,
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
    /// The game the controller cursor is on (by id, so it survives rescans and re-sorts).
    focus_id: Option<String>,
    /// True until the user moves the cursor: it then tracks the first game as the list re-sorts.
    focus_auto: bool,
    chooser_sel: usize,
    columns: usize,
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
    minimized_by_us: bool,
    restore: Restore,
    started: std::time::Instant,
}

impl App {
    fn new(library: PathBuf, startup: Startup) -> Self {
        let mut app = App {
            library,
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
            focus_id: None,
            focus_auto: true,
            chooser_sel: 0,
            columns: 1,
            scroll_to_focus: false,
            settings: startup.settings,
            settings_path: startup.settings_path,
            minimize_on_launch: startup.minimize_on_launch,
            fullscreen: startup.fullscreen,
            scale: startup.scale,
            window_cmds: Vec::new(),
            zoom_request: None,
            minimized_by_us: false,
            restore: Restore::default(),
            started: std::time::Instant::now(),
        };
        app.rescan();
        app
    }

    fn rescan(&mut self) {
        self.chooser = None;
        self.view = match scan(&self.library) {
            Ok(games) => View::Games(
                games
                    .into_iter()
                    .map(|game| {
                        let state = profile::ensure(&game.dir).map_err(|e| e.to_string());
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
            Err(err) if err.kind() == ErrorKind::NotFound => View::Missing,
            Err(err) => View::Error(err.to_string()),
        };
        let games: Vec<Game> = match &self.view {
            View::Games(entries) => entries.iter().map(|e| e.game.clone()).collect(),
            _ => return,
        };
        for game in games {
            match load(&game) {
                Some(info) => {
                    self.infos.insert(game.id.clone(), info);
                }
                None => self.reload(game),
            }
        }
    }

    fn reload(&mut self, game: Game) {
        if self.busy.insert(game.id.clone()) {
            self.errors.remove(&game.id);
            self.reloads.start(game);
        }
    }

    fn collect_reloads(&mut self) {
        while let Some(done) = self.reloads.try_next() {
            self.busy.remove(&done.id);
            match done.result {
                Ok(info) => {
                    self.errors.remove(&done.id);
                    self.infos.insert(done.id, info);
                }
                Err(msg) => {
                    self.errors.insert(done.id, msg);
                }
            }
        }
    }

    fn extract_game(&mut self, dir: &std::path::Path) {
        self.message = Some(match extract::extract(dir, self.encoding) {
            Ok(()) => format!("Extracted {}", dir.display()),
            Err(err) => format!("Extract failed: {err}"),
        });
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
        let Some(entry) = entries.iter_mut().find(|e| e.game.id == id) else {
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
            .map(|e| card_of(e, self.infos.get(&e.game.id)))
            .collect();
        browse::browse(&cards, &self.query)
    }

    /// Keeps the cursor on a game that is on screen, so focus is never lost.
    fn normalize_focus(&mut self, visible: &[usize]) {
        let View::Games(entries) = &self.view else {
            return;
        };
        let on_screen = |id: &String| visible.iter().any(|&i| &entries[i].game.id == id);
        if self.focus_auto || !self.focus_id.as_ref().is_some_and(on_screen) {
            self.focus_id = visible.first().map(|&i| entries[i].game.id.clone());
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
        entries.iter().position(|e| &e.game.id == id)
    }

    fn context(&self) -> Context {
        if self.chooser.is_some() {
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

    fn cycle_ui_scale(&mut self) {
        self.set_ui_scale(window::next_scale(self.scale));
    }

    fn open_chooser(&mut self, index: usize) {
        self.chooser = Some(index);
        self.chooser_sel = 0;
    }

    fn apply(&mut self, command: Command, visible: &[usize]) {
        match command {
            Command::Move(dir) => {
                let View::Games(entries) = &self.view else {
                    return;
                };
                let at = visible
                    .iter()
                    .position(|&i| Some(&entries[i].game.id) == self.focus_id.as_ref())
                    .unwrap_or(0);
                let to = nav::move_focus(at, visible.len(), self.columns, dir);
                if let Some(&i) = visible.get(to) {
                    self.focus_id = Some(entries[i].game.id.clone());
                    self.focus_auto = false;
                    self.scroll_to_focus = true;
                }
            }
            Command::Primary => {
                let Some(i) = self.target_index() else { return };
                let View::Games(entries) = &self.view else {
                    return;
                };
                let entry = &entries[i];
                let dir = entry.game.dir.clone();
                if self.launcher.is_running(&dir) {
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
                if let (Some(i), View::Games(entries)) = (self.target_index(), &self.view) {
                    let game = entries[i].game.clone();
                    self.reload(game);
                }
            }
            Command::OpenDetails => {
                if let Some(id) = self.focus_id.clone() {
                    self.screen = Screen::Details(id);
                }
            }
            Command::Back => self.screen = Screen::Grid,
            Command::ToggleFavorite => {
                if let (Some(i), View::Games(entries)) = (self.target_index(), &self.view) {
                    let id = entries[i].game.id.clone();
                    self.toggle_favorite(&id);
                }
            }
            Command::ChooseExe => {
                if let Some(i) = self.target_index() {
                    self.open_chooser(i);
                }
            }
            Command::ReExtract => {
                if let (Some(i), View::Games(entries)) = (self.target_index(), &self.view)
                    && matches!(entries[i].extract, Ok(Status::Ready { has_zip: true }))
                {
                    let dir = entries[i].game.dir.clone();
                    self.extract_game(&dir);
                }
            }
            Command::CycleSort(delta) => {
                let all = [Sort::LastPlayed, Sort::Title, Sort::DateAdded];
                self.query.sort = cycled(&all, self.query.sort, delta);
            }
            Command::CycleEncoding(delta) => {
                let all = [Encoding::Auto, Encoding::Cp932, Encoding::Utf8];
                self.encoding = cycled(&all, self.encoding, delta);
            }
            Command::Rescan => self.rescan(),
            Command::ToggleFavoritesOnly => self.query.favorites_only = !self.query.favorites_only,
            Command::ResetFilters => {
                self.query.text.clear();
                self.query.favorites_only = false;
            }
            Command::ChooserMove(delta) => {
                let View::Games(entries) = &self.view else {
                    return;
                };
                let len = self
                    .chooser
                    .and_then(|i| entries.get(i))
                    .map_or(0, |e| e.candidates.len());
                let last = len.saturating_sub(1) as i64;
                self.chooser_sel =
                    (self.chooser_sel as i64 + i64::from(delta)).clamp(0, last) as usize;
            }
            Command::ChooserPick => {
                let View::Games(entries) = &self.view else {
                    return;
                };
                let picked = self.chooser.and_then(|i| entries.get(i)).and_then(|e| {
                    Some((
                        e.game.dir.clone(),
                        e.candidates.get(self.chooser_sel)?.clone(),
                    ))
                });
                if let Some((dir, exe)) = picked {
                    match profile::choose(&dir, &exe) {
                        Ok(()) => self.rescan(),
                        Err(err) => self.view = View::Error(err.to_string()),
                    }
                }
            }
            Command::ChooserCancel => self.chooser = None,
            Command::ToggleFullscreen => self.toggle_fullscreen(),
            Command::CycleUiScale => self.cycle_ui_scale(),
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
        let inputs = self.poll_inputs(&ctx);
        let visible = self.visible();
        self.normalize_focus(&visible);
        for input in inputs {
            if let Some(command) = nav::command(self.context(), input) {
                self.apply(command, &visible);
            }
        }
        // Commands can rescan or re-sort; make sure the cursor is still on a visible game.
        let visible = self.visible();
        self.normalize_focus(&visible);
        // Apply a requested zoom; otherwise notice egui's own Ctrl +/- zoom and remember it.
        if let Some(zoom) = self.zoom_request.take() {
            ctx.set_zoom_factor(zoom);
        } else if (ctx.zoom_factor() - self.scale).abs() > 1e-3 {
            self.set_ui_scale(ctx.zoom_factor());
        }

        let mut rescan = false;
        let mut extract_dir: Option<PathBuf> = None;
        let mut create = false;
        let mut open_chooser = None;
        let mut choice = None;
        let mut close_chooser = false;
        let mut launch = None;
        let mut clicked_focus: Option<String> = None;
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
        let mut scale_edit = self.scale;
        let mut scale_dragging = false;
        let mut scale_done = false;
        let mut fullscreen_click = false;
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Rescan").clicked() {
                    rescan = true;
                }
                fullscreen_click = ui.button("Fullscreen").clicked();
                let scale = ui.add(
                    egui::DragValue::new(&mut scale_edit)
                        .range(0.5..=3.0)
                        .speed(0.01)
                        .fixed_decimals(2)
                        .prefix("UI x"),
                );
                scale_dragging = scale.changed();
                scale_done = scale.drag_stopped() || scale.lost_focus();
                egui::ComboBox::from_label("Zip encoding")
                    .selected_text(encoding_label(self.encoding))
                    .show_ui(ui, |ui| {
                        for choice in [Encoding::Auto, Encoding::Cp932, Encoding::Utf8] {
                            ui.selectable_value(&mut self.encoding, choice, encoding_label(choice));
                        }
                    });
                ui.label(self.library.display().to_string());
            });
            if matches!(self.screen, Screen::Grid) {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("Sort")
                        .selected_text(sort_label(self.query.sort))
                        .show_ui(ui, |ui| {
                            for sort in [Sort::LastPlayed, Sort::Title, Sort::DateAdded] {
                                ui.selectable_value(&mut self.query.sort, sort, sort_label(sort));
                            }
                        });
                    ui.label("Search");
                    ui.text_edit_singleline(&mut self.query.text);
                    ui.checkbox(&mut self.query.favorites_only, "Favorites only");
                });
            }
            if let Some(message) = &self.message {
                ui.label(message);
            }
        });

        if fullscreen_click {
            self.toggle_fullscreen();
        }
        if scale_dragging {
            self.scale = scale_edit;
            self.settings.ui_scale = Some(scale_edit);
            self.zoom_request = Some(scale_edit);
        }
        if scale_done {
            self.save_settings();
        }

        let hint = match self.context() {
            Context::Grid => {
                "A Launch   B Reset filters   X Reload   Y Details   LB/RB Sort   LT/RT Zip encoding   Select Favorites only   Start Rescan   R3/F11 Fullscreen   L3 UI scale"
            }
            Context::Details => {
                "A Launch   B Back   X Reload   Y Favorite   LB Choose exe   RB Re-extract   Start Rescan   R3/F11 Fullscreen   L3 UI scale"
            }
            Context::Chooser => "Up/Down Select   A Pick   B Cancel   R3/F11 Fullscreen",
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
                if entries.iter().any(|e| &e.game.id == id) =>
            {
                let entry = entries.iter().find(|e| &e.game.id == id).unwrap();
                let info = self.infos.get(id);
                let view = DetailsView {
                    id,
                    info,
                    cover: info
                        .and_then(|info| cover_path(&entry.game, info))
                        .filter(|path| path.exists()),
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
                };
                let action = details_ui(ui, &view);
                back = action.back;
                if action.reload {
                    details_reload = Some(entry.game.clone());
                }
                if action.extract {
                    details_extract = Some(entry.game.dir.clone());
                }
                if action.launch {
                    launch = entries.iter().position(|e| &e.game.id == id);
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
                let cards: Vec<Card> = entries
                    .iter()
                    .map(|e| card_of(e, self.infos.get(&e.game.id)))
                    .collect();
                let order = browse::browse(&cards, &self.query);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let spacing = ui.spacing().item_spacing.x;
                    let columns = ((ui.available_width() + spacing) / (TILE_WIDTH + spacing))
                        .floor()
                        .max(1.0) as usize;
                    self.columns = columns;
                    for row in order.chunks(columns) {
                        ui.horizontal_top(|ui| {
                            for &i in row {
                                let entry = &entries[i];
                                let tile = Tile {
                                    entry,
                                    info: self.infos.get(&entry.game.id),
                                    error: self.errors.get(&entry.game.id).map(String::as_str),
                                    busy: self.busy.contains(&entry.game.id),
                                    running: self.launcher.is_running(&entry.game.dir),
                                    focused: self.focus_id.as_ref() == Some(&entry.game.id),
                                };
                                let action = tile_ui(ui, &tile);
                                if tile.focused && self.scroll_to_focus {
                                    ui.scroll_to_rect(action.rect.expand(8.0), None);
                                }
                                if action.clicked
                                    || action.launch
                                    || action.details
                                    || action.favorite
                                    || action.extract.is_some()
                                {
                                    clicked_focus = Some(entry.game.id.clone());
                                }
                                if let Some(dir) = action.extract {
                                    extract_dir = Some(dir);
                                }
                                if action.launch {
                                    launch = Some(i);
                                }
                                if action.favorite {
                                    favorite_id = Some(entry.game.id.clone());
                                }
                                if action.details {
                                    open_details = Some(entry.game.id.clone());
                                }
                                if action.clicked && needs_choice(entry) {
                                    open_chooser = Some(i);
                                } else if action.clicked {
                                    open_details = Some(entry.game.id.clone());
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
                        exe_choices(ui, &entry.candidates, Some(self.chooser_sel))
                    {
                        choice = Some((entry.game.dir.clone(), candidate));
                    }
                    if ui.button("Cancel").clicked() {
                        close_chooser = true;
                    }
                });
        }

        self.scroll_to_focus = false;
        if let Some(id) = clicked_focus {
            self.focus_id = Some(id);
            self.focus_auto = false;
        }
        if let Some(i) = launch {
            self.launch_game(i);
        }
        if let Some(i) = open_chooser {
            self.chooser = Some(i);
        }
        if close_chooser {
            self.chooser = None;
        }
        if let Some(id) = open_details {
            self.screen = Screen::Details(id);
        }
        if back {
            self.screen = Screen::Grid;
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
    focused: bool,
}

struct TileAction {
    launch: bool,
    clicked: bool,
    extract: Option<PathBuf>,
    favorite: bool,
    details: bool,
    rect: egui::Rect,
}

/// Draws one game tile: cover, title, exe status and a Reload button.
fn tile_ui(ui: &mut egui::Ui, tile: &Tile) -> TileAction {
    let game = &tile.entry.game;
    let mut action = TileAction {
        launch: false,
        clicked: false,
        extract: None,
        favorite: false,
        details: false,
        rect: egui::Rect::NOTHING,
    };
    let tile_rect = ui.vertical(|ui| {
        ui.set_width(TILE_WIDTH);
        let size = egui::vec2(TILE_WIDTH, 150.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        action.clicked = response.clicked();
        ui.painter()
            .rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
        let cover = tile
            .info
            .and_then(|info| cover_path(game, info))
            .filter(|path| path.exists());
        match cover {
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
        let title = tile
            .info
            .map_or(game.id.as_str(), |info| info.title.as_str());
        // Reserve two title lines so the button rows line up across a grid row.
        ui.vertical(|ui| {
            ui.set_min_height(2.0 * ui.text_style_height(&egui::TextStyle::Body));
            ui.add(egui::Label::new(egui::RichText::new(title).strong()).wrap());
        });
        if tile.running {
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
            let not_extracted = matches!(tile.entry.extract, Ok(Status::NotExtracted));
            let ready = matches!(tile.entry.state, Ok(ExeState::Ready { .. }));
            if tile.running {
                ui.add_enabled(false, egui::Button::new("Launch"));
            } else if not_extracted {
                if ui.button("Extract").clicked() {
                    action.extract = Some(game.dir.clone());
                }
            } else {
                action.launch = ui.add_enabled(ready, egui::Button::new("Launch")).clicked();
            }
            action.details = ui.button("Details").clicked();
            action.favorite = ui
                .selectable_label(tile.entry.stats.favorite, "Fav")
                .clicked();
        });
    });
    action.rect = tile_rect.response.rect;
    if tile.focused {
        ui.painter().rect_stroke(
            action.rect.expand(3.0),
            6.0,
            egui::Stroke::new(3.0, egui::Color32::from_rgb(90, 170, 255)),
            egui::StrokeKind::Outside,
        );
    }
    action
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
    (egui::Key::F11, Input::RightStickClick),
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
        Button::LeftTrigger => Input::LeftBumper,
        Button::RightTrigger => Input::RightBumper,
        Button::LeftTrigger2 => Input::LeftTrigger,
        Button::RightTrigger2 => Input::RightTrigger,
        Button::Start => Input::Start,
        Button::Select => Input::Select,
        Button::LeftThumb => Input::LeftStickClick,
        Button::RightThumb => Input::RightStickClick,
        _ => return None,
    })
}

fn dir_input(dir: nav::Dir) -> Input {
    match dir {
        nav::Dir::Up => Input::Up,
        nav::Dir::Down => Input::Down,
        nav::Dir::Left => Input::Left,
        nav::Dir::Right => Input::Right,
    }
}

/// The item `delta` steps from `current` in `all`, wrapping around.
fn cycled<T: Copy + PartialEq>(all: &[T], current: T, delta: i32) -> T {
    let at = all.iter().position(|&x| x == current).unwrap_or(0) as i32;
    all[(at + delta).rem_euclid(all.len() as i32) as usize]
}
