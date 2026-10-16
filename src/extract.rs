use crate::library::META_DIR;
use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const UTF8_FLAG: u16 = 0x0800;
/// How much is copied between progress reports.
const COPY_CHUNK: usize = 64 * 1024;
const EOCD_SIGNATURE: &[u8] = b"PK\x05\x06";
/// The end-of-central-directory record is at most 22 bytes plus a 65535-byte comment.
const EOCD_SEARCH_WINDOW: u64 = 22 + 65_535;

/// What a game folder needs before it can be played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Only zip archives are present; nothing has been extracted yet.
    NotExtracted,
    /// Archives that cannot be extracted here; the user must do it by hand.
    ExtractManually(Unsupported),
    /// Nothing to extract, or already extracted. `has_zip` says whether the
    /// original archive is still alongside the contents.
    Ready { has_zip: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    SplitArchive,
    SelfExtracting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 when the zip sets the UTF-8 flag, otherwise cp932.
    Auto,
    Utf8,
    Cp932,
}

#[derive(Debug)]
pub enum ExtractError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    Unsupported(Unsupported),
    NoArchive,
    Cancelled,
    UnsafeEntry(String),
    UnreadableName(String),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractError::Io(err) => write!(f, "{err}"),
            ExtractError::Zip(err) => write!(f, "{err}"),
            ExtractError::Unsupported(kind) => write!(f, "extract manually: {kind:?}"),
            ExtractError::NoArchive => write!(f, "no zip archive to extract"),
            ExtractError::Cancelled => write!(f, "cancelled"),
            ExtractError::UnsafeEntry(name) => write!(f, "unsafe path in archive: {name}"),
            ExtractError::UnreadableName(name) => write!(f, "cannot decode archive name: {name}"),
        }
    }
}

impl From<io::Error> for ExtractError {
    fn from(err: io::Error) -> Self {
        ExtractError::Io(err)
    }
}

impl From<zip::result::ZipError> for ExtractError {
    fn from(err: zip::result::ZipError) -> Self {
        ExtractError::Zip(err)
    }
}

struct Listed {
    name: String,
    is_dir: bool,
}

fn list(game_dir: &Path) -> io::Result<Vec<Listed>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(game_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == META_DIR {
            continue;
        }
        out.push(Listed {
            name,
            is_dir: entry.file_type()?.is_dir(),
        });
    }
    Ok(out)
}

fn is_zip(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".zip")
}

fn is_split_part(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let is_numbered = |ext: &str| ext.len() == 3 && ext.bytes().all(|b| b.is_ascii_digit());
    if let Some(ext) = lower.rsplit('.').next() {
        if is_numbered(ext) && (lower.contains(".zip.") || lower.contains(".7z.")) {
            return true;
        }
        // Old-style split zips: foo.z01, foo.z02, ...
        if let Some(rest) = ext.strip_prefix('z') {
            return rest.len() == 2 && rest.bytes().all(|b| b.is_ascii_digit());
        }
    }
    false
}

fn has_zip_tail(path: &Path) -> io::Result<bool> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(EOCD_SEARCH_WINDOW)))?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail)?;
    Ok(tail
        .windows(EOCD_SIGNATURE.len())
        .any(|w| w == EOCD_SIGNATURE))
}

fn unsupported(game_dir: &Path, listed: &[Listed]) -> io::Result<Option<Unsupported>> {
    if listed.iter().any(|e| !e.is_dir && is_split_part(&e.name)) {
        return Ok(Some(Unsupported::SplitArchive));
    }
    if let [only] = listed
        && !only.is_dir
        && only.name.to_ascii_lowercase().ends_with(".exe")
        && has_zip_tail(&game_dir.join(&only.name))?
    {
        return Ok(Some(Unsupported::SelfExtracting));
    }
    Ok(None)
}

pub fn status(game_dir: &Path) -> io::Result<Status> {
    let listed = list(game_dir)?;
    if let Some(kind) = unsupported(game_dir, &listed)? {
        return Ok(Status::ExtractManually(kind));
    }
    let has_zip = listed.iter().any(|e| !e.is_dir && is_zip(&e.name));
    let only_zips = listed.iter().all(|e| !e.is_dir && is_zip(&e.name));
    if has_zip && only_zips {
        Ok(Status::NotExtracted)
    } else {
        Ok(Status::Ready { has_zip })
    }
}

/// Extracts every zip directly in `game_dir`. Each archive is validated in full
/// before anything is written, so a bad entry leaves the folder untouched.
/// How far an extraction has got. Totals are known before the first byte is written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: u64,
    pub files_total: u64,
    /// Files left alone because they already existed ([`Existing::Skip`]); not in the totals.
    pub files_skipped: u64,
}

/// What to do with a file the zip would write where one already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Existing {
    Overwrite,
    Skip,
}

