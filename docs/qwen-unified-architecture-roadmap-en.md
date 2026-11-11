# Qwen Unified Integration Architecture Roadmap

## 1. Goal

This document defines a **non-backward-compatible** roadmap for integrating Qwen into the current AI backend architecture without introducing provider-specific spaghetti logic.

The goals are:

- Make `provider` represent vendor and control-plane ownership, not protocol details.
- Make `kind` represent adapter / protocol family, not a provider-locked implementation label.
- Make `operation` represent capability type, and `model` represent the concrete model name.
- Reuse existing adapters when Qwen shares the same protocol family with existing backends.
- Introduce dedicated kinds only when the protocol or transport meaningfully diverges.
- Leave a stable extension path for Qwen text generation, multimodal understanding, embeddings, rerank, TTS, image generation, and richer realtime capabilities.

Related current documents:

- [backend-adapter/backends-en.md](backend-adapter/backends-en.md)
- [implementation/realtime-asr-implementation-en.md](implementation/realtime-asr-implementation-en.md)
- [web-admin-ui-guide-en.md](web-admin-ui-guide-en.md)

## 2. Background

Based on the Qwen model-selection page and related subpages, Qwen is not a single model family with a single API shape. It is a platform exposing multiple capability families:

- Text generation and multimodal understanding: many models can be addressed through a chat-style invocation pattern with long context, tool calling, and thinking modes.
- Speech recognition: realtime and non-realtime paths are distinct.
- Realtime API: supports `AOQ`, `WebRTC`, and `WebSocket`.
- Embeddings and rerank: separate capability surfaces with their own request/response structures.
- Image / video generation and TTS: separate capability families again.

Therefore, Qwen should not be integrated as:

- `provider = qwen`
- plus an ever-growing set of scattered `qwen_chat`, `qwen_vision`, `qwen_embedding`, `qwen_tts`, `qwen_image`, and `qwen_realtime_*` branches

Instead, the architecture should prefer:

- shared adapters for shared protocol families
- shared operations for shared capability families
- shared provider handling for credentials, preflight, Web Admin presentation, and vendor-level documentation

## 3. Problems in the Current Architecture

### 3.1 `kind` and `provider` are too tightly coupled

Today, `CanonicalBackendKind` infers a default provider, and config validation requires `provider` to match `kind`.

That prevents natural modeling of cases like:

- `provider = qwen`
- `kind = openai-compatible chat family`

even when the actual API contract is close enough to share one adapter family.

### 3.2 `provider` is implicitly treated like a model family

The official Qwen pages show that the platform hosts both Qwen-native and third-party models under the same platform family.

That means:

- `provider = qwen`

should mean:

- the backend uses DashScope / Model Studio control-plane conventions
- credentials, preflight, Web Admin display, and operational ownership belong to Qwen

It should **not** mean:

- the model itself must always belong to the Qwen-native model line

### 3.3 Operations are on the right track, but payloads are still narrow

The current IR already includes:

- `chat_completions`
- `embeddings`
- `image_generation`
- `speech_to_text`
- `text_to_speech`

This is a good direction, but several payloads are still narrower than the real Qwen capability surfaces:

- embedding dimensions and fusion / separate-vector modes
- richer image-generation parameters such as edit/reference controls
- richer TTS voice controls
- more structured realtime multimodal parameters

## 4. Design Principles

### 4.1 Clean separation of concerns

- `provider`
  - vendor, control plane, credential family, preflight family, Web Admin grouping
- `kind`
  - adapter / protocol family
- `operation`
  - capability type
- `transport`
  - `http`, `websocket`, `webrtc`, `aoq`, and so on
- `model`
  - concrete model identifier
- `features`
  - declared capability flags such as tools, thinking, vision input, audio input, and built-in tools

### 4.2 Shared protocol means shared adapter

If multiple providers are sufficiently close in endpoint shape, auth style, request schema, response schema, and error model, they should share the same adapter family.

