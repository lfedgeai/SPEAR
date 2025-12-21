# WASM Samples Build Guide

## Layout
- Source: `samples/wasm-c/hello.c`
- Output: `samples/build/hello.wasm`

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
