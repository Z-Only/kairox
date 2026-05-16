# MCP / Skills / Models 配置重构设计规格

> 日期: 2026-05-14 | 状态: Draft

## 1. 概述

统一重构 MCP Server、Skills、Models 三者的安装/配置/列表/发现流程。核心目标：

- 统一三面板的 scope 展示与交互模式
- 补全项目级覆盖/禁用用户级配置的能力
- 确保 marketplace/discover 与已安装列表状态连通
- 规范化 skills 源管理（移除 skills.sh，保留 skillhub）
- 为所有 MCP server 增加连通性测试

## 2. 配置层级

```
优先级: Local > Project > User > Builtin

~/.kairox/config.toml            ← User (全局生效)
.kairox/config.toml              ← Project (可提交 git)
.kairox/config.local.toml        ← Local (gitignored, 新增)
编译嵌入                           ← Builtin (skills/mcp catalog/models registry)
```

### 2.1 合并规则

| 配置类型      | 合并策略        | 说明                                                                |
| ------------- | --------------- | ------------------------------------------------------------------- |
| MCP Server    | 同名整体覆盖    | 不 deep merge，项目定义完全替代用户定义                             |
| Model Profile | 同名 alias 覆盖 | 项目级 profile 完全替代用户级同名 alias                             |
| Skill         | 同名 shadow     | 高层级生效，低层级标记 `effective: false, shadowed_by: "workspace"` |

### 2.2 项目级对用户级的控制

| 操作            | 效果                                           | 列表呈现                            |
| --------------- | ---------------------------------------------- | ----------------------------------- |
| 覆盖 (Override) | 项目级定义同名项，完全替代用户级定义           | source=`project`，标注 `(覆盖用户)` |
| 禁用 (Disable)  | 项目级仅标记 `enabled = false`，不提供替代定义 | source=`user`，标注 `· 项目已禁用`  |

UI 入口：右键菜单 →「在项目中覆盖」「在项目中禁用」。覆盖时表单预填用户级值。

## 3. 生效配置合并视图

### 3.1 EffectiveItem 数据类型

```rust
struct EffectiveItem<T> {
    value: T,
    source: ConfigScope,              // builtin | user | project | local
    overrides: Option<ConfigScope>,    // 被覆盖的来源 (有冲突时)
    writable: bool,
    deletable: bool,
    enabled: bool,
    disabled_by: Option<ConfigScope>,  // 被哪个层级禁用
}

enum ConfigScope {
    Builtin,
    User,
    Project,
    Local,
}
```

### 3.2 列表视图

默认 Tab 显示合并后的全部生效项，标注 source：

```
MCP Servers (生效配置)

NAME          TRANSPORT  SOURCE               STATUS         TOOLS     ACTIONS
github        stdio      project (覆盖用户)     ● connected     12 tools  [测试] [禁用] [删除]
filesystem    stdio      user                 ○ timeout        0 tools   [测试] [禁用] [删除]
slack         sse        user · 项目已禁用     —               —         [启用]
git           stdio      builtin · ⚠ 未验证    ✕ unavailable   —         [测试连通性]
code-review   —          builtin              ✓ enabled       —         [禁用]
```

可选侧 Tab：`[用户配置]` / `[项目配置]` 切换独立视图用于精确编辑。

### 3.3 三面板统一

| 面板           | 当前 Scope 标签 | 当前 Source 列        | 目标    |
| -------------- | --------------- | --------------------- | ------- |
| MCP Settings   | ❌              | ❌ (字段存在但未渲染) | ✅ 统一 |
| Skill Settings | ✅              | ✅                    | ✅ 保持 |
| Model Settings | ❌              | ❌ (字段存在但未渲染) | ✅ 统一 |

所有列表统一：scope 标签列 + source 元数据 + writable/deletable 控制。

## 4. Tauri Commands 重构

### 4.1 查询命令

```rust
// 各 scope 独立查询
list_mcp_servers(scope: Option<ConfigScope>) -> Vec<McpServerView>
list_skills(scope: Option<ConfigScope>) -> Vec<SkillView>
list_model_profiles(scope: Option<ConfigScope>) -> Vec<ModelProfileView>

// 生效配置 (合并视图)
get_effective_mcp_servers() -> Vec<EffectiveItem<McpServerView>>
get_effective_skills() -> Vec<EffectiveItem<SkillView>>
get_effective_model_profiles() -> Vec<EffectiveItem<ModelProfileView>>
```

### 4.2 安装/添加命令

```rust
install_mcp_server(spec: InstallSpec, target: ConfigScope) -> Result<()>
install_skill(spec: SkillInstallSpec, target: ConfigScope) -> Result<()>
add_model_profile(profile: ProfileDef, target: ConfigScope) -> Result<()>
```

