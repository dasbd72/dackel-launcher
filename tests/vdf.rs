//! Steam's binary key-value format, as used by `shortcuts.vdf`.

use dackel_launcher::vdf::{Value, parse, write};

/// `"shortcuts" { "0" { "AppName" "Game" "appid" 7 } }`
fn sample() -> Vec<u8> {
    let mut b = Vec::new();
    b.push(0x00);
    b.extend(b"shortcuts\0");
    b.push(0x00);
    b.extend(b"0\0");
    b.push(0x01);
    b.extend(b"AppName\0Game\0");
    b.push(0x02);
    b.extend(b"appid\0");
    b.extend(7u32.to_le_bytes());
    b.push(0x08); // end of "0"
    b.push(0x08); // end of "shortcuts"
    b.push(0x08); // end of the file
    b
}

#[test]
fn parsing_gives_the_nested_keys_and_values_in_order() {
    let root = parse(&sample()).unwrap();

    let shortcuts = root.get("shortcuts").unwrap();
    let first = shortcuts.get("0").unwrap();
    assert_eq!(first.get("AppName"), Some(&Value::Str("Game".to_owned())));
    assert_eq!(first.get("appid"), Some(&Value::U32(7)));
}

#[test]
fn writing_what_was_parsed_gives_the_same_bytes() {
    assert_eq!(write(&parse(&sample()).unwrap()), sample());
}

#[test]
fn values_of_other_types_survive_a_round_trip() {
    let mut b = Vec::new();
    b.push(0x03);
    b.extend(b"ratio\0");
    b.extend(1.5f32.to_le_bytes());
    b.push(0x07);
    b.extend(b"big\0");
    b.extend(u64::MAX.to_le_bytes());
    b.push(0x0A);
    b.extend(b"signed\0");
    b.extend((-5i64).to_le_bytes());
    b.push(0x08);

    assert_eq!(write(&parse(&b).unwrap()), b);
}

#[test]
fn a_truncated_or_unknown_file_is_refused_not_guessed_at() {
    let whole = sample();
    for cut in [1, 5, whole.len() - 1] {
        assert!(parse(&whole[..cut]).is_err(), "cut at {cut}");
    }
    assert!(parse(&[0x42, b'k', 0, 0, 0, 0, 0, 0x08]).is_err());
    assert!(parse(&[]).is_err());
}

#[test]
fn text_that_is_not_utf8_is_refused_so_it_is_never_rewritten_wrongly() {
    let mut b = vec![0x01];
    b.extend(b"name\0");
    b.extend([0xFF, 0xFE, 0]);
    b.push(0x08);

    assert!(parse(&b).is_err());
}