impl Progress {
    /// 0.0 to 1.0, by uncompressed bytes (by file count when there are no bytes).
    pub fn fraction(&self) -> f32 {
        let (done, total) = if self.bytes_total > 0 {
            (self.bytes_done, self.bytes_total)
        } else {
            (self.files_done, self.files_total)
        };
        if total == 0 {
            1.0
        } else {
            (done as f32 / total as f32).min(1.0)
        }
    }
}

pub fn extract(game_dir: &Path, encoding: Encoding) -> Result<(), ExtractError> {
    extract_with_progress(game_dir, encoding, |_| {})
}

/// Like [`extract`], reporting progress as it goes. `progress` is called once before
/// anything is written, then as data is copied, so it should be cheap.
pub fn extract_with_progress(
    game_dir: &Path,
    encoding: Encoding,
    progress: impl FnMut(Progress),
) -> Result<(), ExtractError> {
    extract_cancellable(game_dir, encoding, &AtomicBool::new(false), progress)
}

/// Like [`extract_with_progress`], stopping early when `cancel` becomes true.
pub fn extract_cancellable(
    game_dir: &Path,
    encoding: Encoding,
    cancel: &AtomicBool,
    progress: impl FnMut(Progress),
) -> Result<(), ExtractError> {
    extract_with_existing(game_dir, encoding, Existing::Overwrite, cancel, progress)
}

/// What each zip in `game_dir` would write, validated in full, with its output root.
type Prepared = Vec<(zip::ZipArchive<File>, Vec<PlannedEntry>, PathBuf)>;

