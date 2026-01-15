# ExecutionResponse 字段与 metadata 的边界

本文说明 Spear 如何表达执行的关键标识字段，以及为什么这些字段不应该放在 `metadata` 中。

## 背景

历史上，一些“标识类”字段（例如 `invocation_id`、`task_id`、`function_name`、`instance_id`）被塞进了 `ExecutionResponse.metadata`。
这会带来两个问题：

- 这些 key 语义特殊且应稳定，不应与运行时的可变 metadata 混在一起。
- `metadata` 的定位是运行时/辅助信息，可能被过滤、覆盖或复用。

## 当前行为

`ExecutionResponse` 现在将以下标识字段提升为显式字段：

- `invocation_id`
- `task_id`
- `function_name`
- `instance_id`

`metadata` 仅用于承载运行时返回的 metadata（来自 `RuntimeExecutionResponse.metadata` 的字符串化结果）。

## 代码位置

- `ExecutionResponse` 定义：[execution/mod.rs](../src/spearlet/execution/mod.rs)
- 执行记录填充：[manager.rs](../src/spearlet/execution/manager.rs)
- gRPC 映射到 proto `Execution`：[function_service.rs](../src/spearlet/function_service.rs)

