# Backend Support Matrix

This document summarizes the backend capability matrix that is already implemented in the current `spear` codebase.

It separates three concerns:

- what operations exist in IR / protocol enums
- what built-in adapters actually implement
- what Web Admin currently allows users to create

This matrix reflects current behavior only. Historical design notes, reserved fields, or future plans are not treated as shipped support.

## Design Principles

- `kind` identifies the backend adapter family, such as `chat_http`
- `operations` identify the externally declared capability, such as `chat_completions`
- a backend can participate in routing only if:
  - the declared operation is valid
  - the selected `kind` actually implements that operation
- the runtime assembly layer validates `kind` / `operations` consistency before registration

## Built-in Runtime Matrix

| Backend family | Hosting | Transport | Currently supported operations | Notes |
|---|---|---|---|---|
| `chat_http` | remote | `http` | `chat_completions` | Provider-neutral HTTP chat adapter used by OpenAI and Qwen |
| `ollama_chat` | remote or local endpoint-style | `http` | `chat_completions` | Ollama-compatible HTTP adapter |
| `rt_asr_openai_ws` | remote | `websocket` | `speech_to_text` | OpenAI-style realtime websocket ASR adapter |
| `rt_asr_qwen_ws` | remote | `websocket` | `speech_to_text` | Qwen realtime websocket ASR adapter |
| `llamacpp` | local | `http` | `chat_completions` | Node-local managed `llama-server` adapter |
| `stub` | local / internal | in-process | `chat_completions` | Test / development only |

## Current Web Admin Create Flows

Web Admin now exposes two explicit create paths:

- `Create Remote Backend`
- `Create Local Backend`

### Remote

Current remote combinations available from the UI:

| Hosting | Provider | Backend family | Default / common operations | Notes |
|---|---|---|---|---|
| `remote` | `openai` | `chat_http` | `chat_completions` | Standard remote HTTP chat backend |
| `remote` | `openai` | `rt_asr_openai_ws` | `speech_to_text` | Remote websocket ASR backend |
| `remote` | `qwen` | `chat_http` | `chat_completions` | Remote Qwen chat backend through the shared HTTP chat adapter |
| `remote` | `qwen` | `rt_asr_qwen_ws` | `speech_to_text` | Remote Qwen websocket ASR backend |
| `remote` | `ollama` | `ollama_chat` | `chat_completions` | External Ollama-compatible endpoint |

### Local

Current local combinations available from the UI:

| Hosting | Provider | Backend family | Current behavior | Notes |
|---|---|---|---|---|
| `local` | `llamacpp` | `llamacpp` | managed local runtime | Starts / manages node-local `llama-server` |
| `local` | `vllm` | `vllm` | scaffolded / external-endpoint oriented | Not yet a fully managed local process path |

### Local llama.cpp surfaced fields

For `local + llamacpp`, Web Admin now exposes common runtime fields directly and persists them into backend metadata:

- `model_url`
- `model_path`
- `skip_download`
- `download_timeout_s`
- `threads`
- `ctx_size`

## Operations Present in Protocol But Not Fully Implemented

The following operations may still appear in parts of IR, filters, or future-facing abstractions, but there is no complete built-in adapter support today:

- `embeddings`
- `image_generation`
- `text_to_speech`
- `realtime_voice`

So when judging support, use the runtime adapter matrix plus `kind` / `operation` validation, not enum presence alone.

## Recommended Current Usage

- Standard chat backend:
  - `kind = chat_http`
  - `operations = ["chat_completions"]`
- Realtime ASR backend:
  - `kind = rt_asr_openai_ws` or `rt_asr_qwen_ws`
  - `operations = ["speech_to_text"]`
- Local llama.cpp backend:
  - `kind = llamacpp`
  - `operations = ["chat_completions"]`
  - plus local metadata-backed runtime fields

## Discouraged Configurations

The following should not be treated as valid runtime-ready configurations in the current codebase:

- `rt_asr_openai_ws` + `realtime`
- `rt_asr_openai_ws` + `realtime_voice`
- `rt_asr_qwen_ws` + `realtime`
- `rt_asr_qwen_ws` + `realtime_voice`
- `chat_http` + `speech_to_text`
- any `kind` declaring an operation that its adapter does not implement

## Recommended Extension Order

If a new operation needs to become first-class support:

1. implement the adapter behavior first
2. register the `kind` / `operation` mapping in runtime assembly
3. expose it in Web Admin only after runtime support exists

Avoid exposing operations in UI before the runtime can actually execute them.

## Related Current Docs

- [web-admin-overview-en.md](./web-admin-overview-en.md) for the current Web Admin page and API surface
- [web-admin-ui-guide-en.md](./web-admin-ui-guide-en.md) for actual operator workflows
- [ai-backend-unified-control-plane-design-en.md](./ai-backend-unified-control-plane-design-en.md) for the control-plane model behind backend identity, placement, and status
