# CChat 默认模型选择（Default Model Selection）设计

## 1. 背景

当前 CChat 在将会话快照归一化为 `chat_completions` 请求时，如果用户没有通过 `cchat_ctl` 显式设置 `model`，会退回到一个硬编码默认值：

- 代码现状：`normalize_cchat_session` 使用 `unwrap_or("stub-model")`
  - 参考：[chat.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/ai/normalize/chat.rs#L12-L17)

这在使用 stub backend 时是合理的，但当请求被路由到真实 LLM（例如 OpenAI chat completions）时，会导致 upstream 404/400（model_not_found），使用户难以定位问题：

- 错误表现：`upstream status: 404: ... model_not_found: The model 'stub-model' does not exist ...`

## 2. 问题定义

我们希望满足：

- 用户不显式设置 `model` 时，仍能在真实 backend 上得到可用的默认模型。
- 默认模型应可配置，且可按 backend 级别控制。
- 默认模型选择规则透明、可解释，便于排障。
- 不把具体模型名散落到每个 WASM sample 或业务代码里。

## 3. 目标与非目标

### 3.1 目标

- 引入“默认模型”配置，替代对非 stub backend 的 `stub-model` 兜底。
- 定义清晰的优先级：会话参数 > backend 默认 > 全局默认 >（仅 stub）stub-model。
- 对多 backend 场景，避免“随机选一个默认模型”导致行为不可预期。
- 增强可观测性：当无法选择默认模型时，错误信息应指向缺失配置。

### 3.2 非目标

- 不在本设计中实现复杂的“按 feature / transport / json_schema / tools 的模型规则引擎”（可以作为未来增强）。
- 不在本设计中处理模型可用性探测（例如调用 upstream 列表接口）。

## 4. 术语

- **会话参数（session params）**：通过 `cchat_ctl(CTL_SET_PARAM, {key,value})` 写入 `ChatSessionState.params`。
- **backend 默认模型（backend default model）**：在 `[[spearlet.llm.backends]]` 中为某个 backend 声明的默认模型。
- **全局默认模型（global default model）**：在 `[spearlet.llm]` 中声明的默认模型。

## 5. 配置设计

### 5.1 全局默认模型

在 `[spearlet.llm]` 增加字段：

```toml
[spearlet.llm]
default_model = "gpt-4o-mini"
```

语义：当 session 未指定 `model` 且 backend 也没有默认模型时使用。

### 5.2 backend 默认模型（推荐）

在 `[[spearlet.llm.backends]]` 增加字段：

```toml
[[spearlet.llm.backends]]
name = "openai-chat"
kind = "openai_chat_completion"
base_url = "https://api.openai.com/v1"
credential_ref = "openai_chat"
ops = ["chat_completions"]
features = ["supports_tools", "supports_json_schema"]
transports = ["http"]
default_model = "gpt-4o-mini"
weight = 100
priority = 0
```

语义：该 backend 在 chat_completions 操作上，若 session 未指定 `model`，优先使用这里的默认值。

说明：

- 对 OpenAI 这类 provider，默认模型往往与账号权限、地区/组织策略相关，因此更适合由运维/部署侧统一配置。

## 6. 默认模型选择规则

默认模型选择发生在“请求进入路由/调用 adapter”之前，目的是保证请求中 `model` 非空且可解释。

### 6.1 优先级（从高到低）

1. **会话显式 `model`**：`session.params["model"]`（通过 `cchat_ctl` 设置）
2. **路由指定 backend 的默认模型**：当 `session.params["backend"]` 或 routing hints 指定 backend 时，使用该 backend 的 `default_model`
3. **唯一候选 backend 的默认模型**：当路由过滤后只有一个候选 backend 时，使用其 `default_model`
4. **全局默认模型**：`spearlet.llm.default_model`
5. **stub 兜底**：只有当最终选择 backend 为 stub 时，允许回退到 `stub-model`

### 6.2 多候选 backend 的歧义处理

当存在多个候选 backend 且用户未指定 backend、也未指定 model：

- 如果所有候选 backend 的 `default_model` 都存在且相同：可使用该默认值
- 否则：返回错误（InvalidRequest 或 NotSupported），提示用户：
  - 设置 `cchat_ctl(model=...)`，或
  - 设置 `cchat_ctl(backend=...)` 指定 backend

该策略避免“权重随机路由 + 默认模型差异”造成的不可预期行为。

## 7. 错误信息与可观测性

### 7.1 缺失默认模型的错误建议

如果最终选择的 backend 是非 stub，且无法得到任何默认 model：

- 返回错误 message 中应包含：
  - operation（chat_completions）
  - 当前 routing 限制（backend/allowlist/denylist）
  - required_features / required_transports
  - 候选 backend 列表及其 features/transports/default_model（若存在）
  - 行动建议：配置 `spearlet.llm.default_model` 或 backend `default_model`，或在 session 中设置 `model`

### 7.2 日志与 debug

建议在 debug 日志中记录：

- selected_backend
- selected_model
- model_source（session/backend/global/stub）

## 8. 安全性

- 默认模型属于非敏感信息，可进入配置文件。
- 仍需遵循既有规范：API key 只能来自 `credentials[].api_key_env`，不得写入配置文件。

## 9. 迁移方案

### 9.1 向后兼容

短期内可保留 `stub-model` 作为 stub backend 的默认；对非 stub backend 建议逐步从“隐式兜底”迁移到“显式默认”。

### 9.2 配置迁移

- 在 `config/spearlet/config.toml` 中为生产使用的 chat completion backend 补齐：
  - `features = ["supports_tools", "supports_json_schema"]`（若需要 tool calling / json schema）
  - `default_model = "..."`
- 或者在 `[spearlet.llm]` 设置 `default_model` 作为全局兜底。

## 10. 测试计划

- 单元测试：
  - session 指定 model：优先使用 session model
  - routing 指定 backend：使用 backend default_model
  - 单一候选 backend：使用其 default_model
  - 多候选且 default_model 不一致：返回歧义错误
  - stub backend：仍可回退到 stub-model
- 集成测试（WASM sample）：
  - 不在 WASM 里设置 model 时，在真实 backend 上也能跑通（依赖 host 配置 default_model）

