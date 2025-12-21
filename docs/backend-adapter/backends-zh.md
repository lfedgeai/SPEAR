# Backend 编译裁剪、注册发现与配置

本文件聚焦“已编译启用的 backend 如何被发现并参与路由”。

## 1. 编译裁剪（Cargo features）

建议每个 backend 一个 feature，并将重依赖设为 `optional`：

- `backend-openai`（OpenAI-compatible HTTP）
- `backend-azure-openai`
- `backend-vllm`
- `backend-openai-realtime`（WebSocket）
- `backend-stub`

注册表构建通过 `#[cfg(feature = "backend-xxx")]` 注册对应 backend；未启用 feature 的 backend 不参与编译与链接。

## 2. BackendKind 与 BackendInstance

建议区分两层：

- `BackendKind`：实现类型（openai_compatible/azure/vllm/realtime...）
- `BackendInstance`：具体实例（base_url、region、权重、优先级、capabilities、limits）

路由选择对象是 instance。

## 3. Registry 与 CapabilityIndex

- `BackendRegistry`：持有所有启用的 instances、其 capabilities、权重、健康状态句柄
- `CapabilityIndex`：从 registry 派生索引（如 `Operation -> candidates[]`）

legacy 对齐：`GetAPIEndpointInfo` 通过 env key 是否存在过滤 endpoint（`legacy/spearlet/core/models.go`），新设计把这一逻辑收敛到 registry 构建与 discovery。

## 4. Discovery（发现接口）

这里的“discovery”是指“对外暴露当前进程内 registry 的可观测视图”，不是指 backend 之间必须通过网络去互相发现。

- 进程内：router/adapter 直接通过函数调用读取 `BackendRegistry`（这是默认路径，不需要任何 HTTP/gRPC）。
- 对外：可选提供 HTTP/gRPC 端点，用于运维/调试/UI/自动化检查，查看“已编译启用 + 已配置 + 当前健康”的 backend 与 capabilities。

建议提供两类：

1) 控制面（HTTP/gRPC）
- `GET /api/v1/backends`
- `GET /api/v1/capabilities`

2) 任务侧自适应（可选）
- 在 hostcall control 命令中提供 `GET_CAPABILITIES`，返回 JSON

## 5. 配置模型（示例）

示例（TOML 伪代码）：

```toml
[llm]
default_policy = "weighted_round_robin"

[[llm.backends]]
name = "openai-us"
kind = "openai_compatible"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
weight = 80
priority = 10
ops = ["chat_completions", "text_to_speech"]
features = ["supports_stream", "supports_tools", "supports_json_schema"]
transports = ["http"]

[[llm.backends]]
name = "openai-realtime"
kind = "openai_realtime"
base_url = "https://api.openai.com"
api_key_env = "OPENAI_API_KEY"
weight = 100
priority = 20
ops = ["realtime_voice"]
features = ["supports_bidi_stream", "supports_audio_input", "supports_audio_output"]
transports = ["websocket"]
```

## 6. Secret 与网络策略

- `api_key_env` 与 base_url 必须由 host 配置提供；WASM 不可注入。
- backend allowlist/denylist 由 host 配置控制，请求侧只能收缩。
