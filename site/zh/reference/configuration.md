---
title: 配置
description: "`kairox.toml` 的发现顺序、profile schema、MCP server schema、context 预算管理,以及完整示例。"
outline: [2, 3]
---

# 配置

Kairox 从多层 TOML 文件读取配置:用户级配置、workspace 配置,以及可选的本地覆盖文件。这个格式在 TUI 和 GUI 之间完全共享,同一组字段两边都会读取。本页是所有受支持字段的参考手册。

示例的权威来源是仓库根目录下的 [`kairox.toml.example`](https://github.com/Z-Only/kairox/blob/main/kairox.toml.example)。本页负责说明每个字段的含义、何时生效,以及省略时会发生什么。

## 发现顺序

runtime 启动时会先加载内置默认值,再按以下顺序叠加更高优先级的文件:

1. **内置默认值。** Kairox 提供用于离线测试的 `fake` provider、默认禁用且指向 Ollama 的 `local-code` profile,以及当环境中存在 `OPENAI_API_KEY` 时自动启用、指向 OpenAI 的 `fast` profile。
2. **用户级配置。** `~/.kairox/config.toml`,作为按用户的配置层。这里适合放个人 API key 和个人 profile 偏好。
3. **项目级配置。** `./.kairox/config.toml`,从当前工作目录起最多向上回溯 5 层父目录。这是 workspace 级别的文件,可以提交到仓库以共享团队约定。
4. **本地项目覆盖。** `./.kairox/config.local.toml`,从 project root 发现,用于保持 gitignore 的机器本地覆盖项。

更高层会覆盖或扩展更低层。Profile 用同名 alias 替换时保留原顺序,新增 alias 追加在后面。MCP server、knowledge base、hook、LSP server 和 DAP server 按 id 替换。`disabled_mcp_servers` 是累加并集。`instructions` 会用空行拼接。`[context]`、`[features]` 和 `[advisor]` 使用设置了它们的最高层;当 overlay 的 advisor 仍是默认值时会继承下层。

::: tip 项目根目录 vs. workspace 根目录
所谓"项目级配置",是指从进程当前工作目录开始向上查找 `.kairox/config.toml`。在 TUI 中,这就是你执行 `kairox` 命令所在的目录;在 GUI 中,则是创建 session 时选定的 workspace 根目录。五层父目录的向上回溯意味着你可以 `cd` 进任意子目录,依然能找到 workspace 级配置。`config.local.toml` 会作为该 project config 之上的本地覆盖层。
:::

## Profile

profile 是为某个模型命名的一组配置。session 通过名称选择 profile;profile 决定使用哪个 provider 客户端、传入什么 model ID,以及从哪个环境变量里取 API key。

### Profile schema

| 字段                         | 类型   | 必填 | 默认值          | 说明                                                                                         |
| ---------------------------- | ------ | ---- | --------------- | -------------------------------------------------------------------------------------------- |
| `provider`                   | string | 是   | —               | 任意 provider 名称。已知值:`anthropic`、`ollama`、`fake`。其它一律走 OpenAI 兼容客户端。     |
| `model_id`                   | string | 是   | —               | 发送给 API 的模型标识符(例如 `gpt-4.1`、`claude-sonnet-4-20250514`)。                        |
| `enabled`                    | bool   | 否   | `true`          | 禁用的 profile 会被解析,但不会注册进 router,也不会作为可选 profile 展示。                    |
| `base_url`                   | string | 否   | provider 默认值 | API 的 base URL。`anthropic` 可省略以使用官方端点。                                          |
| `connect_timeout_secs`       | int    | 否   | client 默认值   | 支持该选项的客户端使用的 HTTP 连接超时。                                                     |
| `request_timeout_secs`       | int    | 否   | client 默认值   | 整体 HTTP 请求超时。流式客户端通常应保持未设置。                                             |
| `api_key`                    | string | 否   | —               | 直接写明 API key,优先级高于 `api_key_env`。请不要写入提交的文件中。                          |
| `api_key_env`                | string | 否   | —               | 存放 API key 的环境变量名,运行时解析。                                                       |
| `context_window`             | int    | 否   | 来自模型元数据  | 输入加历史的最大 token 数。按三层兜底查找:profile → `ModelRegistry` → provider 默认值。      |
| `output_limit`               | int    | 否   | 来自模型元数据  | 输出的最大 token 数,兜底逻辑同 `context_window`。                                            |
| `max_tokens`                 | int    | 否   | `output_limit`  | 单次响应的上限。Anthropic 会显式用它来设置 `max_tokens` 参数。                               |
| `temperature`                | float  | 否   | provider 默认值 | 采样温度,0.0–2.0。                                                                           |
| `top_p`                      | float  | 否   | provider 默认值 | nucleus sampling,0.0–1.0。                                                                   |
| `top_k`                      | int    | 否   | provider 默认值 | top-k sampling,仅 Anthropic 支持。                                                           |
| `headers`                    | table  | 否   | —               | 附加到每次请求的 HTTP header,常用于企业网关。                                                |
| `client_identity`            | string | 否   | —               | `claude_code` 会添加 Claude Code client header,供按客户端身份分流的 Anthropic 兼容网关使用。 |
| `supports_tools`             | bool   | 否   | 自动探测        | 覆盖自动探测出的 tool calling 能力。                                                         |
| `supports_vision`            | bool   | 否   | 自动探测        | 覆盖自动探测出的视觉能力。                                                                   |
| `supports_reasoning`         | bool   | 否   | 自动探测        | 覆盖自动探测出的推理能力。                                                                   |
| `server_tool_code_execution` | bool   | 否   | `false`         | 启用 Anthropic server-side code execution(`code_execution_20250825`)并添加所需 beta header。 |
| `server_tool_web_search`     | bool   | 否   | `false`         | 启用 Anthropic server-side web search(`web_search_20250305`)。                               |
| `extra_params`               | table  | 否   | —               | 原样透传给 provider 的特定参数(例如 Anthropic 的 `thinking`)。                               |
| `response`                   | string | 否   | —               | 静态响应文本,仅 `fake` provider 使用。                                                       |

### Provider 自动识别

runtime 将 `provider` 映射到具体的客户端实现:

| `provider` / `base_url` 匹配项                                   | 客户端                                            |
| ---------------------------------------------------------------- | ------------------------------------------------- |
| `provider = "anthropic"`                                         | Anthropic SDK,走 `messages` 端点                  |
| `provider = "ollama"`                                            | Ollama HTTP 客户端(默认 `http://localhost:11434`) |
| `provider = "fake"`                                              | 固定返回配置中 `response` 的桩客户端              |
| `provider = "openai_compatible"`                                 | OpenAI Chat Completions 客户端(显式名称)          |
| 自定义 provider 包含 `anthropic`,或 `base_url` 包含 `/anthropic` | Anthropic 兼容客户端                              |
| 其它任意值                                                       | OpenAI 兼容客户端(Groq、xAI、DeepSeek 等)         |

你不必把新 provider 假装成 `openai_compatible`,直接写 `provider = "deepseek"` 即可。runtime 会把未知且不匹配 Anthropic 的 provider 按 OpenAI 兼容处理。

### 完整示例

DeepSeek(自动识别为 OpenAI 兼容):

```toml
[profiles.deepseek]
provider = "deepseek"
model_id = "deepseek-chat"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"
```

显式指定 context 与输出上限的 OpenAI:

```toml
[profiles.gpt4]
provider = "openai_compatible"
model_id = "gpt-4.1"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
context_window = 1_047_576
output_limit = 32_768
```

启用 extended thinking 的 Anthropic Claude:

```toml
[profiles.claude-thinking]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
temperature = 1.0
max_tokens = 32_768

[profiles.claude-thinking.extra_params]
thinking = { type = "enabled", budget_tokens = 16_000 }
```

`extra_params` 会原样透传给 provider。对 Anthropic 来说,extended thinking、beta 特性以及未来新增的参数都通过这种方式触达 API,不需要 Kairox 发布新版本。

本地 Ollama:

```toml
[profiles.local]
provider = "ollama"
model_id = "devstral"
base_url = "http://localhost:11434"
```

Fake provider(用于离线测试或对确定性输出做脚本化处理):

```toml
[profiles.fake]
provider = "fake"
model_id = "fake"
response = "Hello from the Kairox fake provider!"
```

自定义 header(企业网关):

```toml
[profiles.enterprise]
provider = "openai_compatible"
model_id = "enterprise-model"
base_url = "https://internal-gateway.example.com/v1"
api_key_env = "ENTERPRISE_KEY"

[profiles.enterprise.headers]
X-Organization = "my-org"
X-Project = "kairox"
```

覆盖能力探测(用于自动探测结果不准的 provider):

```toml
[profiles.custom-vision]
provider = "custom-provider"
model_id = "vision-model-v1"
base_url = "https://api.example.com/v1"
supports_tools = false
supports_vision = true
```

Anthropic server-side tools 与 Claude Code client identity:

```toml
[profiles.claude-tools]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
server_tool_code_execution = true
server_tool_web_search = true
client_identity = "claude_code"
```

`server_tool_code_execution` 会添加 Anthropic 的 `code-execution-2025-08-25` beta header,并发送 `code_execution_20250825` tool type。`server_tool_web_search` 会发送 `web_search_20250305` tool type。这些是 provider-hosted tools,不是本地 Kairox tool,所以本地 Approval × Sandbox 策略不会作用于它们在 provider 内部的执行。

带显式超时的 Anthropic 兼容网关:

```toml
[profiles.enterprise-claude]
provider = "enterprise-anthropic"
model_id = "claude-sonnet-4-20250514"
base_url = "https://gateway.example.com/anthropic"
api_key_env = "ENTERPRISE_ANTHROPIC_KEY"
connect_timeout_secs = 10
request_timeout_secs = 120
```

### Anthropic 的 key 解析

当 `provider = "anthropic"` 且 `api_key` 和 `api_key_env` 都未设置时,Anthropic 客户端会回退到 `~/.claude/settings.json` 并读取 `ANTHROPIC_AUTH_TOKEN`。对于在同一台机器上已经登录过 Claude Code 的用户来说,这非常方便 —— 不需要任何额外配置。

### Context window 与输出上限的解析

`context_window` 和 `output_limit` 都遵循三层兜底:

1. **profile 中的值**(如果你设置了)。
2. **`ModelRegistry`** 中按 `(provider, model_id)` 查表,内置常见模型的精选值。
3. **provider 默认值**,当 registry 中没有对应条目时使用。

正因如此,常见模型即便两个字段都不写,预算估算也能正确工作。只有当你跑的是非主流模型、forked 端点,或者想保守一些时,才需要显式设置。

## MCP server

`[mcp_servers.<id>]` 用于声明一个 Model Context Protocol server。每个 server 都有唯一 id —— 也就是 section 名 —— runtime、marketplace 和 GUI 都通过这个 id 引用它。

### 通用字段

| 字段                   | 类型   | 默认值  | 说明                                                            |
| ---------------------- | ------ | ------- | --------------------------------------------------------------- |
| `type`                 | string | —       | `"stdio"`、`"sse"` 或 `"streamable_http"`,必填。                |
| `keep_alive`           | bool   | `false` | 为 true 时,即便没有 session 在用,server 也会保持运行。          |
| `idle_timeout_secs`    | int    | `300`   | server 空闲多少秒后被停止;`keep_alive` 为 true 时忽略。         |
| `auto_restart`         | bool   | `true`  | transport 失败时是否自动重启。                                  |
| `max_restart_attempts` | int    | `3`     | manager 放弃前的最大重启尝试次数,超过后发出 `McpServerFailed`。 |

### stdio 专属字段

| 字段      | 类型   | 说明                                                  |
| --------- | ------ | ----------------------------------------------------- |
| `command` | string | 要执行的命令,必填。                                   |
| `args`    | array  | 命令行参数。                                          |
| `env`     | table  | 环境变量。值为空字符串 `""` 时,会从同名环境变量读取。 |
| `cwd`     | string | 子进程的工作目录,默认为 runtime 的 cwd。              |

示例 —— 启用 keep-alive 的本地 filesystem MCP server:

```toml
[mcp_servers.filesystem]
type = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/projects"]
keep_alive = true
```

示例 —— 需要 personal access token 的 GitHub server:

```toml
[mcp_servers.github]
type = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_PERSONAL_ACCESS_TOKEN = "" }  # 空值 = 读取同名环境变量
```

这种空字符串约定表示"server 启动时读取同名环境变量"。如果直接写成 `GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_xxx"`,token 就被硬编码进了配置;留空则交由运行时环境提供。

### SSE 专属字段

| 字段          | 类型   | 说明                                                                                |
| ------------- | ------ | ----------------------------------------------------------------------------------- |
| `url`         | string | SSE 端点,必填。                                                                     |
| `headers`     | table  | HTTP header。值中 `${VAR}` 形式的子串会在请求时从环境变量展开。                     |
| `api_key_env` | string | 设置后,从该环境变量读取值,并以 `Authorization: Bearer <value>` 形式加到 header 上。 |

示例 —— 使用 bearer 认证的远端 search server:

```toml
[mcp_servers.remote-search]
type = "sse"
url = "https://mcp.example.com/search"
headers = { Authorization = "Bearer ${MCP_API_TOKEN}" }
```

或者等价地写成:

```toml
[mcp_servers.remote-search]
type = "sse"
url = "https://mcp.example.com/search"
api_key_env = "MCP_API_TOKEN"
```

对 bearer 认证来说,这两种写法是等价的。希望由 runtime 帮你拼 header 时用 `api_key_env`;需要非 bearer 方案或不同 header 名时用 `headers`。

### Streamable HTTP 专属字段

`streamable_http` 使用 MCP Streamable HTTP transport,并接受和 `sse` 相同的 HTTP 字段:`url`、`headers` 与 `api_key_env`。

示例 —— 使用 Streamable HTTP 的远端 MCP endpoint:

```toml
[mcp_servers.remote-http]
type = "streamable_http"
url = "https://mcp.example.com/mcp"
api_key_env = "MCP_API_TOKEN"
```

示例 —— 长期运行的本地开发型 server:

```toml
[mcp_servers.persistent-remote]
type = "sse"
url = "http://localhost:8080/mcp"
keep_alive = true
```

### 生命周期与可观测性

server 的每次生命周期变化都会发出一个 event:

| Event               | 触发时机                                  |
| ------------------- | ----------------------------------------- |
| `McpServerStarting` | manager 启动 transport 并开始握手。       |
| `McpServerReady`    | 握手成功,tool 已注册完毕。                |
| `McpServerStopped`  | server 被停止(用户操作、空闲超时或关闭)。 |
| `McpServerFailed`   | 失败时附带诊断信息,重启计数器加一。       |

完整的 server 架构见 [Extensibility](../concepts/extensibility),MCP tool 如何在 agent loop 中出现见 [Runtime & Sessions](../concepts/runtime-and-sessions)。

## Knowledge base

`[knowledge_bases.<id>]` 声明一个可把外部文档加入 turn context 的 retriever。来源可以全局启用,也可以限定到指定 model profile alias。

```toml
[knowledge_bases.company-docs]
kind = "sqlite_fts"
path = ".kairox/kb/company.sqlite"
table = "kb_docs"
id_column = "doc_id"
title_column = "title"
content_column = "body"
workspace_id_column = "workspace_id"
profile_aliases = ["fast", "claude"]
max_results = 4
min_score = 0.25
```

| 字段                  | 类型     | 默认值       | 说明                                                                         |
| --------------------- | -------- | ------------ | ---------------------------------------------------------------------------- |
| `kind`                | string   | `sqlite_fts` | 支持值:`sqlite_fts`/`sqlite`、`tantivy`、`bedrock`、`pinecone`、`weaviate`。 |
| `enabled`             | bool     | `true`       | 不删除配置的情况下禁用该 source。                                            |
| `profile_aliases`     | string[] | `[]`         | 为空表示所有 profile 都可见;否则只有列出的 profile alias 能看到该 source。   |
| `path`                | string   | —            | 本地数据库或索引路径,供 SQLite FTS 等本地 connector 使用。                   |
| `endpoint`            | string   | —            | cloud/vector connector 的远端服务 endpoint。                                 |
| `api_key_env`         | string   | —            | 供已接入 runtime adapter 的 connector kind 使用的凭证环境变量。              |
| `region`              | string   | —            | cloud region,用于 Bedrock Knowledge Bases。                                  |
| `knowledge_base_id`   | string   | —            | Bedrock Knowledge Base 标识符。                                              |
| `index_name`          | string   | —            | Pinecone 等 connector 的 vector index 名称。                                 |
| `namespace`           | string   | —            | 可选 vector namespace。                                                      |
| `collection`          | string   | —            | Weaviate 等 connector 的 collection 名称。                                   |
| `table`               | string   | connector    | SQLite FTS table 名。                                                        |
| `id_column`           | string   | connector    | 文档 id 列。                                                                 |
| `title_column`        | string   | connector    | 文档 title 列。                                                              |
| `content_column`      | string   | connector    | 文档正文/content 列。                                                        |
| `workspace_id_column` | string   | connector    | 可选的 workspace 过滤列。                                                    |
| `max_results`         | int      | connector    | 每个 source 的结果数量上限。                                                 |
| `min_score`           | float    | connector    | 丢弃低于该 connector-specific 分数阈值的命中。                               |

SQLite FTS connector 当前已经接入 runtime context assembly。其它 `kind` 值已经进入配置模型,方便对应服务的 retriever 接入同一个 `WorkspaceRetriever` 边界。

## LSP 与 DAP server

`[lsp_servers.<id>]` 和 `[dap_servers.<id>]` 配置原生代码智能与调试 server。这些不是 MCP server;它们由 `agent-lsp` 管理,并通过 `agent-tools` 暴露为动态 tool。

```toml
[lsp_servers.rust-analyzer]
command = "rust-analyzer"
args = ["--stdio"]
languages = ["rust"]
file_patterns = ["*.rs"]
initialization_options = { check = { command = "clippy" } }
auto_start = false

[lsp_servers.rust-analyzer.env]
RA_LOG = "info"

[dap_servers.lldb]
command = "codelldb"
args = ["--port", "0"]
languages = ["rust"]

[dap_servers.lldb.env]
RUST_LOG = "debug"
```

| 字段                     | LSP | DAP | 默认值      | 说明                                       |
| ------------------------ | --- | --- | ----------- | ------------------------------------------ |
| `command`                | 是  | 是  | —           | server 可执行文件,必填。                   |
| `args`                   | 是  | 是  | `[]`        | 命令行参数。                               |
| `env`                    | 是  | 是  | `{}`        | 传给 server 进程的环境变量。               |
| `cwd`                    | 是  | 是  | runtime cwd | server 进程工作目录。                      |
| `languages`              | 是  | 是  | `[]`        | 关联到该 server 的 language id。           |
| `file_patterns`          | 是  | 否  | `[]`        | LSP server 选择时使用的文件 glob。         |
| `initialization_options` | 是  | 否  | 未设置      | 发送到 LSP initialize request 的 JSON 值。 |
| `auto_start`             | 是  | 否  | `true`      | 该 LSP lifecycle 在适用时是否自动启动。    |

## Instructions、hooks 与 feature flags

`instructions` 是可选的 top-level 字符串,会追加在 system prompt 后面。跨配置层合并时,instructions 会用一个空行拼接。

```toml
instructions = """
Follow the repository's Rust and Vue style.
Prefer focused patches and tests that cover the changed behavior.
"""
```

`disabled_mcp_servers` 是累加的 top-level list。project 层可以用它按 id 禁用 user-level MCP server:

```toml
disabled_mcp_servers = ["personal-browser", "experimental-shell"]
```

`[features]` 当前暴露 hooks 开关:

```toml
[features]
hooks = true
```

Command hook 位于 `[hooks.<Event>.<id>]` 下。支持的事件有 `SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PermissionRequest`、`PostToolUse` 和 `Stop`。

```toml
[hooks.Stop.verify]
matcher = "*"
command = "cargo test --workspace --all-targets"
status_message = "Running workspace tests"
timeout_secs = 120
enabled = true

[hooks.PreToolUse.block_rm]
matcher = "shell"
command = "python3 .kairox/hooks/pre_tool.py"
enabled = false
```

| 字段             | 类型   | 默认值 | 说明                                              |
| ---------------- | ------ | ------ | ------------------------------------------------- |
| `matcher`        | string | 未设置 | event-specific selector,例如 tool family 或 `*`。 |
| `command`        | string | —      | 要执行的 shell command,必填。                     |
| `status_message` | string | 未设置 | hook 运行时显示的可选消息。                       |
| `timeout_secs`   | int    | 未设置 | 可选 timeout。                                    |
| `enabled`        | bool   | `true` | 不删除配置的情况下禁用该 hook。                   |

## `[context]` —— compaction 与 token 预算

可选。控制 runtime 何时触发自动 compaction,以及如何为 tool 定义分配预算。

```toml
[context]
auto_compact_threshold = 0.85
# compactor_profile = "fast"
# max_tool_definition_tokens = 25000
```

| 字段                         | 类型   | 默认值             | 说明                                                                                                           |
| ---------------------------- | ------ | ------------------ | -------------------------------------------------------------------------------------------------------------- |
| `auto_compact_threshold`     | float  | `0.85`             | 当组装好的 context 占用达到当前模型预算的这一比例时,runtime 会触发 compaction。`1.0` 表示关闭自动 compaction。 |
| `compactor_profile`          | string | (当前活动 profile) | 用于摘要 LLM 调用的 profile 别名。即便 session 跑在重型推理模型上,也可以钉一个便宜快速的模型来做 compaction。  |
| `max_tool_definition_tokens` | int    | 未设置             | 对序列化后的 MCP tool 定义设置上限。超过时,assembler 会优先丢弃优先级最低的 tool。                             |

compaction pipeline 与防止 compaction 与活动 turn 竞态的 busy-state guard,见 [Memory & Context](../concepts/memory-and-context)。

## `[advisor]` —— tool-call 自反检查

可选。控制 runtime 是否在执行 tool 前,先让第二次 advisor pass 检查计划中的 tool call。

```toml
[advisor]
mode = "lightweight"
# profile = "fast"
# max_concerns = 5
```

| 字段           | 类型   | 默认值 | 说明                                                                                                |
| -------------- | ------ | ------ | --------------------------------------------------------------------------------------------------- |
| `mode`         | string | `off`  | `"off"` 关闭 advisor review。`"lightweight"` 只检查高风险 tool batch。`"full"` 检查每批 tool call。 |
| `profile`      | string | 未设置 | advisor review 使用的 model profile 别名。未设置时复用 session 当前激活的 profile。                 |
| `max_concerns` | int    | `5`    | 单次 review 最多报告的 concern 数量。                                                               |

Advisor review 是 fail-open 的:如果 advisor 模型调用失败或返回无法解析的 JSON,主 agent 会继续执行,runtime 只记录 warning。如果 advisor 返回 `reject`,runtime 会记录 `AdvisorReviewCompleted`,发出一条解释阻断原因的 assistant message,并跳过这一批 tool。

启用 `full` 时建议为 `profile` 指定一个速度快、成本低的模型;advisor 位于 tool 执行前的关键路径上。

## 隐私默认值

当前 `kairox.toml` 中**没有** `[privacy]` 段;隐私相关的默认值在代码里强制执行,而非通过配置。规则如下:

- 使用 `fake` provider 且没有真实 shell tool 的 session,允许开启 verbose tracing 用于开发。
- 使用真实模型客户端或真实 shell tool 的 session,在 production 构建中默认使用**最低级别的 trace**,runtime 在启动时会断言这一点。

如果你在做生产部署、希望放宽这一限制,必须通过 `agent-runtime` 中的 runtime 配置完成,而**不是**通过 TOML 文件。设计意图是:不要让"忘了切配置开关"导致 prompt 或 tool 输出泄漏到共享日志中。

## 环境变量的解析

当前 runtime 在以下三个路径读取环境变量:

1. **profile 的 `api_key_env`。** 在构建 provider 客户端时读取一次。
2. **MCP stdio 的 `env` 表中空值。** `KEY = ""` 表示"读取名为 `KEY` 的环境变量并使用其值"。非空值则原样传入。
3. **MCP SSE / Streamable HTTP 的 `headers` 中的 `${VAR}`。** header 值中 `${VAR}` 形式的子串会在每次请求时展开,因此轮换环境变量就能轮换 header,无需重启 server。

Knowledge base 的 `api_key_env` 会作为 connector 元数据被解析,但当前 build 只接入 SQLite FTS knowledge base adapter,它不需要凭证。后续接入具体服务的 KB adapter 时,应由对应 adapter 消费这个环境变量。

如果某个必需的环境变量缺失,runtime 会发出启动诊断,受影响的 profile 或 server 被标记为不可用,其它配置来源不受影响。

## 本页不涉及的内容

本页是 TOML schema 的参考。runtime 的行为请看 [Runtime & Sessions](../concepts/runtime-and-sessions),MCP 与 skill 背后的概念叙事请看 [Extensibility](../concepts/extensibility)。
