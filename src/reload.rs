use crate::fetch::Fetch;
use crate::info::{GameInfo, refresh};
use crate::library::Game;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, mpsc};
use std::thread;

/// Outcome of one background refresh, delivered back to the UI thread.
pub struct Finished {
    /// `Game::key` of the refreshed game.
    pub key: String,
    pub result: Result<GameInfo, String>,
}

/// Runs info refreshes on worker threads so the UI never waits on the network.
pub struct Reloads {
    fetch: Arc<dyn Fetch + Send + Sync>,
    tx: mpsc::Sender<Finished>,
    rx: mpsc::Receiver<Finished>,
}

impl Reloads {
    pub fn new(fetch: Arc<dyn Fetch + Send + Sync>) -> Self {
        let (tx, rx) = mpsc::channel();
        Reloads { fetch, tx, rx }
    }

    pub fn start(&self, game: Game) {
        let fetch = Arc::clone(&self.fetch);
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = refresh(&game, fetch.as_ref()).map_err(|err| err.to_string());
            let _ = tx.send(Finished {
                key: game.key(),
                result,
            });
        });
    }

    pub fn try_next(&self) -> Option<Finished> {
        self.rx.try_recv().ok()
    }
}

/// What the launcher knows from each game's store page: the saved info, which games are
/// being refreshed now, and why the last refresh of a game failed. Keyed by `Game::key`.
pub struct Infos {
    reloads: Reloads,
    busy: HashSet<String>,
    infos: HashMap<String, GameInfo>,
    errors: HashMap<String, String>,
}

impl Infos {
    pub fn new(fetch: Arc<dyn Fetch + Send + Sync>) -> Self {
        Self {
            reloads: Reloads::new(fetch),
            busy: HashSet::new(),
            infos: HashMap::new(),
            errors: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&GameInfo> {
        self.infos.get(key)
    }

    /// Remembers info that was read from disk.
    pub fn set(&mut self, key: String, info: GameInfo) {
        self.infos.insert(key, info);
    }

    /// Why the last refresh of the game failed, if it did.
    pub fn error(&self, key: &str) -> Option<&str> {
        self.errors.get(key).map(String::as_str)
    }

    /// Whether the game is being refreshed now.
    pub fn is_busy(&self, key: &str) -> bool {
        self.busy.contains(key)
    }

    pub fn any_busy(&self) -> bool {
        !self.busy.is_empty()
    }

    /// Starts refreshing a game in the background, unless it is being refreshed already.
    pub fn reload(&mut self, game: Game) {
        let key = game.key();
        if self.busy.insert(key.clone()) {
            self.errors.remove(&key);
            self.reloads.start(game);
        }
    }

    /// Takes in the refreshes that have finished. Returns how many there were.
    pub fn collect(&mut self) -> usize {
        let mut finished = 0;
        while let Some(done) = self.reloads.try_next() {
            finished += 1;
            self.busy.remove(&done.key);
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
        finished
    }
}
