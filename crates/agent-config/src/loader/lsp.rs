use crate::{ConfigError, DapServerConfig, LspServerConfig};

pub(super) fn parse_lsp_servers(
    table: &toml::value::Table,
    path_for_errors: &str,
) -> Result<Vec<(String, LspServerConfig)>, ConfigError> {
    let mut servers = Vec::new();
    for (id, value) in table {
        let config: LspServerConfig = value.clone().try_into().map_err(|e| ConfigError::Parse {
            path: path_for_errors.to_string(),
            message: format!("lsp_server '{}': {}", id, e),
        })?;
        servers.push((id.clone(), config));
    }
    Ok(servers)
}

pub(super) fn parse_dap_servers(
    table: &toml::value::Table,
    path_for_errors: &str,
) -> Result<Vec<(String, DapServerConfig)>, ConfigError> {
    let mut servers = Vec::new();
    for (id, value) in table {
        let config: DapServerConfig = value.clone().try_into().map_err(|e| ConfigError::Parse {
            path: path_for_errors.to_string(),
            message: format!("dap_server '{}': {}", id, e),
        })?;
        servers.push((id.clone(), config));
    }
    Ok(servers)
}
