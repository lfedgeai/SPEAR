# Spear Backend Adapter Layer Overview

## 1. Background

Spear needs an adapter layer between hostcalls and model backends: normalize hostcall inputs/state into a unified Canonical IR, then route requests to “compiled and enabled” backend instances.

Chat Completions is the first capability to implement, but the design must cover multimodal/multi-operation expansion: image generation, ASR/TTS, embeddings, and real-time voice.

## 2. Goals

- Stable hostcall ABI: avoid frequent changes to the WASM/task-facing interface.
- Compile-time pruning: enable backends via Cargo features; disabled backends do not build/link.
- Capability-based routing: requests declare required/preferred capabilities; the router selects only valid candidates.
- Policy-based selection: weights, priorities, load balancing, fallback, mirroring, hedging.
- Security boundaries: secrets (API keys) and network policy are host-controlled; WASM cannot inject them.

## 3. Layering

Split the host-side path into four layers (from stable to variable):

1) Hostcall ABI layer: decode WASM memory/args and update host-side state.
2) Normalize layer: convert host state into Canonical IR (see `ir-en.md`).
3) Router layer: select backend instance(s) using capabilities and policies (see `routing-en.md`).
4) Backend adapters: translate Canonical IR into backend-specific calls (OpenAI-compatible or not) and return Canonical Responses.

## 4. Alignment with legacy Go

Two legacy patterns map directly:

- Subset matching transform selection (`legacy/spearlet/hostcalls/transform.go`) becomes capability-based routing.
- Multi-endpoint mapping with env-key filtering (`legacy/spearlet/core/models.go`) becomes registry/discovery.

The real-time ASR stream (`legacy/spearlet/stream/rt_asr.go`) indicates streaming/realtime requires dedicated transport and lifecycle modeling.

## 5. Navigation

- Canonical IR: `ir-en.md`
- Operation payload skeletons & capability guidance: `operations-en.md`
- Capability routing and selection policies: `routing-en.md`
- Backend feature gating, registry, discovery, configuration: `backends-en.md`
- Realtime/streaming: `streaming-en.md`
- Error/security/observability: `reliability-security-observability-en.md`
- Migration and MVP plan: `migration-mvp-en.md`

