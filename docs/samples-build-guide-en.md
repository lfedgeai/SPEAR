# WASM Samples Build Guide

## Layout
- Source: `samples/wasm-c/hello.c`
- Source: `samples/wasm-c/chat_completion.c` (Chat Completions sample)
- Source: `samples/wasm-c/mic_rtasr.c` (realtime mic → realtime ASR)
- Output: `samples/build/hello.wasm`

## mic_rtasr prerequisites

- Host binaries must enable `mic-device` (otherwise `source=device` fails)
- Host must configure a realtime ASR backend (e.g. `openai_realtime_ws`) plus its API key
- Default backend name is `openai-realtime-asr` (override at build time via `-DSP_RTASR_BACKEND=\"...\"`)

Note: the `mic_rtasr` sample uses `server_vad` segmentation by default (silence-based).

Suggested setup:

- Spearlet config example: `config/spearlet/config.toml` (includes an `openai_realtime_ws` backend)
- Set env before running: `OPENAI_REALTIME_API_KEY`

How to run: after building `samples/build/mic_rtasr.wasm`, upload it as a WASM executable and run it as a task (see `docs/api-usage-guide-en.md` for the upload/task workflow).

## chat_completion sample

- Uses `SP_MODEL` (default `gpt-4o-mini`) as the request model
- Optional: define `SP_ROUTE_OLLAMA_GEMMA3` at build time to switch the model to `SP_OLLAMA_GEMMA3_MODEL` (default `gemma3:1b`)
- Response JSON includes `_spear.backend` (selected backend name) and `_spear.model` (request model); the sample prints `debug_backend=...`

## Build
- Run: `make samples`
- Compiler priority:
  - Prefer `zig`: `zig cc -target wasm32-wasi`
  - Fallback `clang`: requires `WASI_SYSROOT` pointing to WASI SDK sysroot

## clang usage
- Environment: `WASI_SYSROOT=/opt/wasi-sdk/share/wasi-sysroot` (adjust as needed)
- Command uses: `clang --target=wasm32-wasi --sysroot=$(WASI_SYSROOT)`
- Without SDK or sysroot, command fails; install `zig` or set `WASI_SYSROOT`

## Important changes
- Makefile retains only `samples` target
- Removed `sample-upload` and `sample-register` targets (no upload/register in build workflow)

## Runtime integration
- The generated `hello.wasm` can be uploaded via SMS file service and referenced in task registration `executable.uri`
- Spearlet WASM runtime validates module bytes during instance creation; invalid content errors out

## Sample source
```c
int main() { return 0; }
```
