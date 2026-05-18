mod document;
mod lifecycle;
mod operations;
mod rows;

#[cfg(test)]
mod tests;

pub use lifecycle::McpSettingsLifecycle;
pub use operations::{
    delete_mcp_server_settings, delete_mcp_server_settings_in_file, get_mcp_disabled_tools,
    list_mcp_server_settings, set_mcp_server_enabled, set_mcp_server_enabled_in_file,
    set_mcp_tool_disabled_in_file, upsert_mcp_server_settings, upsert_mcp_server_settings_in_file,
    writable_mcp_config_path,
};

const CONFIG_FILE_NAME: &str = "config.toml";
