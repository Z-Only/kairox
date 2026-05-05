use crate::filesystem::{FsListTool, FsReadTool, FsWriteTool};
use crate::patch::PatchApplyTool;
use crate::registry::{Tool, ToolDefinition, ToolProvider};
use crate::search::RipgrepSearchTool;
use crate::shell::ShellExecTool;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct BuiltinProvider {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl BuiltinProvider {
    pub fn with_defaults(workspace_root: PathBuf) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        let shell = Box::new(ShellExecTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let search = Box::new(RipgrepSearchTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let patch = Box::new(PatchApplyTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let fs_read = Box::new(FsReadTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let fs_write = Box::new(FsWriteTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let fs_list = Box::new(FsListTool::new(workspace_root)) as Box<dyn Tool>;

        tools.insert(shell.definition().tool_id.clone(), Arc::from(shell));
        tools.insert(search.definition().tool_id.clone(), Arc::from(search));
        tools.insert(patch.definition().tool_id.clone(), Arc::from(patch));
        tools.insert(fs_read.definition().tool_id.clone(), Arc::from(fs_read));
        tools.insert(fs_write.definition().tool_id.clone(), Arc::from(fs_write));
        tools.insert(fs_list.definition().tool_id.clone(), Arc::from(fs_list));

        Self { tools }
    }
}

#[async_trait]
impl ToolProvider for BuiltinProvider {
    async fn list_tools(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<_> = self.tools.values().map(|t| t.definition()).collect();
        defs.sort_by(|a, b| a.tool_id.cmp(&b.tool_id));
        defs
    }

    async fn get_tool(&self, tool_id: &str) -> Option<Box<dyn Tool>> {
        self.tools
            .get(tool_id)
            .map(|arc| Box::new(crate::registry::ArcTool { inner: arc.clone() }) as Box<dyn Tool>)
    }

    fn name(&self) -> &str {
        "builtin"
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(tools.len(), 6);
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
}
