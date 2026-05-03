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

Deploy (native Rust flow by default):

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

## Supported modes

### k8s-kind

- Use case: create a kind cluster locally and install SPEAR via Helm
- Dependencies: docker daemon, kind, helm
- Note: kubectl is optional but recommended for debugging

### k8s-existing

- Use case: install SPEAR into an existing Kubernetes cluster
- Dependencies: helm (and cluster access)

### docker-local

- Use case: run SPEAR locally with plain Docker (no Kubernetes)
- Dependencies: docker daemon
- Default ports (configurable via `docker_local.*`):
  - SMS HTTP: `http://127.0.0.1:18080`
  - SMS Web Admin: `http://127.0.0.1:18082` (requires `components.enable_web_admin=true`)
  - Spearlet HTTP: `http://127.0.0.1:18081`
- Useful endpoints:
  - SMS health: `http://127.0.0.1:18080/health`
  - SMS Swagger: `http://127.0.0.1:18080/swagger-ui/`
  - SMS Admin: `http://127.0.0.1:18082/`
- Notes:
  - These ports are plain HTTP (no TLS), so do not use `https://`.
  - The `/` route may return 404; use `/health` or `/swagger-ui/` to validate.

## Design notes (recent changes)

- **Bind vs advertise address**: spearlet binds to `0.0.0.0:50052` inside containers, but must not register `0.0.0.0` as a reachable address in SMS. In docker-local we set `SPEARLET_ADVERTISE_IP=spear-spearlet` so SMS stores a resolvable/reachable address (container name DNS on user-defined bridge), avoiding `all candidates failed` during placement.
- **docker-local entry commands**: SMS/Spearlet images require a subcommand (`sms` / `spearlet`) for the entrypoint. docker-local apply always passes the subcommand so containers keep running.
- **Writable directories**: docker-local injects writable paths to avoid permission-related failures (Web Admin upload 500, execution logs, llama local model download):
  - SMS: `SMS_FILES_DIR=/tmp/sms-files`, `SMS_EXECUTION_LOGS_DIR=/tmp/sms-execution-logs`
  - Spearlet: `SPEARLET_LOCAL_MODELS_DIR=/tmp/spearlet-local-models`, `SPEARLET_STORAGE_DATA_DIR=/tmp/spearlet-storage`
  - These are not persistent across container recreation; add a bind-mount/volume if you need persistence.
- **Cleanup scope**:
  - docker-local: containers (release), network (kind), images (images; destructive, requires `--yes` / TUI confirmation)
  - k8s-kind: release/secret/namespace/kind (namespace/kind are destructive and require confirmation)
- **TUI UX**: breadcrumb navigation in the title, and improved value rendering to avoid empty `[]` and double brackets.

## Notes

- Secrets are read from environment variables only and are never stored in config/state.
- TUI hotkeys: F2 Save, F3 Plan, F4 Apply, F6 Cleanup, F10 Exit; cleanup scope is configurable in the main menu.
- Run `./tools/spear-quickstart/target/debug/spear-quickstart -h` for full help.
