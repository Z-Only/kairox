pub mod client;
pub mod config;
pub mod request;
pub mod streaming;
pub mod tool_accumulator;

pub use client::OpenAiCompatibleClient;
pub use config::OpenAiCompatibleConfig;

#[cfg(test)]
mod tests;
