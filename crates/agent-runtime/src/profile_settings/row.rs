use toml_edit::Item;

#[derive(Debug, Clone)]
pub(crate) struct ProfileSettingsRow {
    pub(crate) provider: String,
    pub(crate) model_id: String,
    pub(crate) enabled: bool,
    pub(crate) context_window: Option<u64>,
    pub(crate) output_limit: Option<u64>,
    pub(crate) temperature: Option<f32>,
    pub(crate) top_p: Option<f32>,
    pub(crate) top_k: Option<u32>,
    pub(crate) max_tokens: Option<u64>,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key_env: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) client_identity: Option<String>,
    /// Where this profile was found: "defaults", "profiles_toml", "user_config", "project_config"
    pub(crate) source: String,
    pub(crate) writable: bool,
}

pub(crate) fn profile_row_from_toml_table(
    item: &Item,
    source: &str,
    writable: bool,
) -> ProfileSettingsRow {
    let table = item.as_table();
    ProfileSettingsRow {
        provider: table
            .and_then(|t| t.get("provider"))
            .and_then(Item::as_str)
            .unwrap_or("")
            .to_string(),
        model_id: table
            .and_then(|t| t.get("model_id"))
            .and_then(Item::as_str)
            .unwrap_or("")
            .to_string(),
        enabled: table
            .and_then(|t| t.get("enabled"))
            .and_then(Item::as_bool)
            .unwrap_or(true),
        context_window: table
            .and_then(|t| t.get("context_window"))
            .and_then(Item::as_integer)
            .map(|v| v as u64),
        output_limit: table
            .and_then(|t| t.get("output_limit"))
            .and_then(Item::as_integer)
            .map(|v| v as u64),
        temperature: table
            .and_then(|t| t.get("temperature"))
            .and_then(Item::as_float)
            .map(|v| v as f32),
        top_p: table
            .and_then(|t| t.get("top_p"))
            .and_then(Item::as_float)
            .map(|v| v as f32),
        top_k: table
            .and_then(|t| t.get("top_k"))
            .and_then(Item::as_integer)
            .map(|v| v as u32),
        max_tokens: table
            .and_then(|t| t.get("max_tokens"))
            .and_then(Item::as_integer)
            .map(|v| v as u64),
        base_url: table
            .and_then(|t| t.get("base_url"))
            .and_then(Item::as_str)
            .map(ToString::to_string),
        api_key_env: table
            .and_then(|t| t.get("api_key_env"))
            .and_then(Item::as_str)
            .map(ToString::to_string),
        api_key: table
            .and_then(|t| t.get("api_key"))
            .and_then(Item::as_str)
            .map(ToString::to_string),
        client_identity: table
            .and_then(|t| t.get("client_identity"))
            .and_then(Item::as_str)
            .map(ToString::to_string),
        source: source.to_string(),
        writable,
    }
}
