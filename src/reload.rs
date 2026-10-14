use crate::fetch::Fetch;
use crate::info::{GameInfo, refresh};
use crate::library::Game;
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
