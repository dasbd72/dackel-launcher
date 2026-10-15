use dackel_launcher::browse::{Card, Query, Sort, browse, format_play_time};

fn card(id: &str, title: &str) -> Card {
    Card {
        id: id.to_owned(),
        title: title.to_owned(),
        circle: None,
        favorite: false,
        last_played: None,
        added: None,
    }
}

fn query(sort: Sort) -> Query {
    Query {
        text: String::new(),
        favorites_only: false,
        sort,
    }
}

fn ids(cards: &[Card], order: &[usize]) -> Vec<String> {
    order.iter().map(|&i| cards[i].id.clone()).collect()
}

#[test]
fn sorts_by_title_ignoring_case() {
    let cards = vec![
        card("RJ2", "banana"),
        card("RJ1", "Apple"),
        card("RJ3", "cherry"),
    ];

    let order = browse(&cards, &query(Sort::Title));

    assert_eq!(ids(&cards, &order), ["RJ1", "RJ2", "RJ3"]);
}

#[test]
fn sorts_by_last_played_most_recent_first_and_never_played_last() {
    let mut old = card("RJ1", "Old");
    old.last_played = Some(100);
    let never = card("RJ2", "Never");
    let mut new = card("RJ3", "New");
    new.last_played = Some(900);
    let cards = vec![old, never, new];

    let order = browse(&cards, &query(Sort::LastPlayed));

    assert_eq!(ids(&cards, &order), ["RJ3", "RJ1", "RJ2"]);
}

#[test]
fn sorts_by_date_added_newest_first() {
    let mut early = card("RJ1", "Early");
    early.added = Some(10);
    let mut late = card("RJ2", "Late");
    late.added = Some(500);
    let cards = vec![early, late];

    let order = browse(&cards, &query(Sort::DateAdded));

    assert_eq!(ids(&cards, &order), ["RJ2", "RJ1"]);
}

fn searching(text: &str) -> Query {
    Query {
        text: text.to_owned(),
        favorites_only: false,
        sort: Sort::Title,
    }
}

#[test]
fn search_matches_title_ignoring_case() {
    let cards = vec![card("RJ1", "Apple Quest"), card("RJ2", "Cherry Blossom")];

    let order = browse(&cards, &searching("APPLE"));

    assert_eq!(ids(&cards, &order), ["RJ1"]);
}

#[test]
fn search_matches_rj_code() {
    let cards = vec![card("RJ111111", "Alpha"), card("RJ222222", "Beta")];

    let order = browse(&cards, &searching("rj2222"));

    assert_eq!(ids(&cards, &order), ["RJ222222"]);
}

#[test]
fn search_matches_circle() {
    let mut with_circle = card("RJ1", "Alpha");
    with_circle.circle = Some("Studio Moonlight".to_owned());
    let cards = vec![with_circle, card("RJ2", "Beta")];

    let order = browse(&cards, &searching("moonlight"));

    assert_eq!(ids(&cards, &order), ["RJ1"]);
}

#[test]
fn empty_search_keeps_every_game() {
    let cards = vec![card("RJ1", "Alpha"), card("RJ2", "Beta")];

    let order = browse(&cards, &searching("  "));

    assert_eq!(ids(&cards, &order), ["RJ1", "RJ2"]);
}

#[test]
fn favorites_only_hides_the_rest() {
    let mut fav = card("RJ1", "Alpha");
    fav.favorite = true;
    let cards = vec![fav, card("RJ2", "Beta")];
    let mut q = query(Sort::Title);
    q.favorites_only = true;

    let order = browse(&cards, &q);

    assert_eq!(ids(&cards, &order), ["RJ1"]);
}

#[test]
fn search_and_favorites_filter_combine() {
    let mut alpha = card("RJ1", "Alpha");
    alpha.favorite = true;
    let mut apricot = card("RJ2", "Apricot");
    apricot.favorite = false;
    let mut alpine = card("RJ3", "Alpine");
    alpine.favorite = true;
    let cards = vec![alpha, apricot, alpine];
    let q = Query {
        text: "al".to_owned(),
        favorites_only: true,
        sort: Sort::Title,
    };

    let order = browse(&cards, &q);

    assert_eq!(ids(&cards, &order), ["RJ1", "RJ3"]);
}

#[test]
fn play_time_reads_as_dashes_until_the_game_was_played() {
    assert_eq!(format_play_time(None), "--");
}

#[test]
fn play_time_shows_hours_and_minutes() {
    assert_eq!(format_play_time(Some(3725)), "1h 02m");
    assert_eq!(format_play_time(Some(90)), "1m");
}
