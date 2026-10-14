use dlsite_launcher::library::{Game, resolve_library, scan};
use dlsite_launcher::profile::{self, ExeState};
use eframe::egui;
use std::io::ErrorKind;
use std::path::PathBuf;

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
        Box::new(move |_cc| Ok(Box::new(App::new(library)))),
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
}

impl App {
    fn new(library: PathBuf) -> Self {
        let mut app = App {
            library,
            view: View::Missing,
            chooser: None,
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
                        Entry { game, state }
                    })
                    .collect(),
            ),
            Err(err) if err.kind() == ErrorKind::NotFound => View::Missing,
            Err(err) => View::Error(err.to_string()),
        };
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
        let mut create = false;
        let mut open_chooser = None;
        let mut choice = None;
        let mut close_chooser = false;

        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Rescan").clicked() {
                    rescan = true;
                }
                ui.label(self.library.display().to_string());
            });
        });

        egui::CentralPanel::default().show(ui, |ui| match &self.view {
            View::Missing => {
                ui.heading("Library not found");
                ui.label(format!("{} does not exist.", self.library.display()));
                if ui.button("Create it").clicked() {
                    create = true;
                }
            }
            View::Error(msg) => {
                ui.colored_label(egui::Color32::LIGHT_RED, msg);
            }
            View::Games(entries) if entries.is_empty() => {
                ui.label("No games yet. Put RJ folders in the library and press Rescan.");
            }
            View::Games(entries) => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (i, entry) in entries.iter().enumerate() {
                            let response = tile(ui, entry);
                            if response.clicked() && needs_choice(entry) {
                                open_chooser = Some(i);
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
                    for candidate in candidates {
                        if ui.button(candidate.display().to_string()).clicked() {
                            choice = Some((entry.game.dir.clone(), candidate.clone()));
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        close_chooser = true;
                    }
                });
        }

        if let Some(i) = open_chooser {
            self.chooser = Some(i);
        }
        if close_chooser {
            self.chooser = None;
        }
        if let Some((dir, exe)) = choice {
            match profile::choose(&dir, &exe) {
                Ok(()) => self.rescan(),
                Err(err) => self.view = View::Error(err.to_string()),
            }
        } else if create {
            self.create_library();
        } else if rescan {
            self.rescan();
        }
    }
}

fn needs_choice(entry: &Entry) -> bool {
    matches!(entry.state, Ok(ExeState::NeedsExeChoice { .. }))
}

fn tile(ui: &mut egui::Ui, entry: &Entry) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(200.0, 150.0), egui::Sense::click());
    ui.painter()
        .rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
    ui.painter().text(
        rect.center_top() + egui::vec2(0.0, 30.0),
        egui::Align2::CENTER_CENTER,
        &entry.game.id,
        egui::FontId::proportional(20.0),
        ui.visuals().text_color(),
    );
    let status = match &entry.state {
        Ok(ExeState::Ready { exe }) => exe.display().to_string(),
        Ok(ExeState::NeedsExeChoice { .. }) => "needs exe choice".to_owned(),
        Err(msg) => msg.clone(),
    };
    ui.painter().text(
        rect.center_bottom() - egui::vec2(0.0, 24.0),
        egui::Align2::CENTER_CENTER,
        status,
        egui::FontId::proportional(13.0),
        ui.visuals().weak_text_color(),
    );
    response
}
