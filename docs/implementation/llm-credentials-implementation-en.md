# LLM Credentials / credential_ref (Detailed design and implementation)

This document describes the implemented approach in this repository: introduce centralized LLM credentials under `spearlet.llm.credentials[]`, and let each backend reference a credential via `credential_ref`.

This enables:

- Different backends using different API keys (e.g., chat vs realtime ASR)
- Multiple backends sharing the same credential without duplicating config
- No plaintext keys in config files (only env-var names)

## 0. Current state

- `LlmConfig` includes `credentials` and `backends`: see [config.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/config.rs#L351-L406)
- `LlmBackendConfig` no longer supports `api_key_env`; it must use `credential_ref`
- To fully remove the legacy path, `LlmConfig/LlmCredentialConfig/LlmBackendConfig` use `deny_unknown_fields`: a config containing `api_key_env` under `[[spearlet.llm.backends]]` fails to parse

Registry behavior:

- Only `credential_ref` is supported: resolve the env-var name via the referenced credential and filter the backend if the env var is missing in `RuntimeConfig.global_environment`: see [registry.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/host_api/registry.rs#L11-L105)

## 1. Schema (TOML)

```toml
[spearlet.llm]
default_policy = "weighted_random"

[[spearlet.llm.credentials]]
name = "openai_chat"
kind = "env"
api_key_env = "OPENAI_CHAT_API_KEY"

[[spearlet.llm.credentials]]
name = "openai_realtime"
kind = "env"
api_key_env = "OPENAI_REALTIME_API_KEY"

[[spearlet.llm.backends]]
name = "openai-chat"
kind = "openai_chat_completion"
base_url = "https://api.openai.com/v1"
credential_ref = "openai_chat"
ops = ["chat_completions"]
transports = ["http"]
weight = 100
priority = 0

[[spearlet.llm.backends]]
name = "openai-realtime-asr"
kind = "openai_realtime_ws"
base_url = "https://api.openai.com/v1"
credential_ref = "openai_realtime"
ops = ["speech_to_text"]
transports = ["websocket"]
weight = 100
priority = 0
```

## 2. Rust structs (src/spearlet/config.rs)

```rust
pub struct LlmConfig {
    pub default_policy: Option<String>,
    pub credentials: Vec<LlmCredentialConfig>,
    pub backends: Vec<LlmBackendConfig>,
}

pub struct LlmCredentialConfig {
    pub name: String,
    pub kind: String,      // v1: "env"
    pub api_key_env: String,
}

pub struct LlmBackendConfig {
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub credential_ref: Option<String>,
    pub weight: u32,
    pub priority: i32,
    pub ops: Vec<String>,
    pub features: Vec<String>,
    pub transports: Vec<String>,
}
```

## 3. RuntimeConfig.global_environment injection (best practice)

Problem: if `RuntimeConfig.global_environment` is empty, backends that require API keys will be filtered, and `${env:...}` template expansion will fail.

Implemented approach:

- Collect only the env vars referenced by `credential_ref` (i.e., `credentials[].api_key_env` that are actually used by configured backends)
- Read those env vars from the OS process environment and inject them into each runtime's `RuntimeConfig.global_environment`

Implementation: [function_service.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/function_service.rs#L57-L92)

Security note:

- Do not inject `std::env::vars()` wholesale; only inject env vars referenced by configuration

## 4. Backend registry logic

Implementation: [registry.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/host_api/registry.rs#L11-L163)

Behavior:

- Build a credential index from `llm.credentials[]`
- For backends that require API keys, resolve `api_key_env` via `credential_ref`
- Filter a backend if:
  - `credential_ref` is missing
  - the referenced credential does not exist
  - the resolved env var is missing in `RuntimeConfig.global_environment`

## 5. Migration

- `[[spearlet.llm.backends]] api_key_env = ...` is removed and rejected by parsing (deny_unknown_fields)
- Migrate by moving env-var names into `[[spearlet.llm.credentials]]` and referencing them from backends via `credential_ref`
