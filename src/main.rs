mod details;

use details::{DetailsView, details_ui, exe_choices};
use dlsite_launcher::browse::{self, Card, Query, Sort};
use dlsite_launcher::extract::{self, Encoding, Status};
use dlsite_launcher::fetch::HttpFetch;
use dlsite_launcher::info::{GameInfo, cover_path, load};
use dlsite_launcher::launch::{Launcher, UmuSpawner, unix_now};
use dlsite_launcher::library::{Game, resolve_library, scan};
use dlsite_launcher::profile::{self, ExeState, Stats};
use dlsite_launcher::proton;
use dlsite_launcher::reload::Reloads;
use eframe::egui;
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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DLsite Launcher",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(App::new(library)))
        }),
    )
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
}

impl App {
    fn new(library: PathBuf) -> Self {
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
                    Ok(()) => format!("Launched {}", entry.game.id),
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

    fn create_library(&mut self) {
        match std::fs::create_dir_all(&self.library) {
            Ok(()) => self.rescan(),
            Err(err) => self.view = View::Error(err.to_string()),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut rescan = false;
        let mut extract_dir: Option<PathBuf> = None;
        let mut create = false;
        let mut open_chooser = None;
        let mut choice = None;
        let mut close_chooser = false;
        let mut reload_games = Vec::new();
        let mut launch = None;
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
        if let Err(err) = self.launcher.poll(unix_now()) {
            self.message = Some(format!("Play time not saved: {err}"));
        }
        if self.launcher.is_running_any() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }

        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Rescan").clicked() {
                    rescan = true;
                }
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
                    ui.horizontal_wrapped(|ui| {
                        for i in order {
                            let entry = &entries[i];
                            let tile = Tile {
                                entry,
                                info: self.infos.get(&entry.game.id),
                                error: self.errors.get(&entry.game.id).map(String::as_str),
                                busy: self.busy.contains(&entry.game.id),
                                running: self.launcher.is_running(&entry.game.dir),
                            };
                            let action = tile_ui(ui, &tile);
                            if let Some(dir) = action.extract {
                                extract_dir = Some(dir);
                            }
                            if action.reload {
                                reload_games.push(entry.game.clone());
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
                });
            }
        });

        if let (Some(i), View::Games(entries)) = (self.chooser, &self.view)
            && let Some(entry) = entries.get(i)
            && let Ok(ExeState::NeedsExeChoice { candidates }) = &entry.state
        {
            egui::Window::new(format!("Choose exe for {}", entry.game.id))
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    if let Some(candidate) = exe_choices(ui, candidates) {
                        choice = Some((entry.game.dir.clone(), candidate));
                    }
                    if ui.button("Cancel").clicked() {
                        close_chooser = true;
                    }
                });
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
        for game in reload_games {
            self.reload(game);
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
}

struct TileAction {
    reload: bool,
    launch: bool,
    clicked: bool,
    extract: Option<PathBuf>,
    favorite: bool,
    details: bool,
}

/// Draws one game tile: cover, title, exe status and a Reload button.
fn tile_ui(ui: &mut egui::Ui, tile: &Tile) -> TileAction {
    let game = &tile.entry.game;
    let mut action = TileAction {
        reload: false,
        launch: false,
        clicked: false,
        extract: None,
        favorite: false,
        details: false,
    };
    ui.vertical(|ui| {
        ui.set_width(200.0);
        let size = egui::vec2(200.0, 150.0);
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
        ui.add(egui::Label::new(egui::RichText::new(title).strong()).wrap());
        let status = match &tile.entry.state {
            Ok(ExeState::Ready { exe }) => exe.display().to_string(),
            Ok(ExeState::NeedsExeChoice { .. }) => "needs exe choice".to_owned(),
            Err(msg) => msg.clone(),
        };
        ui.weak(status);
        match &tile.entry.extract {
            Err(msg) => {
                ui.colored_label(egui::Color32::LIGHT_RED, msg);
            }
            Ok(Status::NotExtracted) => {
                ui.label("not extracted");
                if ui.button("Extract").clicked() {
                    action.extract = Some(game.dir.clone());
                }
            }
            Ok(Status::ExtractManually(_)) => {
                ui.colored_label(egui::Color32::YELLOW, "extract manually");
            }
            Ok(Status::Ready { has_zip }) => {
                if *has_zip && ui.button("Re-extract").clicked() {
                    action.extract = Some(game.dir.clone());
                }
            }
        }
        if let Some(err) = tile.error {
            ui.colored_label(egui::Color32::LIGHT_RED, err);
        }
        if tile.running {
            ui.colored_label(egui::Color32::LIGHT_GREEN, "Running");
        } else {
            let ready = matches!(tile.entry.state, Ok(ExeState::Ready { .. }));
            action.launch = ui.add_enabled(ready, egui::Button::new("Launch")).clicked();
        }
        let label = if tile.busy { "Reloading..." } else { "Reload" };
        ui.horizontal(|ui| {
            action.reload = ui
                .add_enabled(!tile.busy, egui::Button::new(label))
                .clicked();
            let star = if tile.entry.stats.favorite {
                "Unfavorite"
            } else {
                "Favorite"
            };
            action.favorite = ui.button(star).clicked();
            action.details = ui.button("Details").clicked();
        });
    });
    action
}

fn encoding_label(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Auto => "Auto",
        Encoding::Cp932 => "cp932",
        Encoding::Utf8 => "UTF-8",
    }
}
