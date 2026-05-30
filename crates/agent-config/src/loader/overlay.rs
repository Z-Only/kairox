use crate::{Config, ConfigError};

/// Load the main config, ignoring the removed marketplace MCP server overlay.
///
/// `marketplace_content` is accepted only so older call sites that also parse
/// catalog sources can keep a single loading path. MCP server definitions are
/// loaded exclusively from `config.toml`.
pub fn load_with_marketplace_overlay(
    main_content: &str,
    marketplace_content: Option<&str>,
    main_path: &str,
    _marketplace_path: &str,
) -> Result<Config, ConfigError> {
    let cfg = super::load_from_str(main_content, main_path)?;

    let _ = marketplace_content;
    Ok(cfg)
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
