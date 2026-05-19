use std::path::Path;

use agent_core::facade::ProfileSettingsView;
use toml_edit::DocumentMut;

use super::write;

// -- display ordering helpers --

fn load_display_order(document: &DocumentMut) -> Vec<String> {
    document
        .get("display_order")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn save_display_order(document: &mut DocumentMut, order: &[String]) {
    let array =
        toml_edit::Array::from_iter(order.iter().map(|s| toml_edit::Value::from(s.clone())));
    document["display_order"] = toml_edit::Item::Value(toml_edit::Value::Array(array));
}

pub(crate) fn sort_by_display_order(views: &mut [ProfileSettingsView], display_order: &[String]) {
    views.sort_by(|a, b| {
        let pos_a = display_order.iter().position(|s| s == &a.alias);
        let pos_b = display_order.iter().position(|s| s == &b.alias);
        match (pos_a, pos_b) {
            (Some(pa), Some(pb)) => pa.cmp(&pb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.alias.cmp(&b.alias),
        }
    });
}

pub(super) fn load_display_order_from_doc(document: &DocumentMut) -> Vec<String> {
    load_display_order(document)
}

pub async fn move_profile_in_order(
    config_path: &Path,
    alias: &str,
    direction: i32, // -1 for up, +1 for down
) -> agent_core::Result<()> {
    write::mutate_profiles_config(config_path, |document| {
        let mut order = load_display_order(document);
        if let Some(pos) = order.iter().position(|s| s == alias) {
            let new_pos = if direction < 0 {
                pos.saturating_sub(1)
            } else {
                (pos + 1).min(order.len().saturating_sub(1))
            };
            if new_pos != pos {
                order.swap(pos, new_pos);
                save_display_order(document, &order);
            }
        } else {
            // Profile not in order yet — add it at the end
            order.push(alias.to_string());
            save_display_order(document, &order);
        }
        Ok(())
    })
    .await
}
