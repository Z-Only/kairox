---
title: 监控器
description: 后台进程监控 — 启动 shell 命令、将 stdout 行流式传递为会话事件、超时和持久模式，以及生命周期管理。
outline: [2, 3]
---

# 监控器

监控器是后台 shell 进程，将其 stdout 以实时事件的形式流入对话。它们让 Agent 观察长时间运行或无限期的进程 — 日志尾部跟踪、文件监控、构建输出、轮询循环 — 而不阻塞主对话轮次。

监控器是读级别工具：创建一个不需要用户确认，与读取文件相同。Agent 在需要后台观察时自动启动。

## 何时使用监控器

- 在修复 bug 时尾随日志文件观察错误。
- 在构建期间监视目录的文件变化。
- 按间隔轮询远程 API 直到满足条件。
- 流式获取 CI 检查结果。
- 任何"当 X 发生时告诉我"的模式。

## 启动监控器

`monitor.start` 工具启动后台进程并立即返回 `MonitorId`。不会触发用户审批提示 — 该工具的风险分类为读级别。

### 参数

| 参数          | 类型   | 必填 | 描述                                                  |
| ------------- | ------ | ---- | ----------------------------------------------------- |
| `command`     | String | 是   | 要运行的 shell 命令。每行 stdout 成为一个事件。       |
| `description` | String | 是   | 简短的人类可读标签，显示在通知和聊天流中。            |
| `timeout_ms`  | u64    | 否   | 在此毫秒数后终止进程。默认 300000（5 分钟）。         |
| `persistent`  | bool   | 否   | 如果为 `true`，忽略超时，运行直到显式停止或会话结束。 |

### 示例

```json
{
  "command": "tail -f /var/log/app.log | grep --line-buffered ERROR",
  "description": "app.log 中的错误",
  "timeout_ms": 600000,
  "persistent": false
}
```

## 超时模式与持久模式

| 模式     | 生命周期                                          | 适用于                                      |
| -------- | ------------------------------------------------- | ------------------------------------------- |
| **超时** | 在 `timeout_ms` 后终止（默认 300s，最大 3600s）。 | 有界观察 — "在接下来的 10 分钟内监视这个"。 |
| **持久** | 运行直到调用 `monitor.stop` 或会话结束。          | 无限期观察 — 日志尾随、文件监视、PR 监控。  |

超时会发出 `MonitorStopped { reason: Timeout }`。进程正常退出会发出 `MonitorStopped { reason: ExitCode { code } }`;用户主动停止会发出 `MonitorStopped { reason: UserStopped }`。

## 事件传递

数据从监控器的 stdout 到用户聊天流的流程：

1. **stdout 行发出。** 后台进程写入一行 stdout。
2. **注册表捕获。** `MonitorRegistry` 从进程句柄读取,将行包装为 `EventPayload::MonitorEvent { monitor_id, line }`。
3. **开始/停止事件包住输出。** `MonitorStarted` 记录 description、persistent 与 timeout;`MonitorStopped` 记录停止原因;启动或读取失败会发出 `MonitorFailed`。
4. **会话接收事件。** 事件像任何其他领域事件一样进入会话的事件流。
5. **UI 渲染。** TUI 通过 `ChatStreamItem::Monitor` 渲染，GUI 通过 `ChatMonitorItem.vue` 组件渲染。

## 列出和停止监控器

### `monitor.list`

返回当前 session registry 里的所有活跃监控器。每一项包含 monitor id、description、persistent 标记和 timeout。

### `monitor.stop`

通过 `MonitorId` 停止运行中的监控器。后台进程会被终止,并发出最终的 `MonitorStopped { reason: UserStopped }`。停止一个已经结束的 monitor 会返回 not-found tool error。

## 领域类型

monitor event 类型位于 `agent-core`;registry 持有的运行时信息位于 `agent-tools`。

| 类型                | 角色                                                                                           |
| ------------------- | ---------------------------------------------------------------------------------------------- |
| `MonitorStarted`    | `monitor.start` 注册进程时发出的 event payload。                                               |
| `MonitorEvent`      | 单行 stdout 对应的 event payload。                                                             |
| `MonitorStopped`    | monitor 停止时发出的 event payload,停止原因记录在 `MonitorStopReason` 中。                     |
| `MonitorFailed`     | spawn 或读取失败对应的 event payload。                                                         |
| `MonitorStopReason` | 枚举:`ExitCode`、`Timeout`、`UserStopped`、`SessionEnded`。                                    |
| `MonitorInfo`       | `agent-tools` 中由 `monitor.list` 返回的描述符:id、description、command、persistent、timeout。 |
