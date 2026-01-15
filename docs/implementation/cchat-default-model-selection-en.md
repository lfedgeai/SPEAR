# CChat Default Model Selection Design

## 1. Background

When CChat normalizes a session snapshot into an upstream `chat_completions` request, if the user does not explicitly set `model` via `cchat_ctl`, the current implementation falls back to a hard-coded default:

- Current behavior: `normalize_cchat_session` uses `unwrap_or("stub-model")`
  - Reference: [chat.rs](../../src/spearlet/execution/ai/normalize/chat.rs)

This is fine for the stub backend, but when the request is routed to a real LLM backend (e.g. OpenAI Chat Completions), it commonly results in upstream 404/400 (`model_not_found`). This makes the root cause hard to diagnose.

## 2. Problem statement

We want:

- If the user does not set `model`, real backends should still get a usable default model.
- The default model must be configurable, ideally per-backend.
- The rule must be transparent and debuggable.
- Avoid scattering concrete model names across WASM samples or business code.

## 3. Goals and non-goals

### 3.1 Goals

- Introduce a configurable “default model” that replaces `stub-model` fallback for non-stub backends.
- Clear precedence: session param > backend default > global default > (stub only) `stub-model`.
- Avoid unpredictable behavior in multi-backend routing.
- Improve observability: missing default configuration should be obvious from the error.

### 3.2 Non-goals

- No complex rule engine (by feature/transport/json_schema/tools) in this iteration.
- No upstream model availability probing (e.g. listing models).

## 4. Terminology

- **Session params**: values stored into `ChatSessionState.params` via `cchat_ctl(CTL_SET_PARAM, {key,value})`.
- **Backend default model**: a default model declared in `[[spearlet.llm.backends]]`.
- **Global default model**: a fallback model declared in `[spearlet.llm]`.

## 5. Configuration

### 5.1 Global default model

Add a field under `[spearlet.llm]`:

```toml
[spearlet.llm]
default_model = "gpt-4o-mini"
```

Semantics: used when session has no `model` and the selected backend also has no `default_model`.

### 5.2 Backend default model (recommended)

Add a field under `[[spearlet.llm.backends]]`:

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

Semantics: when this backend is selected for `chat_completions` and the session does not set `model`, use this default.

## 6. Default model selection rules

Selection happens before routing/adapters are invoked, to ensure the request has a non-empty, explainable `model`.

### 6.1 Precedence (high to low)

1. **Explicit session `model`**: `session.params["model"]`
2. **Backend default model for an explicitly selected backend**: when `session.params["backend"]` (or routing hints) selects a backend
3. **Default model of the single remaining candidate backend**: after routing filters yield exactly one candidate
4. **Global default model**: `spearlet.llm.default_model`
5. **Stub fallback**: only when the selected backend is the stub backend, fall back to `stub-model`

### 6.2 Ambiguity handling for multiple candidates

If there are multiple candidate backends and the user sets neither backend nor model:

- If all candidates have `default_model` and they are identical: use that shared value.
- Otherwise: return an error telling the user to either:
  - set `cchat_ctl(model=...)`, or
  - set `cchat_ctl(backend=...)`.

This avoids “weighted routing + differing default models” producing non-deterministic behavior.

## 7. Errors and observability

If a non-stub backend is selected but no default model can be determined:

- The error should include:
  - operation (`chat_completions`)
  - routing constraints (backend/allowlist/denylist)
  - required_features / required_transports
  - candidate backends with their features/transports/default_model (if any)
  - suggested actions: configure `spearlet.llm.default_model` or backend `default_model`, or set session `model`

Recommend logging at debug level:

- selected_backend
- selected_model
- model_source (session/backend/global/stub)

## 8. Security

- Default model names are not sensitive and can be kept in config.
- API keys must continue to come from `credentials[].api_key_env` only.

## 9. Migration

- Keep `stub-model` only for the stub backend.
- For real backends, migrate from implicit fallback to explicit defaults.

## 10. Test plan

- Unit tests:
  - session specifies model
  - backend specified by routing uses its default_model
  - single candidate backend uses its default_model
  - multiple candidates with mismatching default_model returns ambiguity error
  - stub backend still falls back to `stub-model`
- Integration (WASM sample):
  - when WASM does not set model, real backend still works via host-side default_model

