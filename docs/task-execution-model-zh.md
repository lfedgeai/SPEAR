# Task 执行模型（方案 A）

本仓库采用“统一请求执行（Request/Response）+ 可选流式”的模型：

- Task 不再区分 “long running / short running”。
- Task 表达的是“可被调用的可执行单元”（具备 endpoint、可执行描述、配置等）。
- 一次调用的交互方式由 invocation/request 的 `execution_mode` 决定（如 sync/async/stream）。

## 设计目标

- 减少概念分叉：Task 状态/类型不承载运行时策略差异。
- 让策略落到可观测且可调参数：超时、并发、放置/调度、重试等。
- 运行时可优化但不暴露为业务枚举：实例复用、预热、缓存属于调度/执行层细节。

## 行为约定

- “是否已有实例”不会作为 Task 的类型；对外仅体现为运行状态与可用性（如 active/inactive 等）。
- “Run/Invoke” 是一次新的执行请求；即使 Task 当前处于 active，也不阻止再次发起执行（由并发/资源限制约束）。