### 4.3 Divergent protocol means dedicated kind

A provider-specific kind is justified only when at least one of the following is true:

- endpoint shape differs materially
- request body schema differs materially
- streaming or realtime protocol differs materially
- response or error model differs materially
- control-plane preflight needs a dedicated path

### 4.4 No backward-compatibility burden

This roadmap assumes we are free to:

- rename backend kinds
- adjust typed-config semantics
- change Web Admin create/edit flows
- migrate tests and docs directly without preserving old aliases

## 5. Target Architecture

## 5.1 Provider semantics

Recommended provider set:

- `openai`
- `qwen`
- `ollama`
- `llamacpp`
- `vllm`
- `internal`

In this model:

- `provider = qwen` means the backend belongs to DashScope / Model Studio as a vendor/control-plane family
- it does **not** mean the underlying model must be a Qwen-native model line

## 5.2 Kind semantics

`kind` should become a real adapter-family label.

Recommended target kinds:

- `chat_http`
  - shared chat-style HTTP JSON adapter family
  - initial providers:
    - `openai`
    - `qwen`
- `rt_asr_openai_ws`
  - OpenAI-style realtime ASR websocket
- `rt_asr_qwen_ws`
  - Qwen realtime ASR websocket
- `embeddings_dashscope_http`
  - DashScope embedding family
- `rerank_dashscope_http`
  - DashScope rerank family
- `image_generation_dashscope_http`
  - first-stage DashScope image-generation family
- `text_to_speech_dashscope_http`
  - DashScope TTS family
- `realtime_multimodal_qwen_ws`
- `realtime_multimodal_qwen_webrtc`
- `realtime_multimodal_qwen_aoq`
- `ollama_chat`
- `llamacpp`
- `vllm`
- `stub`

The most important change is:

- replace `openai_chat_completion`
- with a provider-neutral `chat_http`

This allows a large portion of Qwen text-generation and multimodal-understanding models to fit naturally into the existing chat architecture instead of forcing a dedicated `qwen_llm` adapter.

## 5.3 Operation extensions

The most important missing operation is:

- `rerank`

Recommended target operations:

- `chat_completions`
- `embeddings`
- `rerank`
- `image_generation`
- `speech_to_text`
- `text_to_speech`
- later, when needed:
  - `realtime_multimodal`
  - `speech_to_speech`

## 5.4 Feature declarations

Provider differences should be represented as capability flags whenever possible:

- `supports_stream`
- `supports_tools`
- `supports_thinking`
- `supports_structured_output`
- `supports_vision_input`
- `supports_video_input`
- `supports_audio_input`
- `supports_audio_output`
- `supports_builtin_tools`
- `supports_fusion_embedding`
- `supports_separate_embedding`

## 6. Phased Roadmap

## Phase 0: semantic cleanup for provider and kind

Goal: make `provider` and `kind` mean exactly one thing each.

Changes:

- rewrite `CanonicalBackendKind` naming and semantics
- remove the assumption that each kind maps to exactly one provider
- make `kind` represent adapter family only
- make `provider` represent vendor/control-plane family only
- centralize canonical backend family ids in Rust type-layer constants and a shared Web Admin registry so runtime, forms, and tests stop carrying duplicated literals
- centralize canonical backend provider ids as well, so `openai/qwen/ollama/llamacpp/vllm` do not remain scattered across runtime, control-plane, and frontend logic
- align Web Admin test fixtures with `AI_BACKEND_PROVIDERS.*` too, so `provider_family` literals do not drift again at the test layer

Primary files:

- `src/ai_backend_types.rs`
- `src/spearlet/config.rs`
- `src/sms/ai_backends/validator.rs`
- `src/sms/web_admin/ai_backend_admin.rs`
- `web-admin/src/features/ai-backends/*`

Acceptance criteria:

- the following combination is valid:
  - `provider = qwen`
  - `kind = chat_http`
