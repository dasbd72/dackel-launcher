use dackel_launcher::fonts::{FontSource, locate_in_dirs, parse_fc_match};
use std::fs;
use std::path::PathBuf;

#[test]
fn fc_match_output_gives_the_font_file_and_collection_index() {
    let out = "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc\n2\n";

    assert_eq!(
        parse_fc_match(out),
        Some(FontSource {
            path: PathBuf::from("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
            index: 2,
        })
    );
}

#[test]
fn fc_match_output_without_an_index_defaults_to_zero() {
    let out = "/usr/share/fonts/OTF/ipag.ttf";

    assert_eq!(parse_fc_match(out).unwrap().index, 0);
}

#[test]
fn empty_fc_match_output_is_no_font() {
    assert_eq!(parse_fc_match(""), None);
    assert_eq!(parse_fc_match("\n"), None);
}

#[test]
fn directory_scan_finds_a_known_japanese_font_in_nested_folders() {
    let root = tempfile::tempdir().unwrap();
    let nested = root.path().join("opentype").join("noto");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("DejaVuSans.ttf"), "").unwrap();
    fs::write(nested.join("NotoSansCJK-Regular.ttc"), "").unwrap();

    let found = locate_in_dirs(&[root.path().to_path_buf()]).unwrap();

    assert_eq!(found.path, nested.join("NotoSansCJK-Regular.ttc"));
}

#[test]
fn directory_scan_without_a_japanese_font_finds_nothing() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("DejaVuSans.ttf"), "").unwrap();

    assert_eq!(locate_in_dirs(&[root.path().to_path_buf()]), None);
}
