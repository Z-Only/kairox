# 权限、沙箱与审批策略解耦设计

> 日期: 2026-05-26
> 状态: 已批准实施(Plan A,分两步硬切换)
> 关联代码: `crates/agent-tools/src/permission.rs`(本 PR 标 deprecated),`crates/agent-tools/src/policy/`(新增)

## 0 · 分阶段说明

实施期间发现旧 `PermissionMode` 的调用点(主要是测试)分布在 100+ 文件,单 PR 硬切风险过高。采用分两步硬切换:

- **本 PR (PR-1)**: 新增 `crates/agent-tools/src/policy/` 模块、`PolicyEngine`(以 `ApprovalPolicy + SandboxPolicy` 为输入)、新 facade 方法、新 IPC 命令、GUI 双选择器。`PermissionMode` 标 `#[deprecated]`,通过 `From<PermissionMode> for (ApprovalPolicy, SandboxPolicy)` shim 把所有旧路径桥到新引擎。旧 facade/IPC/GUI 老选择器暂留以保证调用点编译通过。
- **PR-2**(下个 PR): 删除 `PermissionMode` 与旧 facade/IPC/GUI 老路径,机械替换 80+ 测试调用点,提示常量 `PERMISSION_MODE_MIGRATION_NOTE` 用于编译错误指引。

PR-1 已经达成"完善"目标:决策矩阵驱动者已是 `(ApprovalPolicy, SandboxPolicy)` 双正交,用户可在 GUI 独立设置。PR-2 是机械清理。

## 1 · 背景与目标

当前 `PermissionMode`(`ReadOnly` / `Suggest` / `Agent` / `Autonomous` / `Interactive`)把"沙箱能允许什么"与"什么情况下要弹审批"两件事揉在同一枚举里。参考 Codex(ApprovalPolicy × SandboxPolicy 双正交)与 Claude Code(Default/AcceptEdits/Plan/Bypass + allow/deny)后,本期目标:

- **解耦审批策略与沙箱策略**,得到两个正交维度;
- **硬切换**,删除 `PermissionMode`,通过编译期错误 + 迁移表强制升级;
- 不在本期引入 OS 级沙箱(Seatbelt / Landlock)、不引入配置文件驱动的细粒度规则、不持久化 MCP 信任列表(后续设计);
- 保持现有事件名(`PermissionRequested` / `Granted` / `Denied`)以兼容事件存储。

## 2 · 核心类型

### 2.1 `ApprovalPolicy`(`crates/agent-tools/src/policy/approval.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    Never,      // 不弹审批,沙箱拒绝即拒绝
    OnRequest,  // 升权操作弹审批
    Always,     // 全部弹审批(仅 Read 静默)
}

impl Default for ApprovalPolicy {
    fn default() -> Self { Self::OnRequest }
}
```

### 2.2 `SandboxPolicy`(`crates/agent-tools/src/policy/sandbox.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SandboxPolicy {
    ReadOnly,
    WorkspaceWrite {
        #[serde(default)] network_access: bool,
        #[serde(default)] writable_roots: Vec<PathBuf>,
    },
    DangerFullAccess,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self::WorkspaceWrite { network_access: false, writable_roots: vec![] }
    }
}
```

### 2.3 `ToolEffect` / `ToolRisk`(`crates/agent-tools/src/policy/effect.rs`)

不变,从 `permission.rs` 平移。

### 2.4 `PolicyDecision`(`crates/agent-tools/src/policy/decision.rs`)

```rust
pub enum PolicyDecision {
    Allowed,
    DeniedBySandbox { reason: String },
    NeedsApproval { reason: ApprovalReason },
}

