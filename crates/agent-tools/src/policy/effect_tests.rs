use std::path::PathBuf;

use super::*;

#[test]
fn builders_accept_cow_tool_ids() {
    use std::borrow::Cow;

    assert_eq!(
        PolicyRisk::read(Cow::Borrowed("fs.read")).tool_id,
        "fs.read"
    );
    assert_eq!(
        PolicyRisk::write(Cow::Borrowed("fs.write")).effect,
        PolicyEffect::Write { paths: Vec::new() }
    );
    assert_eq!(
        PolicyRisk::write_paths(Cow::Borrowed("fs.write"), Vec::new()).effect,
        PolicyEffect::Write { paths: Vec::new() }
    );
    assert_eq!(
        PolicyRisk::shell(Cow::Borrowed("shell.exec"), false).effect,
        PolicyEffect::Shell { destructive: false }
    );
    assert_eq!(
        PolicyRisk::destructive(Cow::Borrowed("shell.exec")).effect,
        PolicyEffect::Destructive
    );
    assert_eq!(
        PolicyRisk::network(Cow::Borrowed("browser.action"), Vec::new()).effect,
        PolicyEffect::Network { hosts: Vec::new() }
    );
    assert_eq!(
        PolicyRisk::mcp(Cow::Borrowed("mcp_tool"), Cow::Borrowed("my-server")).effect,
        PolicyEffect::McpInvoke {
            server: "my-server".to_string()
        }
    );
}

#[test]
fn read_builder() {
    let risk = PolicyRisk::read("fs.read");
    assert_eq!(risk.tool_id, "fs.read");
    assert_eq!(risk.effect, PolicyEffect::Read);
}

#[test]
fn read_builder_accepts_string() {
    let risk = PolicyRisk::read(String::from("fs.read"));
    assert_eq!(risk.tool_id, "fs.read");
    assert_eq!(risk.effect, PolicyEffect::Read);
}

#[test]
fn write_builder_empty_paths() {
    let risk = PolicyRisk::write("fs.write");
    assert_eq!(risk.tool_id, "fs.write");
    assert_eq!(risk.effect, PolicyEffect::Write { paths: Vec::new() });
}

#[test]
fn write_paths_builder() {
    let paths = vec![PathBuf::from("/tmp/a.txt"), PathBuf::from("/tmp/b.txt")];
    let risk = PolicyRisk::write_paths("fs.write", paths.clone());
    assert_eq!(risk.tool_id, "fs.write");
    assert_eq!(risk.effect, PolicyEffect::Write { paths });
}

#[test]
fn write_paths_builder_empty_vec() {
    let risk = PolicyRisk::write_paths("fs.write", Vec::new());
    assert_eq!(risk.effect, PolicyEffect::Write { paths: Vec::new() });
}

#[test]
fn shell_builder_non_destructive() {
    let risk = PolicyRisk::shell("shell.exec", false);
    assert_eq!(risk.tool_id, "shell.exec");
    assert_eq!(risk.effect, PolicyEffect::Shell { destructive: false });
}

#[test]
fn shell_builder_destructive() {
    let risk = PolicyRisk::shell("shell.exec", true);
    assert_eq!(risk.tool_id, "shell.exec");
    assert_eq!(risk.effect, PolicyEffect::Shell { destructive: true });
}

#[test]
fn destructive_builder() {
    let risk = PolicyRisk::destructive("shell.exec");
    assert_eq!(risk.tool_id, "shell.exec");
    assert_eq!(risk.effect, PolicyEffect::Destructive);
}

#[test]
fn network_builder() {
    let hosts = vec!["api.example.com".to_string(), "cdn.example.com".to_string()];
    let risk = PolicyRisk::network("browser.action", hosts.clone());
    assert_eq!(risk.tool_id, "browser.action");
    assert_eq!(risk.effect, PolicyEffect::Network { hosts });
}

#[test]
fn network_builder_empty_hosts() {
    let risk = PolicyRisk::network("browser.action", Vec::new());
    assert_eq!(risk.effect, PolicyEffect::Network { hosts: Vec::new() });
}

#[test]
fn mcp_builder() {
    let risk = PolicyRisk::mcp("mcp_tool", "my-server");
    assert_eq!(risk.tool_id, "mcp_tool");
    assert_eq!(
        risk.effect,
        PolicyEffect::McpInvoke {
            server: "my-server".to_string()
        }
    );
}

#[test]
fn mcp_builder_accepts_string_args() {
    let risk = PolicyRisk::mcp(String::from("mcp_tool"), String::from("my-server"));
    assert_eq!(risk.tool_id, "mcp_tool");
    assert_eq!(
        risk.effect,
        PolicyEffect::McpInvoke {
            server: "my-server".to_string()
        }
    );
}

#[test]
fn policy_effect_equality() {
    assert_eq!(PolicyEffect::Read, PolicyEffect::Read);
    assert_eq!(PolicyEffect::Destructive, PolicyEffect::Destructive);
    assert_ne!(PolicyEffect::Read, PolicyEffect::Destructive);
    assert_ne!(
        PolicyEffect::Shell { destructive: true },
        PolicyEffect::Shell { destructive: false }
    );
}

#[test]
fn policy_risk_equality() {
    let a = PolicyRisk::read("fs.read");
    let b = PolicyRisk::read("fs.read");
    let c = PolicyRisk::read("fs.write");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn policy_risk_clone() {
    let original = PolicyRisk::write_paths("fs.write", vec![PathBuf::from("/tmp/x")]);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}
