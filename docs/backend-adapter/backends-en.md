# Backend Feature Pruning, Registry/Discovery, and Configuration

This document focuses on how “compiled and enabled” backends are discovered and participate in routing.

## 1. Compile-time pruning (Cargo features)

Use one Cargo feature per backend and mark heavy dependencies as `optional`:

- `backend-openai` (OpenAI-compatible HTTP)
- `backend-azure-openai`
- `backend-vllm`
- `backend-openai-realtime` (WebSocket)
- `backend-stub`

The registry builder registers backends behind `#[cfg(feature = "backend-xxx")]`; disabled backends do not compile/link.

## 2. BackendKind vs BackendInstance

Split into two layers:

- `BackendKind`: implementation type (openai_compatible/azure/vllm/realtime...)
- `BackendInstance`: concrete endpoint (base_url, region, weight, priority, capabilities, limits)

The router selects instances.

## 3. Registry and CapabilityIndex

- `BackendRegistry`: holds enabled instances, their capabilities, weights, and health handles
- `CapabilityIndex`: derived index (e.g., `Operation -> candidates[]`)

Legacy alignment: `GetAPIEndpointInfo` filters endpoints by env keys (`legacy/spearlet/core/models.go`); the new design centralizes this in registry construction and discovery.

## 4. Discovery surfaces

“Discovery” here means exposing an observable view of the in-process registry. It does not mean backends must discover each other via network calls.

- In-process: router/adapters read `BackendRegistry` via normal function calls (default path; no HTTP/gRPC required).
- External: optionally expose HTTP/gRPC endpoints for ops/debug/UI/automation to inspect “compiled + configured + currently healthy” backends and capabilities.

Two recommended surfaces:

1) Control-plane (HTTP/gRPC)
- `GET /api/v1/backends`
- `GET /api/v1/capabilities`

2) Task-side adaptation (optional)
- a hostcall control command like `GET_CAPABILITIES` returning JSON

## 5. Configuration model (example)

TOML pseudocode:

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

## 6. Secrets and network policy

- `api_key_env` and `base_url` must be host-configured; WASM cannot inject them.
- Host-config allowlist/denylist is authoritative; request-side hints can only restrict further.
