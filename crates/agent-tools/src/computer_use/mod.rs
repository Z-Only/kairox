pub mod platform;
pub mod tool;
pub mod types;

pub use tool::ComputerUseTool;
pub use types::{ComputerAction, ComputerResult, CursorPosition, ScreenSize};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
