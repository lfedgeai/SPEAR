# 任务事件订阅器

## 概述

SPEARlet 内置任务事件订阅器，通过连接 SMS 的 `TaskService` 订阅与当前节点相关的任务生命周期事件。订阅器将游标持久化以保证重启后能继续处理（近似一次性处理），并支持可配置的自动重连与退避。

## 组件

- `TaskEventSubscriber`：维护配置与最后处理的事件 ID 游标。
- 节点 UUID 推导：当 `node_name` 是合法 UUID 时直接使用；否则基于 `grpc.addr`、`grpc.port` 与 `node_name` 通过 UUIDv5 派生稳定的 UUID。
- 游标持久化：将 `last_event_id` 存储在 `storage.data_dir` 下，文件名为 `task_events_cursor_{node_uuid}.json`。

## 核心行为

- 通过 `sms_addr` 使用 gRPC 连接 SMS，携带 `node_uuid` 与 `last_event_id` 调用 `SubscribeTaskEvents`。
- 事件流处理：每收到事件时——
  - 更新内存中的 `last_event_id` 并写入持久化文件。
  - 对 `Create` 事件，调用 `GetTask` 获取任务详情，并为后续执行分发做准备（当前为 `todo!` 占位）。
- 忽略不属于当前节点的事件。
- 在连接或流错误时自动重连，等待时间由 `sms_connect_retry_ms` 控制。

## 配置

相关 `SpearletConfig` 字段：

```toml
[spearlet]
node_name = "spearlet-node"
sms_addr = "127.0.0.1:50051"
auto_register = true
heartbeat_interval = 30
cleanup_interval = 300
sms_connect_timeout_ms = 15000
sms_connect_retry_ms = 500
reconnect_total_timeout_ms = 300000

[spearlet.grpc]
addr = "0.0.0.0:50052"

[spearlet.http]
cors_enabled = true
swagger_enabled = true

[spearlet.storage]
backend = "memory"
data_dir = "./data/spearlet"
```

支持的环境变量：`SPEARLET_SMS_ADDR`、`SPEARLET_SMS_CONNECT_TIMEOUT_MS`、`SPEARLET_SMS_CONNECT_RETRY_MS`、`SPEARLET_RECONNECT_TOTAL_TIMEOUT_MS`、`SPEARLET_STORAGE_DATA_DIR`。

## 使用方式

在 SPEARlet 初始化阶段启动订阅器：

```rust
use std::sync::Arc;
use spear_next::spearlet::{config::SpearletConfig, task_events::TaskEventSubscriber};

let config = Arc::new(SpearletConfig::default());
let subscriber = TaskEventSubscriber::new(config.clone());
subscriber.start().await; // 后台运行
```

订阅器将游标持久化到 `storage.data_dir` 中，SPEARlet 重启后可以从最后处理的事件续订。

## 错误处理与韧性

- 连接失败时按 `sms_connect_retry_ms` 延迟重试。
- 优雅处理流错误，延迟后重新订阅。
- 校验 `node_uuid`，仅处理当前节点目标事件。
- 游标文件目录若不存在会自动创建。

## 测试

- 游标读写回路测试：`src/spearlet/task_events_test.rs` 验证 `store_cursor`/`load_cursor` 行为。
- 建议的集成测试：模拟 SMS 事件流与重连场景。

## 代码引用

- `src/spearlet/task_events.rs:44` — 订阅器启动与重连循环
- `src/spearlet/task_events.rs:76` — 事件处理与 `Create` 事件任务详情获取
- `src/spearlet/config.rs:259` — 默认配置值

---

本文档与任务事件订阅器的最新实现保持一致，便于后续扩展与维护。
