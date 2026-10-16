//! Installing winetricks verbs (`dotnet48`, `vcrun2022`, ...) into a game's own prefix, with the
//! `winetricks` that `umu-run` carries: a read-only Deck has none of its own.

use crate::launch::{LaunchPlan, plan_with};
use crate::library::META_DIR;
use crate::profile::{self, Tuning};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A verb offered as a one-click choice.
pub struct Common {
    pub verb: &'static str,
    pub hint: &'static str,
}

/// The one-click choices, in the order they are shown.
pub const COMMON: [Common; 6] = [
    Common {
        verb: "dotnet48",
        hint: ".NET Framework 4.8 (Bakin games ask for it)",
    },
    Common {
        verb: "vcrun2022",
        hint: "Visual C++ runtimes",
    },
    Common {
        verb: "d3dcompiler_47",
        hint: "Direct3D shader compiler",
    },
    Common {
        verb: "d3dx9",
        hint: "DirectX 9 helper libraries",
    },
    Common {
        verb: "corefonts",
        hint: "Arial, Times New Roman and other core fonts",
    },
    Common {
        verb: "cjkfonts",
        hint: "Chinese, Japanese and Korean fonts",
    },
];

/// Reads the typed verbs: words separated by spaces, each made of letters, digits and `_ . = -`
/// (so no shell can get in), none starting with `-`.
pub fn parse_verbs(text: &str) -> Result<Vec<String>, String> {
    let verbs: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
    if verbs.is_empty() {
        return Err("Type or pick at least one verb".to_owned());
    }
    for verb in &verbs {
        let plain = verb
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '=' | '-'));
        if !plain || verb.starts_with('-') {
            return Err(format!("\"{verb}\" is not a winetricks verb"));
        }
    }
    Ok(verbs)
}

/// Adds the verb to the typed text, or takes it out when it is already there.
pub fn toggle_verb(text: &str, verb: &str) -> String {
    let mut words: Vec<&str> = text.split_whitespace().collect();
    match words.iter().position(|w| *w == verb) {
        Some(i) => {
            words.remove(i);
        }
        None => words.push(verb),
    }
    words.join(" ")
}

/// Where the install's output goes.
pub fn log_path(game_dir: &Path) -> PathBuf {
    game_dir.join(META_DIR).join("winetricks.log")
}

/// `umu-run winetricks -q <verbs>` against the game's prefix, with the same Proton, environment
/// and Wine overrides as a launch.
pub fn plan(
    game_dir: &Path,
    verbs: &[String],
    proton: Option<&Path>,
    tuning: &Tuning,
) -> LaunchPlan {
    let mut plan = plan_with(game_dir, Path::new(""), proton, tuning);
    let mut args: Vec<OsString> = vec!["winetricks".into(), "-q".into()];
    args.extend(verbs.iter().map(OsString::from));
    plan.args = args;
    plan.working_dir = game_dir.to_path_buf();
    plan.log_path = log_path(game_dir);
    plan
}

/// Runs the plan to the end and, when winetricks succeeds, records the verbs in the profile.
/// Blocks for minutes: call it off the UI thread.
pub fn install(game_dir: &Path, plan: &LaunchPlan, verbs: &[String]) -> Result<(), String> {
    let program = crate::launch::program_of(plan).map_err(|e| e.to_string())?;
    if let Some(dir) = plan.log_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let log = std::fs::File::create(&plan.log_path).map_err(|e| e.to_string())?;
    let status = crate::launch::command(&program, &plan.args, plan, &log)
        .and_then(|mut c| c.status())
        .map_err(|e| format!("could not run winetricks: {e}"))?;
    if !status.success() {
        return Err(format!(
            "winetricks failed ({}); see {}",
            status,
            plan.log_path.display()
        ));
    }
    profile::add_winetricks(game_dir, verbs).map_err(|e| e.to_string())
}
