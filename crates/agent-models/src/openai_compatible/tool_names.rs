use crate::ToolDefinition;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub(super) struct OpenAiToolNameMap {
    internal_to_wire: HashMap<String, String>,
    wire_to_internal: HashMap<String, String>,
}

impl OpenAiToolNameMap {
    pub(super) fn from_tools(tools: &[ToolDefinition]) -> Self {
        let mut internal_to_wire = HashMap::new();
        let mut wire_to_internal = HashMap::new();
        let mut used_wire_names = HashSet::new();

        for tool in tools {
            if internal_to_wire.contains_key(&tool.name) {
                continue;
            }

            let base = sanitize_openai_tool_name(&tool.name);
            let mut wire_name = base.clone();
            let mut suffix = 2;
            while used_wire_names.contains(&wire_name) {
                wire_name = format!("{base}_{suffix}");
                suffix += 1;
            }

            used_wire_names.insert(wire_name.clone());
            internal_to_wire.insert(tool.name.clone(), wire_name.clone());
            wire_to_internal.insert(wire_name, tool.name.clone());
        }

        Self {
            internal_to_wire,
            wire_to_internal,
        }
    }

    pub(super) fn wire_name(&self, internal_name: &str) -> String {
        self.internal_to_wire
            .get(internal_name)
            .cloned()
            .unwrap_or_else(|| sanitize_openai_tool_name(internal_name))
    }

    pub(super) fn internal_name(&self, wire_name: &str) -> String {
        self.wire_to_internal
            .get(wire_name)
            .cloned()
            .unwrap_or_else(|| wire_name.to_string())
    }
}

fn sanitize_openai_tool_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
#[path = "tool_names_tests.rs"]
mod tests;
