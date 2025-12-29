# Backend Adapter Design Docs Index

This directory contains the Spear Backend Adapter Layer design (multi-operation / multimodal). Docs are split by layer and concern to support incremental implementation.

## Overview

- `overview-en.md`: scope, goals, terminology, and high-level architecture
- `architecture-en.md`: layered design, module boundaries, and hostcall-family evolution

## Specs

- `ir-en.md`: Canonical IR (Envelope/Response/Error/MediaRef)
- `operations-en.md`: operation payload skeletons and capability guidance

## Routing & Backends

- `routing-en.md`: capability modeling, candidate filtering, and selection policies (LB/Fallback/Hedge/Mirror)
- `backends-en.md`: Cargo feature pruning, registry/discovery, configuration model
- `streaming-en.md`: realtime/streaming subsystem (transport, lifecycle, events)

## Engineering

- `reliability-security-observability-en.md`: error model, security boundaries, observability
- `migration-mvp-en.md`: mapping from legacy Go and phased implementation plan
- `implementation-plan-en.md`: implementation plan (file/function level)