pub enum ApprovalReason {
    SandboxRejected,
    PolicyAlways,
    DestructiveEffect,
    UnknownCommand,
    NetworkRequest,
    UntrustedMcpServer,
}
```

## 3 · 决策矩阵

`PolicyEngine::decide(risk: &ToolRisk) -> PolicyDecision`:

1. **MCP 路径**: 若 `ToolEffect::McpInvoke { server }`,信任列表命中 → `Allowed`;未命中 + `Never` → `DeniedBySandbox`;否则 → `NeedsApproval { UntrustedMcpServer }`。
2. **沙箱预判** `sandbox_check`,得到 `SandboxVerdict::{Ok, Reject(reason), NeedsUpgrade(reason)}`。
3. **审批叠加**:

| `(Approval, Sandbox)`           | `Read`    | `Write/Shell{!dest}` | `Destructive / Shell{dest} / Network`           |
| ------------------------------- | --------- | -------------------- | ----------------------------------------------- |
| `(Never, ReadOnly)`             | `Allowed` | `DeniedBySandbox`    | `DeniedBySandbox`                               |
| `(Never, WorkspaceWrite)`       | `Allowed` | `Allowed`\*          | `DeniedBySandbox`(沙箱要升权,Never 拒)          |
| `(Never, DangerFullAccess)`     | `Allowed` | `Allowed`            | `Allowed`                                       |
| `(OnRequest, ReadOnly)`         | `Allowed` | `DeniedBySandbox`    | `DeniedBySandbox`                               |
| `(OnRequest, WorkspaceWrite)`   | `Allowed` | `Allowed`\*          | `NeedsApproval`                                 |
| `(OnRequest, DangerFullAccess)` | `Allowed` | `Allowed`            | `NeedsApproval { DestructiveEffect }`(危险仍审) |
| `(Always, ReadOnly)`            | `Allowed` | `DeniedBySandbox`    | `DeniedBySandbox`                               |
| `(Always, WorkspaceWrite)`      | `Allowed` | `NeedsApproval`      | `NeedsApproval`                                 |
| `(Always, DangerFullAccess)`    | `Allowed` | `NeedsApproval`      | `NeedsApproval`                                 |

> \*`Write` 路径还需 `SandboxPolicy::path_writable()` 校验;越界路径升级为 `NeedsApproval`(WorkspaceWrite)或 `DeniedBySandbox`(ReadOnly)。
> `Network` 在 `WorkspaceWrite { network_access: false }` 下视作 `NeedsUpgrade(NetworkRequest)`。

## 4 · Runtime 流程

### 4.1 `agent-runtime/src/policy.rs`(替换 `permission.rs`)

```rust
pub async fn check_tool_policy(
    engine: &Arc<RwLock<PolicyEngine>>,
    pending: &PendingPermissionsMap,
    bus: &EventBus,
    session_id: SessionId,
    risk: ToolRisk,
) -> Result<(), PolicyError>;
```

事件 `DomainEvent::PermissionRequested/Granted/Denied` 名称保留,payload 字段:

- 新增 `approval_reason: String`(`ApprovalReason` 序列化值)
- 新增 `sandbox_policy_kind: String`
- 旧 `mode` 字段删除(硬切换)

### 4.2 `LocalRuntime` facade

新方法:

- `with_approval_policy(p)` / `with_sandbox_policy(p)`(builder)
- `approval_policy()` / `sandbox_policy()`(getter)
- `set_approval_policy(p)` / `set_sandbox_policy(p)`(全局)
- `set_session_approval(sid, p)` / `set_session_sandbox(sid, p)`(会话级)

删除:`with_permission_mode` / `permission_mode` / `set_permission_mode` / `set_session_permission_mode`。

### 4.3 Agent override

```rust
pub struct AgentDef {
    pub approval_policy: Option<ApprovalPolicy>,
    pub sandbox_policy: Option<SandboxPolicy>,
    // ...其余不变
}
```

优先级: Agent override → Session override → Runtime default。

## 5 · 配置与持久化

### 5.1 `agent-config`

```toml
[policy]
approval = "on_request"

[policy.sandbox]
kind = "workspace_write"
network_access = false
writable_roots = []
```

### 5.2 SessionRow 迁移

```sql
ALTER TABLE sessions ADD COLUMN approval_policy TEXT;
ALTER TABLE sessions ADD COLUMN sandbox_policy TEXT;

UPDATE sessions SET
    approval_policy = CASE permission_mode
        WHEN 'read_only'   THEN 'never'
        WHEN 'suggest'     THEN 'always'
        WHEN 'agent'       THEN 'on_request'
        WHEN 'autonomous'  THEN 'never'
        WHEN 'interactive' THEN 'on_request'
        ELSE 'on_request'
    END,
    sandbox_policy = CASE permission_mode
        WHEN 'read_only'   THEN '{"kind":"read_only"}'
        WHEN 'suggest'     THEN '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
        WHEN 'agent'       THEN '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
        WHEN 'autonomous'  THEN '{"kind":"danger_full_access"}'
        WHEN 'interactive' THEN '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
        ELSE '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    END
