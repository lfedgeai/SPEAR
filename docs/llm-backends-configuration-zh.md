# LLM Backends 配置说明

本文说明如何配置 `spearlet` 的 LLM backends 与凭证（credentials）。

## 配置入口

配置通过 `SPEAR_CONFIG`（TOML）加载；在 Kubernetes 场景下由 Helm 渲染为 `config.toml`。

## Credentials（凭证）

在 `[[spearlet.llm.credentials]]` 下定义密钥来源，通过环境变量引用，避免在配置文件中保存明文密钥。

```toml
[[spearlet.llm.credentials]]
name = "openai_default"
kind = "env"
api_key_env = "OPENAI_API_KEY"
```

## Backends（后端）

每个 backend 配置在 `[[spearlet.llm.backends]]` 下。

必填字段：

- `name`：backend 名称（唯一）
- `kind`：backend 实现类型（字符串）
- `base_url`：服务地址（http(s)）
- `hosting`：必填，只允许 `local` 或 `remote`
- `ops`：支持的操作
- `transports`：支持的传输方式

可选字段：

- `model`：固定模型（部分 backend 支持）
- `credential_ref`：可选的密钥引用（见下文）
- `features`, `weight`, `priority`

示例：

```toml
[[spearlet.llm.backends]]
name = "openai-chat"
kind = "openai_chat_completion"
base_url = "https://api.openai.com/v1"
hosting = "remote"
model = "gpt-4o-mini"
credential_ref = "openai_default"
ops = ["chat_completions"]
features = ["supports_tools", "supports_json_schema"]
transports = ["http"]
weight = 100
priority = 0

[[spearlet.llm.backends]]
name = "openai-realtime-asr"
kind = "openai_realtime_ws"
base_url = "https://api.openai.com/v1"
hosting = "remote"
credential_ref = "openai_default"
ops = ["speech_to_text"]
transports = ["websocket"]
weight = 100
priority = 0
```

## `hosting` 语义

`hosting` 完全以配置为准，主要用于上报与展示（Web Admin / SMS），并让多环境部署更清晰。

- `local`：节点本地（本地进程或本地服务）
- `remote`：远端服务（SaaS 或远端集群）

## `credential_ref` 语义

`credential_ref` 为可选：

- 若配置了 `credential_ref`（非空）：
  - 必须存在同名 credential
  - 对应的 `api_key_env` 必须在运行时环境中存在且非空，否则该 backend 会被视为不可用并被过滤
- 若未配置 `credential_ref`：
  - 视为“无需鉴权”（不会附加 API key header），适用于自建 OpenAI-compatible 代理等场景

## 常见 backend kind

本仓库常见 kind：

- `openai_chat_completion`（HTTP）
- `openai_realtime_ws`（WebSocket）
- `ollama_chat`（HTTP，节点本地）
- `stub`（测试用）

## Managed（本地模型）backends

部分 backends 不来自 `config.toml`，而是由本地模型控制器（例如 Web Admin 的 Local AI Models）创建并持续 reconcile。

路由行为：

- 静态配置 backends 构成基础 registry。
- managed backends 会在路由时合并进入候选集合，用于表达某节点上已部署/可用的本地模型实例。

