# SPEAR Quickstart Tool (tools/spear-quickstart)

This directory contains the SPEAR quickstart tool for local and Kubernetes deployments.

## Rust version

This tool is a standalone Cargo project under `tools/spear-quickstart/`.

Build (from repo root):

```bash
cargo build --manifest-path tools/spear-quickstart/Cargo.toml
```

Create a default config:

```bash
./tools/spear-quickstart/target/debug/spear-quickstart configure --config .tmp/spear-quickstart/config.toml
```

Open menuconfig-like TUI to edit config:

```bash
./tools/spear-quickstart/target/debug/spear-quickstart tui --config .tmp/spear-quickstart/config.toml
```

Print the planned commands (dry-run):

```bash
./tools/spear-quickstart/target/debug/spear-quickstart plan --config .tmp/spear-quickstart/config.toml
```

Deploy (currently supports mode=k8s-kind only; native Rust flow by default):

```bash
export OPENAI_API_KEY=...
./tools/spear-quickstart/target/debug/spear-quickstart apply --config .tmp/spear-quickstart/config.toml --yes
```

Fallback to the legacy script (compat/debug only):

```bash
export SPEAR_QUICKSTART_USE_SCRIPT=1
./tools/spear-quickstart/target/debug/spear-quickstart apply --config .tmp/spear-quickstart/config.toml --yes
```

Cleanup:

```bash
./tools/spear-quickstart/target/debug/spear-quickstart cleanup --config .tmp/spear-quickstart/config.toml --scope release
```

## Notes

- Secrets are read from environment variables only and are never stored in config/state.
- TUI hotkeys: F2 Save, F3 Plan, F4 Apply, F6 Cleanup, F10 Exit; cleanup scope is configurable in the main menu.
- k8s-kind dependencies: docker daemon, kind, helm. kubectl is no longer a hard requirement (but recommended for debugging).
- Run `./tools/spear-quickstart/target/debug/spear-quickstart -h` for full help.