### 4.3 控制命令

```rust
// 项目级禁用/覆盖用户级项
disable_at_scope(id: String, kind: ItemKind, scope: ConfigScope) -> Result<()>
override_at_scope(id: String, kind: ItemKind, spec: OverrideSpec, scope: ConfigScope) -> Result<()>

// 连通性测试
test_mcp_connectivity(server_id: String) -> ConnectivityResult
test_all_mcp_connectivity() -> Vec<(String, ConnectivityResult)>
```

## 5. 安装 Target Scope

所有安装/添加操作都必须有 scope 选择器：

```
添加 MCP Server
┌──────────────────────────────────────┐
│ 安装到:  ○ 用户(全局)  ● 项目        │
│         └─ 仅在当前项目生效           │
│                                      │
│ 本地覆盖
│         └─ 个人临时配置，不提交 git   │
│                                      │
│ 名称: [________________]              │
│ 传输:  stdio ▾                       │
│ ...                                  │
└──────────────────────────────────────┘
```

## 6. Marketplace / Discover 已安装检测

### 6.1 状态判断

安装前查询生效配置，检查同名同类型项是否已存在：

| 检测结果          | 主操作              | 次级信息                         |
| ----------------- | ------------------- | -------------------------------- |
| 未安装            | `[安装]`            | —                                |
| 同 scope 已安装   | `[已安装 ✓]` (禁用) | `[重新安装]` (更新/修复)         |
| 不同 scope 已安装 | `[安装到当前项目]`  | 「用户级已安装，当前项目未安装」 |
| 有更新版本        | `[更新到 vX.Y]`     | 显示已安装版本 vs 最新版本       |

### 6.2 覆盖确认

安装到项目级但用户级已存在同名项时：

```
┌───────────────────────────────────────────┐
│  ⚠ "github" 已在用户配置中安装             │
│                                           │
│  项目级安装将覆盖用户级配置。                │
│  用户级配置不会被删除，离开此项目后恢复。     │
│                                           │
│  用户级: command=uvx, args=[...]           │
│  项目级: command=npx, args=[...]           │
│                                           │
│  [取消]  [查看差异]  [确认覆盖]             │
└───────────────────────────────────────────┘
```

## 7. 内置 (Built-in) 项管理

### 7.1 各类内置项策略

| 类型                              | 显示在列表    | 可禁用 | 可删除 | 备注                                                         |
| --------------------------------- | ------------- | ------ | ------ | ------------------------------------------------------------ |
| 内置 Tools (shell, fs, search 等) | ❌            | —      | —      | 独立于 MCP，不在 MCP 列表展示                                |
| 内置 Skills                       | ✅            | ✅     | ❌     | `scope = builtin`，`deletable = false`                       |
| 内置 MCP Catalog                  | ✅ (安装后)   | ✅     | ❌     | catalog 条目不会自动安装；未安装不出现在「已安装」列表       |
| 内置 Model Registry               | ❌ (参考数据) | —      | —      | 在 model picker 中作为已知模型选项展示，但不在「已安装」列表 |

### 7.2 内置 MCP 可用性

`builtin-catalog.json` 条目增加 `verified: bool` 字段。内置且未经验证的 server 在列表中显示 `⚠ 未验证` 标记。首次启动自动对内置 server 跑连通性检测，不可用项默认禁用。

## 8. 连通性测试

### 8.1 测试定义

- 启动 MCP server → 调用 `tools/list` → 收到非空工具列表 = 成功
- stdio 超时: 15s；SSE 超时: 5s
- 结果缓存于 store 内存，不持久化。Connected 状态 5min 过期

### 8.2 测试入口

| 位置                  | 触发方式                                   |
| --------------------- | ------------------------------------------ |
| 已安装列表 (每行)     | 操作按钮 `[测试连通性]`，状态图标实时反映  |
| 手动添加表单底部      | `[测试连通性]` 按钮，先保存草稿再测试      |
| Catalog 详情 (安装前) | `[测试连通性]` 按钮，需先填写 env vars     |
| 全局                  | 「检测全部连通性」按钮，逐个测试，汇总结果 |

### 8.3 ConnectivityState

```rust
enum ConnectivityState {
    Unknown,
    Checking,
    Connected { tool_count: u32 },
    Failed { reason: String, at: DateTime },
}
```

## 9. Skills 源管理

### 9.1 源变更

- **移除** `skills.sh`（纯索引，无下载链接）
- **保留** `skillhub`（有真实 zip 下载链接）
- **保留** `github`（GitHub repo 安装）
- **保留** `builtin`（编译嵌入）

### 9.2 skillhub 安装流程

