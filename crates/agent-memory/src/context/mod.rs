//! Context assembly: build a token-bounded prompt context from a request,
//! enforcing per-source caps and a global drop-by-priority pass.

mod assembler;
mod budget;
pub mod image_pruning;
mod window;

#[cfg(test)]
mod tests;

pub use agent_core::{ContextSource, ContextUsage};
pub use assembler::{ContextAssembler, ContextBundle, ContextRequest};
pub use budget::ContextBudget;
pub use image_pruning::{ImageEntry, ImagePruningStrategy};
