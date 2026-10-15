use dackel_launcher::steam_input::{HINT, Watch};

#[test]
fn arrow_keys_without_any_pad_event_under_steam_raise_the_hint_once() {
    let mut watch = Watch::new(true);

    assert_eq!(watch.arrow_pressed(), None);
    assert_eq!(watch.arrow_pressed(), None);
    assert_eq!(watch.arrow_pressed(), Some(HINT));
    assert_eq!(watch.arrow_pressed(), None);
}

#[test]
fn no_hint_when_not_started_from_steam() {
    let mut watch = Watch::new(false);

    assert!((0..10).all(|_| watch.arrow_pressed().is_none()));
}

#[test]
fn no_hint_once_the_pad_has_been_seen() {
    let mut watch = Watch::new(true);

    watch.pad_seen();

    assert!((0..10).all(|_| watch.arrow_pressed().is_none()));
}