```
CatalogDetail / SkillDiscover 选中 skill
  → 获取下载 URL:
    https://skillhub-xxx.cos.accelerate.myqcloud.com/skills/<name>/<version>.zip
  → 下载到临时目录
  → 解压 → 验证 SKILL.md 存在
  → 移动到目标目录 (~/.kairox/skills/<name>/ 或 .kairox/skills/<name>/)
  → 写入 skills-state.toml (source, version, download_url, installed_at)
```

### 9.3 多源合并展示

同一 skill 存在于多个源时，合并为单卡片：

```
self-improving-agent
Automated self-improvement workflows

可用源:
● skillhub  v3.0.21  [安装] ← 推荐(最新)
○ github    v3.0.20  [安装]
```

默认推荐版本最高的源。

### 9.4 更新策略

- 更新检查仅查询**安装源**（`install_source` 字段），不跨源
- 安装源不可达 → `update_available = "unknown"`，提示可切换源
- 手动「切换源」操作：卸载当前 → 从新源安装，需用户确认

### 9.5 skills-state.toml 字段扩展

```toml
[skills."self-improving-agent"]
enabled = true
activation_mode = "suggest"
install_source = "skillhub"
install_url = "https://skillhub-xxx.cos.accelerate.myqcloud.com/skills/self-improving-agent/3.0.21.zip"
version = "3.0.21"
installed_at = "2026-05-14T10:30:00Z"
last_update_check = "2026-05-14T10:30:00Z"
update_available = false
# 新增字段
available_sources = ["skillhub", "github"]  # 安装时记录的其他可用源
```

## 10. 其他改进

### 10.1 `.mcp.json` 导入

- 检测项目根目录 `.mcp.json` (Claude Code 格式)
- 提示用户「发现 Claude Code MCP 配置，是否导入？」
- 格式转换 JSON → TOML，写入当前 scope 的 `kairox.toml`
- 单向导入，不自动同步

### 10.2 统一 MCP 配置源

当前 MCP server 定义分散在 `kairox.toml` 的 `[mcp_servers]` 段和独立 `mcp_servers.toml` 文件。

**方案**：统一到 `kairox.toml` 的 `[mcp_servers]` 段。移除 `mcp_servers.toml` 独立文件（仅保留读取兼容以迁移旧数据）。

### 10.3 生效配置查看器

新增面板：「生效配置」→ 展开可查看每项每个字段的来源：

```
github (stdio)
  command: npx                     ← project (覆盖用户)
  args: [-y, @modelcontextprotocol/server-github]  ← project
  env: {GITHUB_TOKEN: "***"}       ← local (覆盖项目)
  enabled: true                    ← user
```

### 10.4 Profile 导出/导入

- 导出：将当前 MCP + Skills + Models 配置导出为命名 profile TOML 片段
- 导入：从 profile 导入配置到当前 scope
- 用途：团队共享项目配置模板

## 11. 实施优先级

| 优先级 | 改动                                        | 影响范围                                 |
| ------ | ------------------------------------------- | ---------------------------------------- |
| P0     | EffectiveItem 类型 + 合并视图 + Source 标注 | agent-core, agent-config, 三面板         |
| P0     | Marketplace/Discover 已安装检测             | catalog store, marketplace/Discover 组件 |
| P0     | MCP 连通性测试 (列表 + 添加卡片)            | agent-mcp, MCP store, McpSettingsPane    |
| P1     | 项目级覆盖/禁用用户级配置 (后端 + UI)       | agent-config, 三面板                     |
| P1     | 安装/添加 target scope 选择器               | 所有安装/添加表单                        |
| P1     | skillhub zip 下载解压 + skills.sh 移除      | agent-skills, catalog                    |
| P1     | 多源 skill 合并展示                         | Discover/Catalog 组件                    |
| P1     | 内置 MCP 可用性标记与首次检测               | agent-mcp, builtin-catalog.json          |
| P2     | Local scope (第三级 gitignored 配置)        | agent-config discovery/merge             |
| P2     | 生效配置查看器                              | 新面板                                   |
| P2     | 统一 MCP 配置源                             | agent-config loader/writer               |
| P2     | 安装源不可达降级                            | agent-skills state                       |
| P3     | `.mcp.json` 导入                            | 独立导入工具                             |
| P3     | Profile 导出/导入                           | 新功能                                   |

## 12. 不变更项

- 内置 Tools (shell, fs.read, fs.write, fs.list, patch, search) 保持独立于 MCP，不出现在 MCP 列表
- 内置 Model Registry 保持为参考数据，不在「已安装」列表显示
- Skill SKILL.md 格式不变
- MCP 协议不变
- 现有 test 覆盖不受影响
