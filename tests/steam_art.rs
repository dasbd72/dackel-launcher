//! The images written for a game's Steam entry: the cover as capsules, a hero and an icon.

use dackel_launcher::steam_art::{IconSource, cover_hero, cover_icon, exe_icon, write_game_art};
use image::{DynamicImage, GenericImageView, Rgb, RgbImage};
use std::fs;
use std::path::PathBuf;

fn icon_exe() -> Vec<u8> {
    fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/icon.exe")).unwrap()
}

/// 100x200: top third green, middle red, bottom blue.
fn banded() -> DynamicImage {
    let mut img = RgbImage::new(100, 200);
    for (_, y, px) in img.enumerate_pixels_mut() {
        *px = match y {
            0..=66 => Rgb([0, 200, 0]),
            67..=133 => Rgb([200, 0, 0]),
            _ => Rgb([0, 0, 200]),
        };
    }
    DynamicImage::ImageRgb8(img)
}

fn save_cover(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("cover.png");
    banded().save(&path).unwrap();
    path
}

#[test]
fn the_icon_of_an_exe_is_read_from_its_resources() {
    let icon = exe_icon(&icon_exe()).unwrap();

    assert_eq!(icon.dimensions(), (8, 8));
    let [r, g, b, _] = icon.get_pixel(3, 3).0;
    assert!(r > 150 && g < 80 && b < 80, "{r} {g} {b}");
}

#[test]
fn bytes_that_are_not_a_program_or_have_no_icon_are_an_error() {
    assert!(exe_icon(b"not a program").is_err());
    let exe = icon_exe();
    assert!(
        exe_icon(&exe[..0x200]).is_err(),
        "headers only, no resources"
    );
}

#[test]
fn the_cover_icon_is_the_middle_square_of_the_cover() {
    let icon = cover_icon(&banded());

    assert_eq!(icon.dimensions(), (256, 256));
    let [r, g, b, _] = icon.get_pixel(128, 128).0;
    assert!(
        r > 150 && g < 60 && b < 60,
        "centre is the red band: {r} {g} {b}"
    );
}

#[test]
fn the_hero_is_the_right_size_and_a_soft_version_of_the_cover() {
    let hero = cover_hero(&banded());

    assert_eq!(hero.dimensions(), (3840, 1240));
    // Blurred: the hard edge between bands is gone, so no pixel is pure at a band boundary.
    let [r, g, b, a] = hero.get_pixel(1920, 620).0;
    assert_eq!(a, 255);
    assert!(r > 100, "the middle of the cover is red: {r} {g} {b}");
}

#[test]
fn a_game_gets_capsules_a_hero_and_the_cover_as_icon() {
    let dir = tempfile::tempdir().unwrap();
    let grid = dir.path().join("grid");

    let used = write_game_art(&grid, 9, &save_cover(dir.path()), IconSource::Cover, None).unwrap();

    assert_eq!(used, IconSource::Cover);
    for (name, size) in [
        ("9.png", (100, 200)),
        ("9p.png", (100, 200)),
        ("9_hero.png", (3840, 1240)),
        ("9_icon.png", (256, 256)),
    ] {
        let img = image::open(grid.join(name)).unwrap();
        assert_eq!(img.dimensions(), size, "{name}");
    }
}

#[test]
fn the_icon_can_come_from_the_exe_instead() {
    let dir = tempfile::tempdir().unwrap();
    let grid = dir.path().join("grid");
    let exe = dir.path().join("Game.exe");
    fs::write(&exe, icon_exe()).unwrap();

    let used = write_game_art(
        &grid,
        9,
        &save_cover(dir.path()),
        IconSource::Exe,
        Some(&exe),
    )
    .unwrap();

    assert_eq!(used, IconSource::Exe);
    assert_eq!(
        image::open(grid.join("9_icon.png")).unwrap().dimensions(),
        (8, 8)
    );
}

#[test]
fn an_exe_without_an_icon_falls_back_to_the_cover_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let grid = dir.path().join("grid");
    let exe = dir.path().join("Game.exe");
    fs::write(&exe, b"not a program").unwrap();

    for exe in [Some(exe.as_path()), None] {
        let used = write_game_art(&grid, 9, &save_cover(dir.path()), IconSource::Exe, exe).unwrap();

        assert_eq!(used, IconSource::Cover);
        assert_eq!(
            image::open(grid.join("9_icon.png")).unwrap().dimensions(),
            (256, 256)
        );
    }
}
