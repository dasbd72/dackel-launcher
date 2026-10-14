//! Pure sort, search and favorites filter for the home grid.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub circle: Option<String>,
    pub favorite: bool,
    /// Unix seconds of the last launch, if the game was ever played.
    pub last_played: Option<u64>,
    /// Unix seconds when the game folder was added to the library, if known.
    pub added: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    LastPlayed,
    Title,
    DateAdded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub favorites_only: bool,
    pub sort: Sort,
}

/// Indices into `cards`, filtered by `query` and in its sort order.
pub fn browse(cards: &[Card], query: &Query) -> Vec<usize> {
    let needle = query.text.trim().to_lowercase();
    let mut order: Vec<usize> = (0..cards.len())
        .filter(|&i| !query.favorites_only || cards[i].favorite)
        .filter(|&i| needle.is_empty() || matches_text(&cards[i], &needle))
        .collect();
    order.sort_by(|&a, &b| match query.sort {
        Sort::Title => cards[a]
            .title
            .to_lowercase()
            .cmp(&cards[b].title.to_lowercase()),
        Sort::LastPlayed => cards[b].last_played.cmp(&cards[a].last_played),
        Sort::DateAdded => cards[b].added.cmp(&cards[a].added),
    });
    order
}

fn matches_text(card: &Card, needle: &str) -> bool {
    card.title.to_lowercase().contains(needle)
        || card.id.to_lowercase().contains(needle)
        || card
            .circle
            .as_ref()
            .is_some_and(|c| c.to_lowercase().contains(needle))
}

/// Play time for display: `--` when never recorded, otherwise `1h 02m` or `5m`.
pub fn format_play_time(secs: Option<u64>) -> String {
    match secs {
        None => "--".to_owned(),
        Some(secs) => {
            let minutes = secs / 60;
            match minutes / 60 {
                0 => format!("{minutes}m"),
                hours => format!("{hours}h {:02}m", minutes % 60),
            }
        }
    }
}
