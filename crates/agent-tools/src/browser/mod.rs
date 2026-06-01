pub mod actions;
pub mod batch;
pub mod playwright;
pub mod tool;
pub mod types;

pub use batch::BrowserBatchTool;
pub use tool::BrowserTool;
pub use types::{BrowserAction, BrowserResult, BrowserState};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