- typed views, validators, and Web Admin all accept that shape
- Rust runtime code and Web Admin defaults both consume one shared family-id source of truth
- Rust and Web Admin logic consume one shared provider-id source of truth too

## Phase 1: unify chat and multimodal understanding

Goal: let a large portion of Qwen text-generation and multimodal-understanding models use one shared chat-completions family.

Changes:

- rename or refactor `OpenAIChatCompletionBackendAdapter` into a provider-neutral adapter family, for example:
  - `ChatHttpBackendAdapter`
- keep shared support for:
  - `messages`
  - `tools`
  - passthrough `params`
- allow `provider = qwen` to use this kind
- allow both `openai` and `qwen` in preflight for this kind

Why first:

- Qwen official text-generation and multimodal-understanding models fit best into the current `chat_completions` abstraction
- the current IR already has enough breadth through:
  - `ChatMessage.content = Value`
  - `tools`
  - `params`

Acceptance criteria:

```toml
[[spearlet.ai.backends]]
name = "qwen-chat"
provider = "qwen"
kind = "chat_http"
hosting = "remote"
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
model = "qwen3.7-plus"
credential_ref = "qwen_prod"
ops = ["chat_completions"]
features = ["supports_tools", "supports_thinking"]
transports = ["http"]
```

- the same kind also works for `provider = openai`
- Web Admin can create, edit, and render these backends

## Phase 2: preserve Qwen realtime ASR as a dedicated family

Goal: migrate the existing Qwen realtime ASR support into the new naming system without collapsing it into a shared realtime family.

Changes:

- rename `qwen_asr_realtime_ws` to:
  - `rt_asr_qwen_ws`
- keep a dedicated adapter implementation
- keep a dedicated remote-preflight path
- update live tests, samples, and Web Admin accordingly

Why dedicated:

- Qwen realtime ASR has a different websocket URL derivation, `session.update` structure, and transcript-event semantics from OpenAI
- it does not belong to the “share one adapter family” category

Acceptance criteria:

- Rust `live_caption`
- Rust `voice_chat`
- `qwen_realtime_asr_live_tests`
- `realtime_asr_live_tests`

all migrate to the new kind name and continue to pass

## Phase 3: add Qwen embeddings and rerank

Goal: make vectorization and reranking first-class operations.

Changes:

- add `Rerank` to `Operation`
- extend payloads:
  - `EmbeddingsPayload`
    - optional `dimensions`
    - optional `encoding_mode`
    - extension room for multimodal embeddings
  - add `RerankPayload`
    - `query`
    - `documents`
    - `top_n`
    - optional provider params
- introduce:
  - `embeddings_dashscope_http`
  - `rerank_dashscope_http`

Why not force this through chat:

- embeddings and rerank are separate capability families with different request and response shapes

Acceptance criteria:

- runtime adapter matrix clearly represents embeddings / rerank support
- router can choose backends correctly by operation
- Web Admin can create and display Qwen embeddings / rerank backends

## Phase 4: add Qwen TTS and image generation

Goal: cover the next most common Qwen capability families beyond chat and ASR.

Changes:

- extend `TextToSpeechPayload`
  - allow richer provider params
- extend `ImageGenerationPayload`
  - allow richer provider params
  - split later only if necessary
- introduce:
  - `text_to_speech_dashscope_http`
  - `image_generation_dashscope_http`

Note:

- the first delivery does not need to cover the full video-generation or image-editing surface
- the goal is a first-class minimal viable integration

## Phase 5: design fuller Qwen realtime support

Goal: leave a durable architecture for Qwen speech-to-speech and multimodal realtime capabilities.

Changes:

- add operations such as:
  - `realtime_multimodal`
  - or `speech_to_speech`
- add transports:
  - `webrtc`
  - `aoq`
- introduce:
  - `realtime_multimodal_qwen_ws`
  - `realtime_multimodal_qwen_webrtc`
  - `realtime_multimodal_qwen_aoq`

