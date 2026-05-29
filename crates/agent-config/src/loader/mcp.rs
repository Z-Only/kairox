use crate::{ConfigError, McpServerConfig, McpTransportType};

pub(super) fn parse_mcp_servers(
    mcp_servers_toml: &toml::value::Table,
    path_for_errors: &str,
) -> Result<Vec<(String, McpServerConfig)>, ConfigError> {
    let mut mcp_servers = Vec::new();

    for (id, value) in mcp_servers_toml {
        let server_config: McpServerConfig =
            value.clone().try_into().map_err(|e| ConfigError::Parse {
                path: path_for_errors.to_string(),
                message: format!("mcp_server '{}': {}", id, e),
            })?;

        validate_required_fields(id, &server_config, path_for_errors)?;
        mcp_servers.push((id.clone(), server_config));
    }

    Ok(mcp_servers)
}

fn validate_required_fields(
    id: &str,
    server_config: &McpServerConfig,
    path_for_errors: &str,
) -> Result<(), ConfigError> {
    match &server_config.r#type {
        McpTransportType::Stdio if server_config.command.is_none() => Err(ConfigError::Parse {
            path: path_for_errors.to_string(),
            message: format!("mcp_server '{}': stdio requires 'command'", id),
        }),
        McpTransportType::Sse if server_config.url.is_none() => Err(ConfigError::Parse {
            path: path_for_errors.to_string(),
            message: format!("mcp_server '{}': sse requires 'url'", id),
        }),
        McpTransportType::StreamableHttp if server_config.url.is_none() => {
            Err(ConfigError::Parse {
                path: path_for_errors.to_string(),
                message: format!("mcp_server '{}': streamable_http requires 'url'", id),
            })
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
