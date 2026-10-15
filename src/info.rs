use crate::fetch::{Fetch, Fetched};
use crate::library::{Game, META_DIR, is_dlsite_id};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const INFO_FILE: &str = "info.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameInfo {
    pub id: String,
    pub title: String,
    pub circle: Option<String>,
    pub genres: Vec<String>,
    pub description: String,
    pub cover: Option<String>,
    pub found: bool,
}

pub fn refresh(game: &Game, fetch: &dyn Fetch) -> io::Result<GameInfo> {
    // Only DLsite works have a store page; any other game is shown by its folder name.
    let parsed = if is_dlsite_id(&game.id) {
        let url = format!(
            "https://www.dlsite.com/maniax-touch/work/=/product_id/{}.html",
            game.id
        );
        match fetch.get(&url)? {
            Fetched::Body(body) => parse_page(&game.id, &String::from_utf8_lossy(&body)),
            Fetched::NotFound => None,
        }
    } else {
        None
    };
    let (mut info, cover_url) = match parsed {
        Some(parsed) => parsed,
        None => (fallback(game), None),
    };

    let meta = game.dir.join(META_DIR);
    fs::create_dir_all(&meta)?;
    remove_old_covers(&meta)?;
    if let Some(cover_url) = cover_url
        && let Fetched::Body(bytes) = fetch.get(&cover_url)?
    {
        let name = format!("cover.{}", cover_extension(&cover_url));
        fs::write(meta.join(&name), bytes)?;
        info.cover = Some(name);
    }
    let json = serde_json::to_vec_pretty(&info).map_err(io::Error::other)?;
    fs::write(meta.join(INFO_FILE), json)?;
    Ok(info)
}

fn fallback(game: &Game) -> GameInfo {
    let title = game
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| game.id.clone());
    GameInfo {
        id: game.id.clone(),
        title,
        circle: None,
        genres: Vec::new(),
        description: String::new(),
        cover: None,
        found: false,
    }
}

fn parse_page(id: &str, html: &str) -> Option<(GameInfo, Option<String>)> {
    let doc = Html::parse_document(html);
    let text = |sel: &str| -> Option<String> {
        let sel = Selector::parse(sel).ok()?;
        let el = doc.select(&sel).next()?;
        let t: String = el.text().collect::<String>().trim().to_owned();
        (!t.is_empty()).then_some(t)
    };
    let all = |sel: &str| -> Vec<String> {
        let sel = Selector::parse(sel).unwrap();
        doc.select(&sel)
            .map(|el| el.text().collect::<String>().trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect()
    };

    let title = text("#work_name")?;
    // The intro text is `<br>`-separated lines: join the text nodes of the first block.
    let description = Selector::parse(".c-productContent__text")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| {
            el.text()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let cover_url = {
        let sel = Selector::parse(r#"meta[property="og:image"]"#).unwrap();
        doc.select(&sel)
            .next()
            .and_then(|m| m.value().attr("content"))
            .map(|src| match src.strip_prefix("//") {
                Some(rest) => format!("https://{rest}"),
                None => src.to_owned(),
            })
    };
    let info = GameInfo {
        id: id.to_owned(),
        title,
        circle: text(r#"a[href*="/maker_id/"]"#),
        genres: all(".c-productTitleBox__genre a"),
        description,
        cover: None,
        found: true,
    };
    Some((info, cover_url))
}

fn remove_old_covers(meta: &Path) -> io::Result<()> {
    for entry in fs::read_dir(meta)? {
        let path = entry?.path();
        let is_cover = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("cover."));
        if is_cover {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn cover_extension(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    match path.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() && ext.len() <= 4 && !ext.contains('/') => ext,
        _ => "jpg",
    }
}

/// The saved info for a game, or `None` if it was never refreshed.
pub fn load(game: &Game) -> Option<GameInfo> {
    let bytes = fs::read(game.dir.join(META_DIR).join(INFO_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn cover_path(game: &Game, info: &GameInfo) -> Option<PathBuf> {
    info.cover
        .as_ref()
        .map(|name| game.dir.join(META_DIR).join(name))
}
