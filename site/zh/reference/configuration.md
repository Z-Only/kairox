---
title: 配置
description: "`kairox.toml` 的发现顺序、profile schema、MCP server schema、context 预算管理,以及完整示例。"
outline: [2, 3]
---

# 配置

Kairox 的所有配置都集中在一个 TOML 文件中:`kairox.toml`(或位于 `.kairox/` 目录下的 `config.toml`)。这个文件的格式在 TUI 和 GUI 之间完全共享,写一次,两边都能读。本页是其中每一个字段的参考手册。

示例的权威来源是仓库根目录下的 [`kairox.toml.example`](https://github.com/Z-Only/kairox/blob/main/kairox.toml.example)。本页负责说明每个字段的含义、何时生效,以及省略时会发生什么。

## 发现顺序

runtime 启动时会按以下顺序查找配置文件,并使用第一个找到的:

1. **项目级配置。** `./.kairox/config.toml`,从当前工作目录起最多向上回溯 5 层父目录。这是 workspace 级别的文件,可以提交到仓库以共享团队约定,也可以加入 gitignore 作为个人覆盖项。
2. **用户级配置。** `~/.kairox/config.toml`,作为按用户的兜底。这里适合放个人 API key 和个人 profile 偏好。
3. **内置默认值。** 如果上面两个文件都不存在,Kairox 提供合理的默认配置:用于离线测试的 `fake` provider、指向 Ollama 的 `local-code` profile,以及当环境中存在 `OPENAI_API_KEY` 时自动启用、指向 OpenAI 的 `fast` profile。

项目级配置**不会**与用户级配置合并 —— 先找到哪个就用哪个。如果你希望两者都生效,请把用户文件中需要的内容复制到项目文件里。

::: tip 项目根目录 vs. workspace 根目录
所谓"项目级配置",是指从进程当前工作目录开始向上查找 `.kairox/config.toml`。在 TUI 中,这就是你执行 `kairox` 命令所在的目录;在 GUI 中,则是创建 session 时选定的 workspace 根目录。五层父目录的向上回溯意味着你可以 `cd` 进任意子目录,依然能找到 workspace 级配置。
:::

## Profile

profile 是为某个模型命名的一组配置。session 通过名称选择 profile;profile 决定使用哪个 provider 客户端、传入什么 model ID,以及从哪个环境变量里取 API key。

### Profile schema

| 字段                 | 类型   | 必填 | 默认值          | 说明                                                                                     |
| -------------------- | ------ | ---- | --------------- | ---------------------------------------------------------------------------------------- |
| `provider`           | string | 是   | —               | 任意 provider 名称。已知值:`anthropic`、`ollama`、`fake`。其它一律走 OpenAI 兼容客户端。 |
| `model_id`           | string | 是   | —               | 发送给 API 的模型标识符(例如 `gpt-4.1`、`claude-sonnet-4-20250514`)。                    |
| `base_url`           | string | 否   | provider 默认值 | API 的 base URL。`anthropic` 可省略以使用官方端点。                                      |
| `api_key`            | string | 否   | —               | 直接写明 API key,优先级高于 `api_key_env`。请不要写入提交的文件中。                      |
| `api_key_env`        | string | 否   | —               | 存放 API key 的环境变量名,运行时解析。                                                   |
| `context_window`     | int    | 否   | 来自模型元数据  | 输入加历史的最大 token 数。按三层兜底查找:profile → `ModelRegistry` → provider 默认值。  |
| `output_limit`       | int    | 否   | 来自模型元数据  | 输出的最大 token 数,兜底逻辑同 `context_window`。                                        |
| `max_tokens`         | int    | 否   | `output_limit`  | 单次响应的上限。Anthropic 会显式用它来设置 `max_tokens` 参数。                           |
| `temperature`        | float  | 否   | provider 默认值 | 采样温度,0.0–2.0。                                                                       |
| `top_p`              | float  | 否   | provider 默认值 | nucleus sampling,0.0–1.0。                                                               |
| `top_k`              | int    | 否   | provider 默认值 | top-k sampling,仅 Anthropic 支持。                                                       |
| `headers`            | table  | 否   | —               | 附加到每次请求的 HTTP header,常用于企业网关。                                            |
| `supports_tools`     | bool   | 否   | 自动探测        | 覆盖自动探测出的 tool calling 能力。                                                     |
| `supports_vision`    | bool   | 否   | 自动探测        | 覆盖自动探测出的视觉能力。                                                               |
| `supports_reasoning` | bool   | 否   | 自动探测        | 覆盖自动探测出的推理能力。                                                               |
| `extra_params`       | table  | 否   | —               | 原样透传给 provider 的特定参数(例如 Anthropic 的 `thinking`)。                           |
| `response`           | string | 否   | —               | 静态响应文本,仅 `fake` provider 使用。                                                   |

### Provider 自动识别

runtime 将 `provider` 映射到具体的客户端实现:

| `provider` 取值     | 客户端                                            |
| ------------------- | ------------------------------------------------- |
| `anthropic`         | Anthropic SDK,走 `messages` 端点                  |
| `ollama`            | Ollama HTTP 客户端(默认 `http://localhost:11434`) |
| `fake`              | 固定返回配置中 `response` 的桩客户端              |
| `openai_compatible` | OpenAI Chat Completions 客户端(显式名称)          |
| 其它任意值          | OpenAI 兼容客户端(Groq、xAI、DeepSeek 等)         |

你不必把新 provider 假装成 `openai_compatible`,直接写 `provider = "deepseek"` 即可。runtime 会把所有未知 provider 都按 OpenAI 兼容处理。

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
| `type`                 | string | —       | `"stdio"` 或 `"sse"`,必填。                                     |
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

## 隐私默认值

当前 `kairox.toml` 中**没有** `[privacy]` 段;隐私相关的默认值在代码里强制执行,而非通过配置。规则如下:

- 使用 `fake` provider 且没有真实 shell tool 的 session,允许开启 verbose tracing 用于开发。
- 使用真实模型客户端或真实 shell tool 的 session,在 production 构建中默认使用**最低级别的 trace**,runtime 在启动时会断言这一点。

如果你在做生产部署、希望放宽这一限制,必须通过 `agent-runtime` 中的 runtime 配置完成,而**不是**通过 TOML 文件。设计意图是:不要让"忘了切配置开关"导致 prompt 或 tool 输出泄漏到共享日志中。

## 环境变量的解析

环境变量在以下三个地方被读取:

1. **profile 的 `api_key_env`。** 在构建 provider 客户端时读取一次。
2. **MCP stdio 的 `env` 表中空值。** `KEY = ""` 表示"读取名为 `KEY` 的环境变量并使用其值"。非空值则原样传入。
3. **MCP SSE 的 `headers` 中的 `${VAR}`。** header 值中 `${VAR}` 形式的子串会在每次请求时展开,因此轮换环境变量就能轮换 header,无需重启 server。

如果某个必需的环境变量缺失,runtime 会发出启动诊断,受影响的 profile 或 server 被标记为不可用,其它 profile 与 server 不受影响。

## 本页不涉及的内容

本页是 TOML schema 的参考。runtime 的行为请看 [Runtime & Sessions](../concepts/runtime-and-sessions),MCP 与 skill 背后的概念叙事请看 [Extensibility](../concepts/extensibility)。
