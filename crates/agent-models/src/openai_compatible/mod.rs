pub mod client;
pub mod config;
pub mod request;
pub mod streaming;
pub mod tool_accumulator;
mod tool_names;

pub use client::OpenAiCompatibleClient;
pub use config::OpenAiCompatibleConfig;

#[cfg(test)]
mod tests;
