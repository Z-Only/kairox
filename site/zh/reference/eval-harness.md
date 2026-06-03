---
title: 评估工具
description: 基于 JSONL 场景的 Kairox agent 会话无头基准测试运行器。
---

# 评估工具

`kairox-eval` 是 Kairox agent 会话的无头基准测试运行器。它通过与 TUI 和 GUI 相同的 `LocalRuntime`/`AppFacade` 路径执行 JSONL 场景，记录每个场景的 JSONL 结果，并生成用于版本间对比的聚合指标。

## 运行

```bash
cargo run -p agent-eval --bin kairox-eval -- run \
  --scenarios examples/eval/smoke.jsonl \
  --output target/eval/results.jsonl \
  --summary target/eval/summary.json \
  --workspace .
```

默认行为：

- 加载所选 workspace 的标准 Kairox 模型配置；
- 使用 `on_request` 审批策略 + `workspace_write` 沙箱；
- 启用以 `--workspace` 为根目录的内置工具；
- 禁用 MCP server 和 hook 以确保可复现性。

仅在基准测试明确依赖时使用 `--enable-mcp` 或 `--enable-hooks`。

## 场景格式

JSONL 文件中每个非空、非注释行即为一个场景：

```json
{
  "id": "fake-smoke",
  "prompt": "Say hello from the configured fake model.",
  "profile": "fake",
  "approval_policy": "on_request",
  "sandbox_policy": { "kind": "workspace_write" },
  "tags": ["smoke"],
  "expected": {
    "assistant_contains": ["Kairox"],
    "event_types": ["UserMessageAdded", "AssistantMessageCompleted"],
    "min_tool_invocations": 0,
    "max_tool_failures": 0
  }
}
```

| 字段              | 必填 | 说明                                                                                      |
| ----------------- | ---- | ----------------------------------------------------------------------------------------- |
| `id`              | 是   | 场景唯一标识                                                                              |
| `prompt`          | 是   | 发送给 agent 的用户消息                                                                   |
| `profile`         | 否   | 模型 profile 覆盖（回退到 `--profile`，再回退到配置默认值）                               |
| `approval_policy` | 否   | `never` / `on_request` / `always`                                                         |
| `sandbox_policy`  | 否   | `{ "kind": "read_only" }` / `{ "kind": "workspace_write" }` / `{ "kind": "full_access" }` |
| `tags`            | 否   | 字符串标签，用于 `--tags` 过滤                                                            |
| `expected`        | 否   | 断言块（见下文）                                                                          |

### 期望断言

| 字段                   | 类型       | 说明                           |
| ---------------------- | ---------- | ------------------------------ |
| `assistant_contains`   | `string[]` | 助手回复必须包含的子字符串     |
| `event_types`          | `string[]` | trace 中必须出现的领域事件类型 |
| `min_tool_invocations` | `number`   | 预期的最少工具调用次数         |
| `max_tool_failures`    | `number`   | 允许的最大工具失败次数         |

## 结果指标

每个结果行包含：

- 通过/失败状态及期望失败详情；
- 实际使用的 profile；
- 最终助手回复；
- 耗时；
- 事件类型序列；
- 工具调用和失败计数；
- 最后一次上下文输入 token 估算值和上下文窗口（当 runtime 产生该事件时）；
- 设置 `--include-trace` 时包含完整 trace。

汇总报告包含总用例数、通过数、成功率、耗时、工具计数和累计上下文输入 token 估算。

## CLI 参数

| 参数              | 默认值     | 说明                        |
| ----------------- | ---------- | --------------------------- |
| `--scenarios`     | 必填       | JSONL 场景文件路径          |
| `--output`        | 必填       | 逐场景 JSONL 结果输出路径   |
| `--summary`       | 必填       | 聚合 JSON 汇总输出路径      |
| `--workspace`     | `.`        | 工具沙箱的 workspace 根目录 |
| `--profile`       | 配置默认值 | 默认模型 profile            |
| `--tags`          | 全部       | 逗号分隔的标签过滤器        |
| `--fail-fast`     | `false`    | 首次失败即停止              |
| `--include-trace` | `false`    | 在结果中包含完整事件 trace  |
| `--enable-mcp`    | `false`    | 场景执行期间启用 MCP server |
| `--enable-hooks`  | `false`    | 场景执行期间启用 hook       |

## 架构

评估工具基于 TUI 和 GUI 使用的同一个 `LocalRuntime` 构建。它执行完整的 runtime 路径 —— 模型调用、工具执行、策略执行、上下文预算 —— 无需任何 UI 层。这确保评估结果反映生产行为。

`agent-eval` 中的关键类型：

- `EvalHarness` —— 编排场景执行，管理 runtime 的建立和销毁。
- `EvalScenario` —— 解析后的 JSONL 行，包含 prompt、profile、策略、标签和期望。
- `EvalReport` —— 将各场景结果汇总为整体报告。

## 相关链接

- [运行时与会话](../concepts/runtime-and-sessions) —— runtime 如何处理每一 turn。
- [权限与工具](../concepts/permissions-and-tools) —— 评估期间使用的审批和沙箱策略。
- [配置](./configuration) —— 模型 profile 和上下文预算设置。
