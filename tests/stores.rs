use dackel_launcher::library::{Store, store_of};

#[test]
fn an_rj_id_belongs_to_dlsite() {
    assert_eq!(store_of("RJ01389782"), Some(Store::DLsite));
    assert_eq!(store_of("RJ1"), Some(Store::DLsite));
}

#[test]
fn an_ra_id_belongs_to_072project() {
    assert_eq!(store_of("RA25128779"), Some(Store::Project072));
}

#[test]
fn anything_else_has_no_store() {
    for id in [
        "RJ",
        "RA",
        "RAab12",
        "rj123",
        "ra123",
        "Some Other Game",
        "temp_RA123",
    ] {
        assert_eq!(store_of(id), None, "{id}");
    }
}

#[test]
fn stores_have_the_names_shown_to_the_user() {
    assert_eq!(Store::DLsite.label(), "DLsite");
    assert_eq!(Store::Project072.label(), "072Project");
}
