//! Helpers and mapping logic for MCP Registry entries.
//!
//! Pure functions — no IO, no caching, no network. Translates the API DTOs
//! defined in [`super::types`] into internal [`ServerEntry`] values.

use super::types::{McpPackage, McpServerWrapper};
use crate::catalog::remote::RemoteError;
use crate::catalog::{
    EnvVarSpec, InstallSpec, RuntimeKind, RuntimeRequirement, ServerEntry, TrustLevel,
};
use std::collections::BTreeMap;

// ── Helpers ──────────────────────────────────────────────────────────

pub fn first_sentence(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    let cut = trimmed
        .find(['.', '\n'])
        .map(|i| &trimmed[..i])
        .unwrap_or(trimmed);
    if cut.chars().count() <= max_chars {
        cut.to_string()
    } else {
        cut.chars().take(max_chars).collect::<String>() + "…"
    }
}

/// Returns `true` when the `_meta` block indicates this is the latest
/// published version. Entries without the flag default to `true` so we
/// don't accidentally drop servers whose registry metadata is incomplete.
pub fn is_latest(meta: &Option<serde_json::Value>) -> bool {
    meta.as_ref()
        .and_then(|m| m.get("io.modelcontextprotocol.registry/official"))
        .and_then(|official| official.get("isLatest"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

pub fn infer_runtime_from_package(pkg: &McpPackage) -> Option<RuntimeRequirement> {
    let kind = match pkg.registry_type.as_str() {
        "npm" => RuntimeKind::Node,
        "pypi" => RuntimeKind::Python,
        _ => return None,
    };
    Some(RuntimeRequirement {
        kind,
        min_version: None,
        install_hint: None,
    })
}

// ── Mapping ──────────────────────────────────────────────────────────

pub fn map_mcp_to_entry(
    source_id: &str,
    wrapper: &McpServerWrapper,
    trust_ceiling: TrustLevel,
) -> Result<ServerEntry, RemoteError> {
    let srv = &wrapper.server;
    let id = srv.name.clone();
    let display_name = srv
        .title
        .clone()
        .unwrap_or_else(|| srv.name.rsplit('/').next().unwrap_or(&srv.name).to_string());
    let description = srv.description.clone().unwrap_or_default();
    let summary = if description.is_empty() {
        display_name.clone()
    } else {
        first_sentence(&description, 200)
    };

    // Build InstallSpec: prefer remote endpoints, fall back to packages.
    let install = if let Some(remote) = srv.remotes.first() {
        let headers: BTreeMap<String, String> = remote
            .headers
            .iter()
            .filter_map(|h| h.name.clone().map(|n| (n, String::new())))
            .collect();
        match remote.transport_type.as_str() {
            "streamable-http" => InstallSpec::StreamableHttp {
                url: remote.url.clone(),
                headers,
            },
            _ => InstallSpec::Sse {
                url: remote.url.clone(),
                headers,
            },
        }
    } else if let Some(pkg) = srv.packages.first() {
        build_install_from_package(pkg)
    } else {
        // No connection info at all — placeholder.
        InstallSpec::Stdio {
            command: srv.name.clone(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        }
    };

    // Trust: the official registry is curated; treat all entries as
    // community level, clamped by the source ceiling.
    let trust = TrustLevel::Community.min(trust_ceiling);

    // Runtime requirements inferred from packages.
    let mut requirements: Vec<RuntimeRequirement> = Vec::new();
    for pkg in &srv.packages {
        if let Some(req) = infer_runtime_from_package(pkg) {
            if !requirements.iter().any(|r| r.kind == req.kind) {
                requirements.push(req);
            }
        }
    }

    // Environment variables from packages.
    let mut default_env: Vec<EnvVarSpec> = Vec::new();
    for pkg in &srv.packages {
        for ev in &pkg.environment_variables {
            let key = match &ev.name {
                Some(k) if !k.is_empty() => k.clone(),
                _ => continue,
            };
            if default_env.iter().any(|e| e.key == key) {
                continue;
            }
            default_env.push(EnvVarSpec {
                key: key.clone(),
                label: key,
                description: ev.description.clone().unwrap_or_default(),
                required: ev.is_required.unwrap_or(false),
                secret: ev.is_secret.unwrap_or(false),
                default: None,
            });
        }
    }

    // Headers from remote endpoints — surfaced as configurable fields.
    for remote in &srv.remotes {
        for h in &remote.headers {
            let name = match &h.name {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };
            if default_env.iter().any(|e| e.key == *name) {
                continue;
            }
            default_env.push(EnvVarSpec {
                key: name.clone(),
                label: name.clone(),
                description: h.description.clone().unwrap_or_default(),
                required: h.is_required.unwrap_or(false),
                secret: h.is_secret.unwrap_or(false),
                default: None,
            });
        }
    }

    let homepage = srv
        .website_url
        .clone()
        .or_else(|| srv.repository.as_ref().and_then(|r| r.url.clone()));

    Ok(ServerEntry {
        id,
        source: source_id.to_string(),
        display_name,
        summary,
        description,
        categories: vec![],
        tags: vec![],
        author: None,
        homepage,
        version: srv.version.clone(),
        install,
        requirements,
        trust,
        default_env,
        icon: None,
        verified: false,
    })
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;

pub fn build_install_from_package(pkg: &McpPackage) -> InstallSpec {
    let is_stdio = pkg
        .transport
        .as_ref()
        .map(|t| t.transport_type == "stdio")
        .unwrap_or(true);

    if is_stdio {
        let (command, args) = match pkg.registry_type.as_str() {
            "npm" => (
                "npx".to_string(),
                vec!["-y".to_string(), pkg.identifier.clone()],
            ),
            "pypi" => ("uvx".to_string(), vec![pkg.identifier.clone()]),
            _ => (pkg.identifier.clone(), vec![]),
        };
        InstallSpec::Stdio {
            command,
            args,
            env: BTreeMap::new(),
            cwd: None,
        }
    } else {
        // Non-stdio package transport — unlikely but handle gracefully.
        InstallSpec::Stdio {
            command: pkg.identifier.clone(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        }
    }
}
