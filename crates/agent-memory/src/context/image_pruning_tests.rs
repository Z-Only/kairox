use super::{prune_images, ImageEntry, ImagePruningStrategy};

fn make_entries(positions: &[usize]) -> Vec<ImageEntry> {
    positions
        .iter()
        .map(|&pos| ImageEntry {
            position: pos,
            estimated_tokens: 100,
            content: format!("image_{pos}"),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// ImagePruningStrategy::None
// ---------------------------------------------------------------------------

#[test]
fn none_strategy_keeps_all_images() {
    let mut entries = make_entries(&[0, 1, 2, 3, 4]);
    prune_images(&mut entries, &ImagePruningStrategy::None);
    assert_eq!(entries.len(), 5);
}

// ---------------------------------------------------------------------------
// StripOldestImages
// ---------------------------------------------------------------------------

#[test]
fn strip_oldest_keep_2_on_5_images_keeps_last_2() {
    let mut entries = make_entries(&[0, 1, 2, 3, 4]);
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripOldestImages { keep: 2 },
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].position, 3);
    assert_eq!(entries[1].position, 4);
}

#[test]
fn strip_oldest_keep_10_on_3_images_keeps_all() {
    let mut entries = make_entries(&[0, 1, 2]);
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripOldestImages { keep: 10 },
    );
    assert_eq!(entries.len(), 3);
}

#[test]
fn strip_oldest_keep_0_removes_all() {
    let mut entries = make_entries(&[0, 1, 2]);
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripOldestImages { keep: 0 },
    );
    assert!(entries.is_empty());
}

#[test]
fn strip_oldest_on_empty_is_noop() {
    let mut entries: Vec<ImageEntry> = Vec::new();
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripOldestImages { keep: 5 },
    );
    assert!(entries.is_empty());
}

// ---------------------------------------------------------------------------
// StripImagesAtIntervals
// ---------------------------------------------------------------------------

#[test]
fn strip_at_intervals_2_on_5_keeps_first_every_2nd_last() {
    let mut entries = make_entries(&[0, 1, 2, 3, 4]);
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripImagesAtIntervals { interval: 2 },
    );
    // indices 0, 2, 4 → positions 0, 2, 4
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].position, 0);
    assert_eq!(entries[1].position, 2);
    assert_eq!(entries[2].position, 4);
}

#[test]
fn strip_at_intervals_3_on_6_keeps_first_every_3rd_last() {
    let mut entries = make_entries(&[0, 1, 2, 3, 4, 5]);
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripImagesAtIntervals { interval: 3 },
    );
    // indices kept: 0 (first), 3 (every 3rd), 5 (last)
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].position, 0);
    assert_eq!(entries[1].position, 3);
    assert_eq!(entries[2].position, 5);
}

#[test]
fn strip_at_intervals_1_keeps_all() {
    let mut entries = make_entries(&[0, 1, 2, 3, 4]);
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripImagesAtIntervals { interval: 1 },
    );
    assert_eq!(entries.len(), 5);
}

#[test]
fn strip_at_intervals_on_empty_is_noop() {
    let mut entries: Vec<ImageEntry> = Vec::new();
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripImagesAtIntervals { interval: 3 },
    );
    assert!(entries.is_empty());
}

#[test]
fn strip_at_intervals_on_single_entry_keeps_it() {
    let mut entries = make_entries(&[42]);
    prune_images(
        &mut entries,
        &ImagePruningStrategy::StripImagesAtIntervals { interval: 5 },
    );
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].position, 42);
}

#[test]
fn default_strategy_is_none() {
    let strategy = ImagePruningStrategy::default();
    assert_eq!(strategy, ImagePruningStrategy::None);
}
