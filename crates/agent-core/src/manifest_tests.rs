use super::*;

#[test]
fn parses_skill_manifest() {
    let manifest: ExtensionManifest = toml::from_str(
        r#"
id = "skill.code-review"
name = "Code Review"
version = "0.1.0"
description = "Review code changes"
extension_type = "skill"
triggers = ["review"]
prompt_templates = ["Check correctness and tests"]
required_tools = ["git.diff"]
required_permissions = ["filesystem.read"]
core_version = ">=0.1.0"
"#,
    )
    .unwrap();

    assert_eq!(manifest.id, "skill.code-review");
    assert_eq!(manifest.extension_type, ExtensionType::Skill);
    assert_eq!(manifest.required_tools, vec!["git.diff"]);
}
