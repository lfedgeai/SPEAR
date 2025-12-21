# Spear Backend Adapter Layer 设计概览

## 1. 背景

Spear 需要在 hostcall 与模型后端之间增加一层 adapter layer：将 Spear hostcall 的参数与状态（例如 chat session）规范化为统一的 Canonical IR，再由路由层把请求分发到“已编译启用”的后端实例。

Chat Completions 是第一阶段能力，但设计必须覆盖后续的图像生成、ASR/TTS、Embeddings 以及实时语音等多操作与多模态能力。

## 2. 目标

- 稳定 hostcall ABI：WASM/任务侧接口尽量不随业务演进频繁变化。
- 可编译裁剪：不同 backend 用 Cargo feature 控制编译启用，未启用的不参与编译/链接。
- 能力驱动路由：请求声明 required/preferred capabilities，路由器只从满足条件的候选中选择。
- 策略驱动选择：支持权重、优先级、负载均衡、fallback、镜像、hedged 等业界惯例。
- 安全隔离：secret（API key）与 URL/网络策略由 host 管控，WASM 不可注入。

## 3. 总体分层

建议按四层拆分（从稳定到可变）：

1) Hostcall 协议层：读取 WASM 内存与参数，写入 host 侧会话/请求状态。
2) Normalize 层：将 host 状态转换为 Canonical IR（见 `ir-zh.md`）。
3) Router 层：基于 capabilities 与 policy 选择 backend instance（见 `routing-zh.md`）。
4) Backend Adapter 层：把 Canonical IR 转为具体后端 API（OpenAI-compatible 或非兼容）并返回 Canonical Response。

## 4. 与 legacy Go 的对齐

legacy Go 中的两类模式可直接继承：

- transform 子集匹配（`legacy/spearlet/hostcalls/transform.go`）对应新设计的 capability-based routing。
- 多 endpoint + env key 筛选（`legacy/spearlet/core/models.go`）对应新设计的 registry/discovery。

实时 ASR（`legacy/spearlet/stream/rt_asr.go`）说明 streaming/realtime 是独立 transport 与生命周期，应作为独立 operation 与子系统处理。

## 5. 文档导航

- Canonical IR：`ir-zh.md`
- 多操作字段骨架与能力建议：`operations-zh.md`
- 能力路由与选路策略：`routing-zh.md`
- backend 编译裁剪、注册、发现与配置：`backends-zh.md`
- realtime/streaming：`streaming-zh.md`
- 错误/安全/可观测性：`reliability-security-observability-zh.md`
- legacy 映射与 MVP 计划：`migration-mvp-zh.md`

