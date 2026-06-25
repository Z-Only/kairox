use super::*;

fn tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: String::new(),
        parameters: serde_json::json!({"type": "object"}),
    }
}

#[test]
fn maps_dotted_internal_name_to_openai_safe_wire_name() {
    let names = OpenAiToolNameMap::from_tools(&[tool("fs.read")]);

    assert_eq!(names.wire_name("fs.read"), "fs_read");
    assert_eq!(names.internal_name("fs_read"), "fs.read");
}

#[test]
fn leaves_already_safe_tool_name_unchanged() {
    let names = OpenAiToolNameMap::from_tools(&[tool("shell_exec")]);

    assert_eq!(names.wire_name("shell_exec"), "shell_exec");
    assert_eq!(names.internal_name("shell_exec"), "shell_exec");
}

#[test]
fn disambiguates_colliding_sanitized_names() {
    let names = OpenAiToolNameMap::from_tools(&[tool("fs.read"), tool("fs_read")]);

    assert_eq!(names.wire_name("fs.read"), "fs_read");
    assert_eq!(names.wire_name("fs_read"), "fs_read_2");
    assert_eq!(names.internal_name("fs_read"), "fs.read");
    assert_eq!(names.internal_name("fs_read_2"), "fs_read");
}