Important rule:

- do not continue to overload `speech_to_text` for future multimodal realtime conversations
- do not let the current ASR websocket family become the container for future voice-agent or multimodal session semantics

## 7. File-Level Change Map

### 7.1 Type and config layer

- `src/ai_backend_types.rs`
  - rewrite kind taxonomy and provider rules
- `src/spearlet/config.rs`
  - loosen provider/kind validation from one-to-one to “kind supports provider set”
- `src/sms/ai_backends/validator.rs`
  - migrate from exact provider-kind matching to allowed-provider-set validation

### 7.2 Runtime assembly layer

- `src/spearlet/ai/backend_assembly.rs`
  - remap kinds to the new adapter families
- `src/spearlet/execution/ai/backends/chat_http.rs`
  - refactor into a provider-neutral chat HTTP JSON adapter
- `src/spearlet/execution/ai/backends/rt_asr_qwen_ws.rs`
  - rename into the new kind family while keeping the dedicated implementation
- later add:
  - embeddings / rerank / TTS / image-generation adapters

### 7.3 Preflight layer

- `src/spearlet/ai/remote_preflight.rs`
  - preflight by provider family
  - stop assuming a one-to-one match between runtime kind and preflight family
- `src/spearlet/http_gateway/backend_preflight_handlers.rs`
  - update provider-family parsing and handler output

### 7.4 Web Admin layer

- `web-admin/src/features/ai-backends/AiBackendEditorForm.ts`
- `web-admin/src/features/ai-backends/AiBackendsPage.tsx`
- `web-admin/src/features/ai-backends/AiBackendDetailPage.utils.ts`
- `web-admin/src/api/ai-backends.ts`

Goals:

- keep provider and kind distinct but constrained
- let Qwen chat / embeddings / rerank / TTS / image generation be created in a first-class way
- keep `model` as a freeform string instead of hardcoded UI enums

## 8. Test Strategy

### 8.1 Unit tests

- provider/kind validation combinations
- config typed views
- preflight family detection
- runtime assembly mapping
- Web Admin form / payload / detail behavior

### 8.2 Integration tests

- `chat_http` passes under `provider = openai`
- `chat_http` passes under `provider = qwen`
- `rt_asr_qwen_ws` live tests pass
- minimal real API tests for embeddings / rerank

### 8.3 Sample verification

- Rust `live_caption`
- Rust `voice_chat`
- chat sample calling `qwen3.7-plus` under `provider = qwen`

## 9. Documentation Plan

These documents should be updated together with the implementation:

- [backend-adapter/backends-en.md](backend-adapter/backends-en.md)
- [backend-support-matrix-en.md](backend-support-matrix-en.md)
- [implementation/realtime-asr-implementation-en.md](implementation/realtime-asr-implementation-en.md)
- [web-admin-ui-guide-en.md](web-admin-ui-guide-en.md)

and the matching Chinese documents.

## 10. Recommended Execution Order

The recommended implementation order is:

1. clean up provider / kind semantics
2. unify the chat-completions family so Qwen LLM use cases work first
3. migrate Qwen realtime ASR into the new naming system
4. add rerank
5. add embeddings
6. add TTS
7. add image generation
8. finally design fuller realtime multimodal support

Why this order:

- it covers the largest number of Qwen models with the smallest architectural cost
- it does not pollute the already-working dedicated Qwen realtime ASR path
- it lets embeddings / rerank / TTS / image generation ship as clean follow-up PRs

## 11. Final Recommendation

If only one recommendation is retained, it should be this:

- **Do not introduce a provider-specific Qwen chat adapter first. Promote the current chat HTTP adapter into a provider-neutral protocol family, and let `provider = qwen` reuse it.**

That direction:

- matches the official Qwen platform reality of many models and capability families sharing related API contracts
- matches the current architecture’s `operation + transport + model` direction
- avoids growing a long-term tree of `qwen_*` LLM-specific branches
