use super::*;

#[test]
fn instructions_view_serializes() {
    let view = InstructionsView {
        system: "You are helpful.".into(),
        user: Some("Be concise.".into()),
        project: None,
    };
    let json = serde_json::to_string(&view).unwrap();
    assert!(json.contains("You are helpful."));
    assert!(json.contains("Be concise."));
}

#[test]
fn mcp_settings_input_serializes_stdio_transport() {
    let input = McpServerSettingsInput {
        name: "filesystem".to_string(),
        transport: McpServerSettingsTransport::Stdio {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ],
            env: BTreeMap::from([("ROOT".to_string(), "/tmp".to_string())]),
        },
        enabled: true,
        description: Some("Local files".to_string()),
    };

    let encoded = serde_json::to_string(&input).expect("input should serialize");
    assert!(encoded.contains("filesystem"));
    assert!(encoded.contains("stdio"));
}