fn prepare(game_dir: &Path, encoding: Encoding) -> Result<Prepared, ExtractError> {
    let listed = list(game_dir)?;
    if let Some(kind) = unsupported(game_dir, &listed)? {
        return Err(ExtractError::Unsupported(kind));
    }
    let mut zips: Vec<&str> = listed
        .iter()
        .filter(|e| !e.is_dir && is_zip(&e.name))
        .map(|e| e.name.as_str())
        .collect();
    if zips.is_empty() {
        return Err(ExtractError::NoArchive);
    }
    zips.sort();

    let mut jobs = Vec::new();
    for name in zips {
        let mut archive = zip::ZipArchive::new(File::open(game_dir.join(name))?)?;
        let plan = plan(&mut archive, encoding)?;
        let root = if plan.wrap {
            let stem = Path::new(name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            game_dir.join(stem)
        } else {
            game_dir.to_path_buf()
        };
        jobs.push((archive, plan.entries, root));
    }
    Ok(jobs)
}

/// The existing files (relative to `game_dir`) that extracting would replace. Writes nothing.
pub fn overwrites(game_dir: &Path, encoding: Encoding) -> Result<Vec<PathBuf>, ExtractError> {
    let mut found = Vec::new();
    for (_, entries, root) in prepare(game_dir, encoding)? {
        for entry in entries.iter().filter(|e| !e.is_dir) {
            let out = root.join(&entry.rel);
            if out.is_file() {
                found.push(out.strip_prefix(game_dir).unwrap_or(&out).to_path_buf());
            }
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// Like [`extract_cancellable`], choosing what happens to files that already exist.
pub fn extract_with_existing(
    game_dir: &Path,
    encoding: Encoding,
    existing: Existing,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> Result<(), ExtractError> {
    let mut jobs = prepare(game_dir, encoding)?;
    let mut skipped = 0;
    if existing == Existing::Skip {
        for (_, entries, root) in &mut jobs {
            entries.retain(|e| {
                let keep = e.is_dir || !root.join(&e.rel).is_file();
                skipped += u64::from(!keep);
                keep
            });
        }
    }

    let done = Progress {
        bytes_total: jobs.iter().flat_map(|(_, e, _)| e).map(|e| e.size).sum(),
        files_total: jobs
            .iter()
            .flat_map(|(_, e, _)| e)
            .filter(|e| !e.is_dir)
            .count() as u64,
        files_skipped: skipped,
        ..Progress::default()
    };
    progress(done);

    let mut created = Created::default();
    let result = write_entries(jobs, cancel, &mut progress, done, &mut created);
    if let Some(part) = created.part.take() {
        let _ = fs::remove_file(part);
    }
    if result.is_err() {
        created.roll_back();
    }
    result
}

/// Everything one run created, so a cancel can undo exactly that and nothing else.
#[derive(Default)]
struct Created {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
    /// The temporary file being written right now.
    part: Option<PathBuf>,
}

impl Created {
    fn roll_back(&mut self) {
        for file in self.files.drain(..) {
            let _ = fs::remove_file(file);
        }
        // Deepest first; `remove_dir` only removes an empty folder, so anything that was
        // already there (or that the user added meanwhile) is never touched.
        self.dirs
            .sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
        for dir in self.dirs.drain(..) {
            let _ = fs::remove_dir(dir);
        }
    }
}

/// `create_dir_all` that remembers which folders it actually had to create.
fn create_dirs_tracked(path: &Path, created: &mut Created) -> io::Result<()> {
    let mut missing = Vec::new();
    let mut current = path;
    while !current.exists() {
        missing.push(current.to_path_buf());
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    fs::create_dir_all(path)?;
    created.dirs.extend(missing);
    Ok(())
}

fn part_path(out: &Path) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

fn write_entries(
    jobs: Vec<(zip::ZipArchive<File>, Vec<PlannedEntry>, PathBuf)>,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
    mut done: Progress,
    created: &mut Created,
) -> Result<(), ExtractError> {
    let mut buffer = vec![0u8; COPY_CHUNK];
    for (mut archive, entries, root) in jobs {
        for entry in entries {
            if cancel.load(Ordering::SeqCst) {
                return Err(ExtractError::Cancelled);
            }
            let out = root.join(&entry.rel);
            if entry.is_dir {
                create_dirs_tracked(&out, created)?;
                continue;
            }
            if let Some(parent) = out.parent() {
                create_dirs_tracked(parent, created)?;
            }
            let existed = out.exists();
            // Written under a temporary name and renamed when complete, so a cancel or a
            // crash never leaves a half-written file under the real name.
            let part = part_path(&out);
            let mut src = archive.by_index(entry.index)?;
            let mut dst = File::create(&part)?;
            created.part = Some(part.clone());
            loop {
                if cancel.load(Ordering::SeqCst) {
                    return Err(ExtractError::Cancelled);
                }
                let n = src.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                dst.write_all(&buffer[..n])?;
                done.bytes_done += n as u64;
                progress(done);
            }
            drop(dst);
            fs::rename(&part, &out)?;
            created.part = None;
            if !existed {
                created.files.push(out);
            }
            done.files_done += 1;
            progress(done);
        }
    }
    Ok(())
}

struct PlannedEntry {
    index: usize,
    rel: PathBuf,
    is_dir: bool,
    /// Uncompressed size in bytes; 0 for directories.
    size: u64,
}

struct Plan {
    /// True when the archive has loose top-level files and needs its own folder.
    wrap: bool,
    entries: Vec<PlannedEntry>,
}

fn plan(archive: &mut zip::ZipArchive<File>, encoding: Encoding) -> Result<Plan, ExtractError> {
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let raw = archive.by_index_raw(index)?;
        let utf8_flag = raw.flags().as_u16() & UTF8_FLAG != 0;
        let is_dir = raw.is_dir();
        let size = if is_dir { 0 } else { raw.size() };
        let name = decode_name(raw.name_raw(), encoding, utf8_flag)?;
        drop(raw);
        if let Some(rel) = safe_relative(&name)? {
            entries.push(PlannedEntry {
                index,
                rel,
                is_dir,
                size,
            });
        }
    }

    let single_top = entries
        .windows(2)
        .all(|pair| pair[0].rel.components().next() == pair[1].rel.components().next())
        && !entries.is_empty();
    let all_inside_folder = entries
        .iter()
        .all(|e| e.is_dir || e.rel.components().count() > 1);
    let wrap = !(single_top && all_inside_folder);
    Ok(Plan { wrap, entries })
}

fn decode_name(raw: &[u8], encoding: Encoding, utf8_flag: bool) -> Result<String, ExtractError> {
    let as_utf8 = match encoding {
        Encoding::Auto => utf8_flag,
        Encoding::Utf8 => true,
        Encoding::Cp932 => false,
    };
    let decoded: Option<Cow<str>> = if as_utf8 {
        std::str::from_utf8(raw).ok().map(Cow::Borrowed)
    } else {
        encoding_rs::SHIFT_JIS.decode_without_bom_handling_and_without_replacement(raw)
    };
    decoded
        .map(Cow::into_owned)
        .ok_or_else(|| ExtractError::UnreadableName(String::from_utf8_lossy(raw).into_owned()))
}

/// Returns the entry's path relative to the extraction root, `None` for
/// entries that name the root itself, or an error for anything that could
/// escape it.
fn safe_relative(name: &str) -> Result<Option<PathBuf>, ExtractError> {
    let unsafe_entry = || ExtractError::UnsafeEntry(name.to_owned());
    let normalized = name.replace('\\', "/");
    if normalized.contains(':') {
        return Err(unsafe_entry());
    }
    let mut rel = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => {
                if part == META_DIR && rel.as_os_str().is_empty() {
                    return Err(unsafe_entry());
                }
                rel.push(part);
            }
            Component::CurDir => {}
            _ => return Err(unsafe_entry()),
        }
    }
    Ok((!rel.as_os_str().is_empty()).then_some(rel))
}
