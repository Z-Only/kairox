use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use agent_core::facade::{McpServerSettingsInput, McpServerSettingsTransport};
use agent_core::CoreError;
use toml_edit::{value, Array, DocumentMut, Item, Table};

pub(super) async fn mutate_mcp_config<F>(config_path: &Path, mutate: F) -> agent_core::Result<()>
where
    F: FnOnce(&mut DocumentMut) -> agent_core::Result<()>,
{
    let raw = match tokio::fs::read_to_string(config_path).await {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(CoreError::InvalidState(format!(
                "failed to read MCP config: {error}"
            )));
        }
    };
    let mut document = parse_document(&raw)?;
    mutate(&mut document)?;

    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            CoreError::InvalidState(format!("failed to create MCP config directory: {error}"))
        })?;
    }
    tokio::fs::write(config_path, document.to_string())
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to write MCP config: {error}")))
}

pub(super) fn parse_document(raw: &str) -> agent_core::Result<DocumentMut> {
    raw.parse::<DocumentMut>()
        .map_err(|error| CoreError::InvalidState(format!("failed to parse MCP config: {error}")))
}

pub(super) fn upsert_server_table(document: &mut DocumentMut, input: &McpServerSettingsInput) {
    let server_table = ensure_server_table(document, &input.name);
    server_table["enabled"] = value(input.enabled);
    match &input.description {
        Some(description) => server_table["description"] = value(description.clone()),
        None => {
            server_table.remove("description");
        }
    }

    match &input.transport {
        McpServerSettingsTransport::Stdio { command, args, env } => {
            server_table["type"] = value("stdio");
            server_table["command"] = value(command.clone());
            server_table["args"] = value(string_array(args));
            replace_optional_table(server_table, "env", env);
            server_table.remove("url");
            server_table.remove("headers");
        }
        McpServerSettingsTransport::Sse { url, headers } => {
            server_table["type"] = value("sse");
            server_table["url"] = value(url.clone());
            replace_optional_table(server_table, "headers", headers);
            server_table.remove("command");
            server_table.remove("args");
            server_table.remove("env");
        }
        McpServerSettingsTransport::StreamableHttp { url, headers } => {
            server_table["type"] = value("streamable_http");
            server_table["url"] = value(url.clone());
            replace_optional_table(server_table, "headers", headers);
            server_table.remove("command");
            server_table.remove("args");
            server_table.remove("env");
        }
    }
}

pub(super) fn ensure_server_table<'a>(
    document: &'a mut DocumentMut,
    server_id: &str,
) -> &'a mut Table {
    let servers_table = ensure_mcp_servers_table(document);
    if !servers_table.contains_key(server_id) || !servers_table[server_id].is_table() {
        servers_table[server_id] = Item::Table(Table::new());
    }
    servers_table[server_id]
        .as_table_mut()
        .expect("server table should exist")
}

pub(super) fn read_disabled_tools_from_document(
    document: &DocumentMut,
    server_id: &str,
) -> HashSet<String> {
    let Some(servers) = document["mcp_servers"].as_table() else {
        return HashSet::new();
    };
    let Some(server) = servers.get(server_id).and_then(|s| s.as_table()) else {
        return HashSet::new();
    };
    let Some(arr) = server.get("disabled_tools").and_then(|v| v.as_array()) else {
        return HashSet::new();
    };
    arr.iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect()
}

fn ensure_mcp_servers_table(document: &mut DocumentMut) -> &mut Table {
    if !document.as_table().contains_key("mcp_servers") || !document["mcp_servers"].is_table() {
        document["mcp_servers"] = Item::Table(Table::new());
    }
    document["mcp_servers"]
        .as_table_mut()
        .expect("mcp_servers table should exist")
}

fn replace_optional_table(server_table: &mut Table, key: &str, values: &BTreeMap<String, String>) {
    if values.is_empty() {
        server_table.remove(key);
        return;
    }

    server_table[key] = string_map_to_inline(values);
}

fn string_map_to_inline(values: &BTreeMap<String, String>) -> Item {
    let mut inline = toml_edit::InlineTable::new();
    for (key, value_text) in values {
        inline.insert(key, value_text.clone().into());
    }
    Item::Value(toml_edit::Value::InlineTable(inline))
}

fn string_array(values: &[String]) -> Array {
    let mut array = Array::default();
    for value_text in values {
        array.push(value_text.as_str());
    }
    array
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_map_to_inline_uses_equals_not_colon() {
        let input: BTreeMap<String, String> = BTreeMap::from([("REPO_PATH".into(), ".".into())]);
        let item = string_map_to_inline(&input);
        let rendered = item.to_string();
        assert!(
            !rendered.contains("\":"),
            "inline table must use '=' not ':':\n{rendered}",
        );
        // Also verify toml 1.1.2 can parse it.
        let table_str = format!("[t]\nenv = {rendered}");
        let parsed: toml::value::Table =
            toml::from_str(&table_str).expect("string_map_to_inline must produce valid TOML");
        let env = parsed["t"]["env"].as_table().unwrap();
        assert_eq!(env["REPO_PATH"].as_str(), Some("."));
    }
}
