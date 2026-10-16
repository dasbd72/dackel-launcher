use crate::library::META_DIR;
use crate::profile::{self, ExeState, Tuning};
use crate::tuning::{prefix_dir, split_args};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Everything needed to start a game, computed without touching the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<OsString>,
    pub env: BTreeMap<String, String>,
    pub working_dir: PathBuf,
    pub log_path: PathBuf,
}

/// Builds the `umu-run` invocation for a game's exe. Pure: no filesystem access.
pub fn plan(game_dir: &Path, exe_rel: &Path, proton: Option<&Path>) -> LaunchPlan {
    plan_with(game_dir, exe_rel, proton, &Tuning::default())
}

/// Like `plan`, with the game's own env and arguments applied. The user's env wins over
/// the defaults, except `WINEPREFIX`: the game's prefix is what Reset prefix manages.
pub fn plan_with(
    game_dir: &Path,
    exe_rel: &Path,
    proton: Option<&Path>,
    tuning: &Tuning,
) -> LaunchPlan {
    let exe = game_dir.join(exe_rel);
    let meta = game_dir.join(META_DIR);
    let mut env = BTreeMap::new();
    env.insert(
        "WINEPREFIX".to_owned(),
        prefix_dir(game_dir).display().to_string(),
    );
    env.insert("LANG".to_owned(), "ja_JP.UTF-8".to_owned());
    env.insert("LC_ALL".to_owned(), "ja_JP.UTF-8".to_owned());
    env.insert("GAMEID".to_owned(), game_id(game_dir));
    if let Some(proton) = proton {
        env.insert("PROTONPATH".to_owned(), proton.display().to_string());
    }
    let prefix = env["WINEPREFIX"].clone();
    env.extend(tuning.env.clone());
    env.insert("WINEPREFIX".to_owned(), prefix);
    let mut args = vec![exe.clone().into_os_string()];
    args.extend(
        split_args(&tuning.args)
            .unwrap_or_else(|_| tuning.args.split_whitespace().map(str::to_owned).collect())
            .into_iter()
            .map(OsString::from),
    );
    let working_dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    LaunchPlan {
        program: "umu-run".to_owned(),
        args,
        env,
        working_dir,
        log_path: meta.join("last-run.log"),
    }
}

/// `umu-` followed by the exe file name with only alphanumerics kept.
/// `umu-<RJ code>`: unique per game, unlike an exe-name id (many games ship `Game.exe`).
fn game_id(game_dir: &Path) -> String {
    let code = game_dir.file_name().map(|n| n.to_string_lossy());
    format!("umu-{}", code.as_deref().unwrap_or_default())
}

/// Starts a process from a plan. The only seam that touches the system.
pub trait Spawner {
    fn spawn(&self, plan: &LaunchPlan) -> io::Result<Box<dyn Process>>;
}

/// A spawned game, polled for exit.
pub trait Process {
    fn has_exited(&mut self) -> io::Result<bool>;
}

#[derive(Debug)]
pub enum LaunchError {
    NotReady,
    AlreadyRunning,
    Io(io::Error),
}

impl From<io::Error> for LaunchError {
    fn from(err: io::Error) -> Self {
        LaunchError::Io(err)
    }
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaunchError::NotReady => write!(f, "game has no exe chosen yet"),
            LaunchError::AlreadyRunning => write!(f, "game is already running"),
            LaunchError::Io(err) => write!(f, "{err}"),
        }
    }
}

/// Runs plans as real processes. Each game gets its own process group so
/// the launcher exiting (or being killed) does not take the game with it.
pub struct UmuSpawner;

struct Child(std::process::Child);

impl Process for Child {
    fn has_exited(&mut self) -> io::Result<bool> {
        Ok(self.0.try_wait()?.is_some())
    }
}

impl Spawner for UmuSpawner {
    fn spawn(&self, plan: &LaunchPlan) -> io::Result<Box<dyn Process>> {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};

        if let Some(dir) = plan.log_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let program = if plan.program == "umu-run" {
            crate::umu::locate_default().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, crate::umu::missing_message())
            })?
        } else {
            PathBuf::from(&plan.program)
        };
        let log = std::fs::File::create(&plan.log_path)?;
        let log_err = log.try_clone()?;
        let child = Command::new(&program)
            .args(&plan.args)
            .envs(&plan.env)
            .current_dir(&plan.working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .process_group(0)
            .spawn()?;
        Ok(Box::new(Child(child)))
    }
}

struct Running {
    game_dir: PathBuf,
    started: u64,
    process: Box<dyn Process>,
}

/// Tracks running games and records launches and play time in profiles.
pub struct Launcher<S: Spawner> {
    spawner: S,
    running: Vec<Running>,
}

impl<S: Spawner> Launcher<S> {
    pub fn new(spawner: S) -> Self {
        Launcher {
            spawner,
            running: Vec::new(),
        }
    }

    /// Launches a game whose exe is known. `now` is unix seconds.
    pub fn start(
        &mut self,
        game_dir: &Path,
        state: &ExeState,
        proton: Option<&Path>,
        now: u64,
    ) -> Result<(), LaunchError> {
        let ExeState::Ready { exe } = state else {
            return Err(LaunchError::NotReady);
        };
        self.poll(now)?;
        if self.is_running(game_dir) {
            return Err(LaunchError::AlreadyRunning);
        }
        let tuning = profile::read_tuning(game_dir)?;
        let plan = plan_with(game_dir, exe, proton, &tuning);
        let process = self.spawner.spawn(&plan)?;
        // Track the game first: once it is running, nothing below may lose sight of it.
        self.running.push(Running {
            game_dir: game_dir.to_path_buf(),
            started: now,
            process,
        });
        profile::record_launch(game_dir, now)?;
        Ok(())
    }

    pub fn is_running_any(&self) -> bool {
        !self.running.is_empty()
    }

    pub fn is_running(&self, game_dir: &Path) -> bool {
        self.running.iter().any(|r| r.game_dir == game_dir)
    }

    /// Finishes sessions whose process has exited, adding their play time.
    pub fn poll(&mut self, now: u64) -> io::Result<()> {
        let mut still_running = Vec::with_capacity(self.running.len());
        let mut result = Ok(());
        for mut run in std::mem::take(&mut self.running) {
            match run.process.has_exited() {
                Ok(false) => still_running.push(run),
                Ok(true) => {
                    let secs = now.saturating_sub(run.started);
                    if let Err(err) = profile::add_play_time(&run.game_dir, secs) {
                        result = Err(err);
                    }
                }
                Err(err) => {
                    result = Err(err);
                    still_running.push(run);
                }
            }
        }
        self.running = still_running;
        result
    }
}

/// Current unix time in seconds.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
