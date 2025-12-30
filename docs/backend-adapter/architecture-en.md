# Layered Architecture and Module Boundaries

## 1. Layer responsibilities

Split the “hostcall → backend request” path into four layers:

1) **Hostcall ABI layer (stable)**
- Decodes WASM args and memory layout
- Writes into host-side session/state (e.g., chat session) or per-request state
- Contains no backend protocol or routing logic

2) **Normalize layer (adapter core)**
- Converts host-side state into `CanonicalRequestEnvelope`
- Performs validation, defaults, semantic alignment
- Does not perform network calls

3) **Router layer (control plane)**
- Selects backend instance(s) based on `operation + requirements + policy`
- Maintains runtime signals: health, concurrency/limits, weights/priorities

4) **Backend adapters (data plane)**
- Translates Canonical IR into backend-specific schemas
- OpenAI-compatible: pass-through or light mapping
- Non-compatible: explicit translation and downgrade rules

## 2. Hostcall family evolution

When expanding from chat to image/asr/tts/embeddings/realtime, there are two common approaches:

Naming guidance:

- The hostcall family prefix should reflect the operation semantics; there is no need to force a single-letter prefix across all components.
- `cchat` is understood as “completion chat”, so keeping `cchat_*` is reasonable; for other operations use semantics-aligned prefixes.

### 2.1 Option A: per-operation hostcall families (recommended default)

- `cchat_*`: completion chat
- `emb_*`: embeddings
- `img_*`: image (generation/edit/variation, etc.)
- `asr_*`: speech-to-text (ASR)
- `tts_*`: text-to-speech (TTS)
- `rt_*`: realtime (or reuse a stream subsystem)

Pros: clear ABI, easier debugging, operation-specific argument shaping.

### 2.2 Option B: a unified `ai_call/transform` hostcall (fast iteration)

- One hostcall carries `operation + payload`, host dispatches by operation
- Closer to the legacy Go `TransformRegistry` style

Pros: fastest extension; cons: more complex ABI interpretation and debugging.

## 3. Suggested Rust module layout

Under `src/spearlet/execution`:

- `hostcall/`: WASM hostcall ABI, memory read/write, session writes
- `adapter/`: normalization (session → Canonical IR)
- `router/`: registry, capabilities, policies, health/limits
- `backends/`: backend adapters (Cargo feature gated)
- `stream/`: realtime/streaming subsystem (shares registry/capabilities with router)

## 4. Security boundaries

- URLs and API keys are host-configured; WASM can only choose backend/model names and declare requirements.
- Request-level allowlists can only restrict further; policy overrides must be constrained by host config.
