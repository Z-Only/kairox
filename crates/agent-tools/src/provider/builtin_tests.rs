use super::*;

#[tokio::test]
async fn builtin_provider_lists_all_tools() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tools = provider.list_tools().await;
    let tool_ids: Vec<&str> = tools.iter().map(|t| t.tool_id.as_str()).collect();
    assert!(
        tool_ids.contains(&"shell.exec"),
        "missing shell.exec, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"search.ripgrep"),
        "missing search.ripgrep, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"patch.apply"),
        "missing patch.apply, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"fs.read"),
        "missing fs.read, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"fs.write"),
        "missing fs.write, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"fs.list"),
        "missing fs.list, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"monitor.start"),
        "missing monitor.start, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"monitor.stop"),
        "missing monitor.stop, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"monitor.list"),
        "missing monitor.list, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"browser.batch"),
        "missing browser.batch, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"computer.use"),
        "missing computer.use, got: {:?}",
        tool_ids
    );
    assert_eq!(tools.len(), 12);
}

#[tokio::test]
async fn builtin_provider_gets_tool_by_id() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tool = provider.get_tool("shell.exec").await;
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().definition().tool_id, "shell.exec");
}

#[tokio::test]
async fn builtin_provider_returns_none_for_unknown() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tool = provider.get_tool("nonexistent").await;
    assert!(tool.is_none());
}
