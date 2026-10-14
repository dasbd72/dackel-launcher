use dlsite_launcher::blur::{MAX_SIDE, blurred_thumbnail};
use image::{ImageBuffer, Rgba};
use std::fs;

/// A harsh black/white checkerboard: as sharp as an image gets.
fn write_checkerboard(path: &std::path::Path, w: u32, h: u32) {
    let img = ImageBuffer::from_fn(w, h, |x, y| {
        if (x / 4 + y / 4) % 2 == 0 {
            Rgba([0u8, 0, 0, 255])
        } else {
            Rgba([255u8, 255, 255, 255])
        }
    });
    img.save(path).unwrap();
}

/// Mean squared difference between neighboring pixels: high for sharp images, low for
/// blurred ones. (Absolute differences would not do: blurring a clean edge spreads it
/// out without changing the total change across it.)
fn edge_energy(rgba: &[u8], w: usize, h: usize) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0;
    for y in 0..h {
        for x in 0..w - 1 {
            let a = rgba[(y * w + x) * 4] as f64;
            let b = rgba[(y * w + x + 1) * 4] as f64;
            total += (a - b) * (a - b);
            count += 1.0;
        }
    }
    total / count
}

#[test]
fn the_result_is_tiny_so_nothing_in_the_picture_can_be_made_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cover.png");
    write_checkerboard(&path, 560, 420);

    let blurred = blurred_thumbnail(&path).unwrap();

    assert!(blurred.width <= MAX_SIDE as usize && blurred.height <= MAX_SIDE as usize);
    assert_eq!(blurred.rgba.len(), blurred.width * blurred.height * 4);
}

#[test]
fn the_aspect_ratio_is_kept_so_it_fits_the_same_frame() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cover.png");
    write_checkerboard(&path, 560, 420);

    let blurred = blurred_thumbnail(&path).unwrap();

    let ratio = blurred.width as f64 / blurred.height as f64;
    assert!((ratio - 560.0 / 420.0).abs() < 0.1, "ratio was {ratio}");
}

#[test]
fn the_image_is_actually_blurred_not_just_shrunk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cover.png");
    // A checkerboard whose squares are bigger than one thumbnail pixel survives a plain
    // shrink with hard edges; the blur must smooth them.
    let img = ImageBuffer::from_fn(400, 400, |x, y| {
        if (x / 40 + y / 40) % 2 == 0 {
            Rgba([0u8, 0, 0, 255])
        } else {
            Rgba([255u8, 255, 255, 255])
        }
    });
    img.save(&path).unwrap();

    let blurred = blurred_thumbnail(&path).unwrap();
    let plain = image::open(&path)
        .unwrap()
        .thumbnail(MAX_SIDE, MAX_SIDE)
        .to_rgba8();

    let blurred_edges = edge_energy(&blurred.rgba, blurred.width, blurred.height);
    let plain_edges = edge_energy(
        plain.as_raw(),
        plain.width() as usize,
        plain.height() as usize,
    );
    assert!(
        blurred_edges < plain_edges * 0.7,
        "blurred {blurred_edges} vs plain {plain_edges}"
    );
}

#[test]
fn a_missing_or_corrupt_file_gives_nothing_never_the_real_image() {
    let dir = tempfile::tempdir().unwrap();
    assert!(blurred_thumbnail(&dir.path().join("missing.jpg")).is_none());

    let corrupt = dir.path().join("bad.jpg");
    fs::write(&corrupt, b"this is not an image").unwrap();
    assert!(blurred_thumbnail(&corrupt).is_none());
}

#[test]
fn the_same_image_always_blurs_the_same_way() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cover.png");
    write_checkerboard(&path, 300, 200);

    let a = blurred_thumbnail(&path).unwrap();
    let b = blurred_thumbnail(&path).unwrap();

    assert_eq!(a.rgba, b.rgba);
}
