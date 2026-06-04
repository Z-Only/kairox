use crate::browser::{BrowserBatchTool, BrowserTool};
use crate::computer_use::ComputerUseTool;
use crate::filesystem::{FsListTool, FsReadTool, FsWriteTool};
use crate::monitor::{
    MonitorListTool, MonitorRegistry, MonitorStartTool, MonitorStopTool, MONITOR_LIST_TOOL_ID,
    MONITOR_START_TOOL_ID, MONITOR_STOP_TOOL_ID,
};
use crate::patch::PatchApplyTool;
use crate::registry::{Tool, ToolDefinition, ToolProvider};
use crate::search::RipgrepSearchTool;
use crate::shell;
use crate::shell::ShellExecTool;
use agent_core::DomainEvent;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct BuiltinProvider {
    tools: HashMap<String, Arc<dyn Tool>>,
    monitor_registry: Arc<MonitorRegistry>,
}

#[derive(Clone)]
pub struct WorkspaceScopedBuiltinTools {
    monitor_registry: Arc<MonitorRegistry>,
}

impl WorkspaceScopedBuiltinTools {
    pub fn new(event_tx: tokio::sync::broadcast::Sender<DomainEvent>) -> Self {
        Self::with_monitor_registry(Arc::new(MonitorRegistry::new(PathBuf::from("."), event_tx)))
    }

    pub fn with_monitor_registry(monitor_registry: Arc<MonitorRegistry>) -> Self {
        Self { monitor_registry }
    }

    pub fn tool(&self, tool_id: &str, workspace_root: PathBuf) -> Option<Box<dyn Tool>> {
        if let Some(tool) = workspace_scoped_stateless_builtin_tool(tool_id, workspace_root.clone())
        {
            return Some(tool);
        }

        match tool_id {
            MONITOR_START_TOOL_ID => Some(Box::new(MonitorStartTool::for_workspace(
                self.monitor_registry.clone(),
                workspace_root,
            ))),
            MONITOR_STOP_TOOL_ID => Some(Box::new(MonitorStopTool::new(
                self.monitor_registry.clone(),
            ))),
            MONITOR_LIST_TOOL_ID => Some(Box::new(MonitorListTool::new(
                self.monitor_registry.clone(),
            ))),
            _ => None,
        }
    }

    pub async fn stop_all_monitors(&self) {
        self.monitor_registry.stop_all().await;
    }
}

impl BuiltinProvider {
    pub fn with_defaults(workspace_root: PathBuf) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(64);
        Self::with_defaults_and_event_tx(workspace_root, event_tx)
    }

    pub fn with_defaults_and_event_tx(
        workspace_root: PathBuf,
        event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    ) -> Self {
        let monitor_registry = Arc::new(MonitorRegistry::new(workspace_root.clone(), event_tx));
        Self::with_defaults_and_monitor_registry(workspace_root, monitor_registry)
    }

    pub fn with_defaults_and_monitor_registry(
        workspace_root: PathBuf,
        monitor_registry: Arc<MonitorRegistry>,
    ) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        let shell = Box::new(ShellExecTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let search = Box::new(RipgrepSearchTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let patch = Box::new(PatchApplyTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let fs_read = Box::new(FsReadTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let fs_write = Box::new(FsWriteTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let fs_list = Box::new(FsListTool::new(workspace_root.clone())) as Box<dyn Tool>;

        let browser_tool = BrowserTool::new(workspace_root.clone());
        let browser_batch =
            Box::new(BrowserBatchTool::new(browser_tool.manager())) as Box<dyn Tool>;
        let browser = Box::new(browser_tool) as Box<dyn Tool>;

        let computer_use = Box::new(ComputerUseTool::new()) as Box<dyn Tool>;

        let mon_start = Box::new(MonitorStartTool::new(monitor_registry.clone())) as Box<dyn Tool>;
        let mon_stop = Box::new(MonitorStopTool::new(monitor_registry.clone())) as Box<dyn Tool>;
        let mon_list = Box::new(MonitorListTool::new(monitor_registry.clone())) as Box<dyn Tool>;

        tools.insert(shell.definition().tool_id.clone(), Arc::from(shell));
        tools.insert(browser.definition().tool_id.clone(), Arc::from(browser));
        tools.insert(
            browser_batch.definition().tool_id.clone(),
            Arc::from(browser_batch),
        );
        tools.insert(search.definition().tool_id.clone(), Arc::from(search));
        tools.insert(patch.definition().tool_id.clone(), Arc::from(patch));
        tools.insert(fs_read.definition().tool_id.clone(), Arc::from(fs_read));
        tools.insert(fs_write.definition().tool_id.clone(), Arc::from(fs_write));
        tools.insert(fs_list.definition().tool_id.clone(), Arc::from(fs_list));
        tools.insert(mon_start.definition().tool_id.clone(), Arc::from(mon_start));
        tools.insert(mon_stop.definition().tool_id.clone(), Arc::from(mon_stop));
        tools.insert(mon_list.definition().tool_id.clone(), Arc::from(mon_list));
        tools.insert(
            computer_use.definition().tool_id.clone(),
            Arc::from(computer_use),
        );

        Self {
            tools,
            monitor_registry,
        }
    }

    pub fn monitor_registry(&self) -> &Arc<MonitorRegistry> {
        &self.monitor_registry
    }
}

pub fn workspace_scoped_builtin_tool(
    tool_id: &str,
    workspace_root: PathBuf,
) -> Option<Box<dyn Tool>> {
    workspace_scoped_stateless_builtin_tool(tool_id, workspace_root)
}

fn workspace_scoped_stateless_builtin_tool(
    tool_id: &str,
    workspace_root: PathBuf,
) -> Option<Box<dyn Tool>> {
    match tool_id {
        shell::SHELL_TOOL_ID => Some(Box::new(ShellExecTool::new(workspace_root))),
        shell::SEARCH_TOOL_ID => Some(Box::new(RipgrepSearchTool::new(workspace_root))),
        shell::PATCH_TOOL_ID => Some(Box::new(PatchApplyTool::new(workspace_root))),
        "fs.read" => Some(Box::new(FsReadTool::new(workspace_root))),
        "fs.write" => Some(Box::new(FsWriteTool::new(workspace_root))),
        "fs.list" => Some(Box::new(FsListTool::new(workspace_root))),
        _ => None,
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
#[path = "builtin_tests.rs"]
mod tests;
