//! Image pruning strategies for multimodal context management.
//!
//! When a conversation includes many images (screenshots, diagrams, etc.)
//! they can quickly exhaust the context window. These strategies let the
//! [`ContextAssembler`](super::assembler::ContextAssembler) trim images
//! before the global budget-enforcement pass.

/// An image entry in the conversation, ordered by position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageEntry {
    /// Position index in the conversation (for ordering).
    pub position: usize,
    /// Estimated token cost of this image.
    pub estimated_tokens: u64,
    /// The image content/reference (opaque to the pruner).
    pub content: String,
}

/// Strategy for pruning images when context is tight.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ImagePruningStrategy {
    /// Keep all images (default — no pruning).
    #[default]
    None,
    /// Keep only the N most recent images, remove older ones.
    StripOldestImages { keep: usize },
    /// Keep images at regular intervals: first, every Kth, and last.
    StripImagesAtIntervals { interval: usize },
}

/// Apply an [`ImagePruningStrategy`] to a list of image entries in-place.
///
/// Entries are assumed to be sorted by `position` (ascending). The function
/// preserves that invariant.
pub fn prune_images(entries: &mut Vec<ImageEntry>, strategy: &ImagePruningStrategy) {
    match strategy {
        ImagePruningStrategy::None => { /* keep all */ }
        ImagePruningStrategy::StripOldestImages { keep } => {
            if *keep == 0 {
                entries.clear();
                return;
            }
            let len = entries.len();
            if len <= *keep {
                return;
            }
            // Keep the last `keep` entries (most recent by position).
            let start = len - keep;
            *entries = entries.split_off(start);
        }
        ImagePruningStrategy::StripImagesAtIntervals { interval } => {
            if entries.is_empty() || *interval <= 1 {
                // interval=0 or 1 means keep everything.
                return;
            }
            let len = entries.len();
            if len <= 1 {
                return;
            }
            let last_idx = len - 1;
            let kept: Vec<ImageEntry> = entries
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    // Always keep first
                    *i == 0
                    // Keep every interval-th element
                    || *i % interval == 0
                    // Always keep last
                    || *i == last_idx
                })
                .map(|(_, e)| e.clone())
                .collect();
            *entries = kept;
        }
    }
}

#[cfg(test)]
#[path = "image_pruning_tests.rs"]
mod tests;
