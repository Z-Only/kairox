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
