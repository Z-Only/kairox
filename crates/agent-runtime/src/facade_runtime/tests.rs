// Test files split out of the historical `facade_runtime.rs` `mod tests {}`
// block; each module focuses on one slice of `LocalRuntime` behaviour.
mod support;

mod cancel_tests;
mod compaction_tests;
mod execution_mode_tests;
mod send_message_tests;
mod session_lifecycle_tests;
mod switch_model_tests;