WHERE permission_mode IS NOT NULL;
```

旧 `permission_mode` 列保留一个版本周期,下个 spec 触发 DROP。

### 5.3 硬切换提示

```rust
pub const PERMISSION_MODE_MIGRATION_NOTE: &str = "PermissionMode 已删除...";
```

## 6 · GUI

### 6.1 IPC 命令(Specta 重生)

新增:

- `get_session_approval_policy(session_id) -> ApprovalPolicy`
- `set_session_approval_policy(session_id, policy)`
- `get_session_sandbox_policy(session_id) -> SandboxPolicy`
- `set_session_sandbox_policy(session_id, policy)`
- `trust_mcp_server(server)` / `untrust_mcp_server(server)`(本期仍内存)

删除:`get_session_permission_mode` / `set_session_permission_mode`。

### 6.2 组件拆分

`ChatPermissionSelector.vue` → 删除,改两个并列组件:

- `ChatApprovalSelector.vue`(3 项)
- `ChatSandboxSelector.vue`(3 项 + 二级 `network_access` toggle)

### 6.3 i18n

新增 keys `chat.approval`, `chat.approvals.*`, `chat.sandbox`, `chat.sandboxes.*`, `chat.sandboxNetwork`,删除 `chat.permission` / `chat.permissions.*`。

### 6.4 Playwright mock + tauri-pilot

`apps/agent-gui/e2e/tauri-mock.js` 同步新 IPC handler。完成后用 `tauri-pilot` 实测 3 个场景:Always+WorkspaceWrite 弹审、Never+ReadOnly 拒、信任 MCP server 静默放行。

## 7 · 错误处理

| 场景                             | 行为                                             |
| -------------------------------- | ------------------------------------------------ |
| `ApprovalPolicy::parse("foo")`   | `PolicyParseError::UnknownApproval` + 列出合法值 |
| `SandboxPolicy::parse` JSON 损坏 | `ConfigError::InvalidSandboxPolicy` 包裹         |
| WorkspaceWrite 路径越界          | `PolicyDecision::DeniedBySandbox` + path 列表    |
| 审批超时 30s                     | `PolicyError::ApprovalTimeout`                   |
| 用户 Deny                        | `PolicyError::ApprovalDenied { reason }`         |
| 旧 DB 行 `permission_mode` 残留  | 启动迁移;新列 NULL → 走 `PolicyConfig` 默认      |
| 旧代码引用 `PermissionMode`      | 编译错误 + `PERMISSION_MODE_MIGRATION_NOTE`      |

## 8 · 测试

### 8.1 单测

- `ApprovalPolicy` / `SandboxPolicy` parse/display/serde round-trip
- `SandboxPolicy::path_writable` 边界
- `PolicyEngine::decide` 全决策矩阵 3×3×6=54 case 表驱动
- MCP 信任路径 3 case

### 8.2 集成

- `check_tool_policy` Happy/Denied/NeedsApproval
- 会话/Agent override 合并优先级
- `set_session_approval` / `set_session_sandbox` 持久化(in-memory SQLite)
- 事件 payload 新字段断言

### 8.3 Store migration

- `migrate_v?_add_policy_columns` upgrade
- 旧 `permission_mode` 5 个值各跑一遍,新列正确填充

### 8.4 GUI

- Vitest: 两个新组件 + `useSessionStore` 双 setter
- Playwright E2E: 切策略 → 触发 mock 工具 → 等审批事件 → 接受/拒绝
- **tauri-pilot**: 第 6.4 节 3 个场景

## 9 · 风险与回滚

- 风险: 旧 `permission_mode` 序列化在 DB 落了 5 种值,migration SQL 必须覆盖全部 5 种 + NULL → 默认。
- 回滚: 单 PR 实施,失败则 revert commit。旧列保留可在 revert 后继续读。
- 验证: `tauri-pilot` 实际交互 + Playwright E2E + Rust 矩阵测试。
