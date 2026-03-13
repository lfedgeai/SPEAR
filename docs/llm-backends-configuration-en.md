# LLM Backends Configuration

This document describes how to configure `spearlet` LLM backends and credentials.

## Where it lives

The configuration is loaded from `SPEAR_CONFIG` (TOML) and rendered by Helm into `config.toml` for Kubernetes deployments.

## Credentials

Define API key sources under `[[spearlet.llm.credentials]]` and reference environment variables instead of storing plaintext keys.

```toml
[[spearlet.llm.credentials]]
name = "openai_default"
kind = "env"
api_key_env = "OPENAI_API_KEY"
```

## Backends

Each backend is configured under `[[spearlet.llm.backends]]`.

Required fields:

- `name`: unique backend name
- `kind`: backend implementation kind (string)
- `base_url`: base URL (http(s))
- `hosting`: required, must be `local` or `remote`
- `ops`: supported operations
- `transports`: supported transports

Optional fields:

- `model`: fixed model for some backends
- `credential_ref`: optional secret reference (see below)
- `features`, `weight`, `priority`

Example:

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

## `hosting` semantics

`hosting` is configuration-driven and is used for reporting (Web Admin / SMS) and for clarity in multi-environment deployments.

- `local`: runs on the node (node-local process or local service)
- `remote`: external service (SaaS or remote cluster endpoint)

## `credential_ref` semantics

`credential_ref` is optional:

- If `credential_ref` is set (non-empty):
  - the referenced credential must exist
  - the referenced `api_key_env` must be present and non-empty in the runtime environment, otherwise the backend is filtered as unavailable
- If `credential_ref` is not set:
  - the backend is treated as “no-auth” (no API key header), useful for OpenAI-compatible proxies that do not require a key

## Backend kinds

Common kinds in this repository:

- `openai_chat_completion` (HTTP)
- `openai_realtime_ws` (WebSocket)
- `ollama_chat` (HTTP, node-local)
- `stub` (testing)

## Managed (local model) backends

Some backends are not configured in `config.toml`. They are created and reconciled by local model controllers (e.g. Web Admin “Local AI Models”).

Routing behavior:

- Configured backends (static) form the base registry.
- Managed backends are merged at routing time and can override availability for a provider/model combination on a node.

