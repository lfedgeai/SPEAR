# SPEAR Quickstart Tool Design Doc (Draft)

This document proposes an “extended quickstart tool” to replace/augment the current script and provide a Linux `menuconfig`-like TUI workflow: edit a config file first, then run deployment or cleanup.

## 0. Current implementation status (important)

The repository now ships a Rust quickstart tool at `tools/spear-quickstart/` (a standalone Cargo project) and the earlier Python implementation has been removed. Compared to the “target capabilities” in this doc, the current state is roughly:

- CLI subcommands: `configure / tui / plan / apply / status / cleanup`
- `plan`: implemented (prints a phase-oriented plan for `mode=k8s-kind`, plus a legacy-script fallback note)
- TUI: menuconfig-like editor + function keys wired (`Plan/Apply/Status/Cleanup`), with explicit confirm/result dialogs
- `apply`: implemented for `mode=k8s-kind` and `mode=docker-local` (MVP); native Rust orchestration (docker/kind/helm + Kubernetes API + Docker API). `k8s-existing` not implemented
- `state`: not implemented yet; cleanup is scope-driven (not state-driven)
- Helm/values/secret: plumbed into `mode=k8s-kind` apply (namespace/release/values, logging overrides, images, optional OpenAI secret from env)
-  - docker-local (MVP): start `sms/spearlet` via Docker network + docker run; publish HTTP ports from config; OpenAI key is injected from env only; optionally enable SMS built-in Router Filter (reserved for future plugin extensions)

References (current state):
- Legacy script (fallback): `../scripts/kind-openai-quickstart.sh`
- Helm OpenAI values: `../deploy/helm/spear/values-openai.yaml`

## 1. Background and Current Behavior

Today, `../scripts/kind-openai-quickstart.sh` essentially does:
- Dependency checks: `docker/kind/kubectl/helm`
- Kind cluster lifecycle: create/delete/reuse
- Image builds: SMS / SPEARlet (optional SMS built-in Router Filter)
- Kind image loading: load local images into kind nodes
- Helm install/upgrade: `helm upgrade --install` + values file + a few `--set` overrides
- Readiness waits: rollout/wait ready
- OpenAI key flow: if local `OPENAI_API_KEY` is set, create `openai-api-key` Secret; Helm values inject it into SPEARlet env

The Rust quickstart tool now implements the same high-level flow directly for `mode=k8s-kind`, and keeps the script as an optional fallback for compatibility/debugging.

It’s great for a one-off local validation, but becomes harder to maintain and reproduce as options and target environments grow.

## 2. Goals / Non-goals

### Goals
- Single source of truth: a config file captures build/cache/deploy target/component toggles/logging/timeouts.
- menuconfig-like TUI: interactive editing, then save and apply.
- Three mutually exclusive modes:
  - `k8s-kind`: create/reuse a kind cluster + deploy via Helm
  - `k8s-existing`: use an existing Kubernetes cluster (kubeconfig/context) + deploy via Helm
  - `docker-local`: run components via Docker directly (no Kubernetes required)
- Cleanup support: remove previously deployed resources safely and deterministically.
- Non-interactive usage: `apply --config xxx --yes` for CI/scripts.

### Non-goals (v1)
- No complex profile inheritance system (start with default + user config + CLI override).
- No production image publishing pipeline by default (registry login/push), but keep extension points.
- Never store secrets in config or state (read from env at runtime only).

## 3. Recommended Implementation (Best Practice)

### Recommendation: single-directory Rust tool + ratatui/crossterm TUI + TOML config
- TUI: `ratatui + crossterm` (Rust ecosystem, no ncurses dependency)
- Config: TOML (`toml` + `serde`)
- Maintainability: modular validate/plan/apply/cleanup/state, avoiding Bash script sprawl

If Bash is required, `whiptail/dialog` is an option but adds dependencies and tends to be less portable on macOS.

## 4. Directory Layout (Standalone Tool Directory)

Proposed directory (example):
- `tools/spear-quickstart/`
  - `Cargo.toml`
  - `src/`
    - `main.rs` (entry; subcommands: configure/tui/plan/apply/status/cleanup)
    - `config.rs` (schema/default/merge/validate)
    - `tui.rs` (menuconfig-like editor)
    - `deploy.rs` (orchestration for docker/kind/helm + Kubernetes/Docker APIs)
    - `state.rs` (persist/load state; to be implemented)
    - `modes/` (optional split: k8s_kind/k8s_existing/docker_local)

Compatibility: keep `../scripts/kind-openai-quickstart.sh`; later it can become a wrapper that invokes the new tool.

## 5. CLI Shape (Subcommands)

Examples:
- `./tools/spear-quickstart/target/debug/spear-quickstart configure --config .tmp/spear-quickstart/config.toml`
- `./tools/spear-quickstart/target/debug/spear-quickstart plan --config .tmp/spear-quickstart/config.toml`
- `./tools/spear-quickstart/target/debug/spear-quickstart apply --config .tmp/spear-quickstart/config.toml --yes`
- `./tools/spear-quickstart/target/debug/spear-quickstart status --config .tmp/spear-quickstart/config.toml`
- `./tools/spear-quickstart/target/debug/spear-quickstart cleanup --config .tmp/spear-quickstart/config.toml --scope release,namespace,kind --yes`

Behavior:
- `configure`: edit config only, no environment changes.
- `plan`: print steps/commands/targets for review.
- `apply`: deploy; on success write state for safe cleanup.
- `cleanup`: state-driven cleanup; without state, do minimal safe cleanup (e.g., Helm uninstall only).

## 6. Config Spec (TOML, v1)

Principles: mutually exclusive modes, safe defaults, no secrets persisted.

```toml
version = 1

[mode]
# one of: "k8s-kind" | "k8s-existing" | "docker-local"
name = "k8s-kind"

[paths]
workdir = "."
state_dir = ".tmp/spear-quickstart"

[build]
enabled = true
pull_base = true
no_cache = false
debian_suite = "trixie"

[images]
tag = "local"
sms_repo = "spear-sms"
spearlet_repo = "spear-spearlet"

[components]
enable_web_admin = true
enable_router_filter = true
enable_e2e = false
spearlet_with_node = true
spearlet_with_llama_server = true

[logging]
debug = true
log_level = "info"
log_format = "json"

[timeouts]
rollout = "300s"

[k8s]
namespace = "spear"
release_name = "spear"
chart_path = "deploy/helm/spear"
values_files = ["deploy/helm/spear/values-openai.yaml"]

[k8s.kind]
cluster_name = "spear-openai"
reuse_cluster = false
keep_cluster = true
kubeconfig_file = ".tmp/kubeconfig-kind-spear-openai"

[k8s.existing]
# when mode.name = "k8s-existing"
kubeconfig = ""      # optional
context = ""         # optional, else current context

[secrets.openai]
# one of: "skip" | "from-env"
source = "from-env"
env_name = "OPENAI_API_KEY"
k8s_secret_name = "openai-api-key"
k8s_secret_key = "OPENAI_API_KEY"

[docker_local]
# when mode.name = "docker-local"
network_name = "spear-quickstart"
sms_name = "spear-sms"
spearlet_name = "spear-spearlet"
publish_sms_http = "18080:8080"
publish_spearlet_http = "18081:8081"
```

Notes:
- `mode.name` selects the execution path; non-applicable sections should be rejected or ignored with clear validation messages.
- `k8s.values_files` is an array for future extension.
- `secrets.openai` supports env injection only; never store API keys in config.

## 7. TUI (menuconfig-like) UX

Suggested main menu:
1) Mode: `k8s-kind / k8s-existing / docker-local`
2) Build: enable, pull_base, no_cache, debian_suite, tag/repo
3) Components: web admin / router filter / e2e / spearlet targets
4) K8s/Helm (k8s-only): namespace, release, values files, timeouts, logging
5) Kind (k8s-kind only): cluster name, reuse/keep, kubeconfig path
6) Existing Cluster (k8s-existing only): kubeconfig/context
7) Secrets: show OPENAI_API_KEY presence (no value echo)
8) Plan & Apply: show plan; confirm then apply
9) Cleanup: cleanup submenu (scope selection + double confirmation)
10) Save & Exit

Best practices:
- Safe default: when a kind cluster exists, prompt “Reuse / Recreate” and default to Reuse.
- Destructive cleanup requires double confirmation unless `--yes`.
- Lightweight validation when leaving pages (paths, values files, chart path).

## 8. Apply Workflow

Split into 5 phases and show them in `plan` output.

### Phase A: Validate
- Check required tools per mode: docker daemon, kind, helm (+ kubeconfig reachability via Kubernetes API)
- Validate config: paths, mutually exclusive mode, field constraints

### Phase B: Prepare
- Create `state_dir`
- Initialize/load state; if already deployed, allow upgrade or require cleanup (controlled by TUI/CLI)

### Phase C: Build Images (optional)
- Map build options to docker build flags (aligned with current script: `--pull`, `--no-cache`)
- Persist image list to state (for kind load / docker-local)

### Phase D: Deploy (mode-specific)

#### k8s-kind
- Create/reuse kind cluster
- Export kubeconfig to a file
- Kind load docker images (when needed)
- Create namespace if missing
- Secrets: if `from-env` and env exists, create/update Secret; else skip
- Helm upgrade/install (values_files + required overrides)
- Rollout/wait ready (configurable timeout)

#### k8s-existing
- Use kubeconfig/context without modifying the user’s default kubeconfig
- Remaining steps similar to k8s-kind (no kind operations)

#### docker-local
- Create docker network if missing
- Run sms and spearlet containers (based on `docker_local.*` network/names/port mappings)
- Inject OpenAI key via env only (no persistence)
- Minimal health/connectivity checks

### Phase E: Post
- Print next-step instructions (kubectl get pods, port-forward, docker logs)
- Write state for status/cleanup

## 9. State File and Cleanup (Idempotent and Safe)

Core rule: cleanup must be state-driven, not guess-driven.

Suggested state fields:
- mode, config_path, apply_time
- k8s: namespace, release, kubeconfig/context, whether namespace/secret were created
- kind: cluster_name, whether created/reused, kubeconfig_file
- docker-local: network, container names/ids, ports
- images: built image list

Cleanup policy:
- Read state and cleanup in reverse order: Helm uninstall → optional secret/namespace → optional kind delete → docker containers/network.
- Support scope: `--scope release,namespace,kind,images,containers,network`
- Protection:
  - Don’t delete clusters not created by the tool unless explicitly confirmed
  - Don’t delete namespace by default

## 10. Security and Secret Handling
- Never store `OPENAI_API_KEY` in config or state.
- Supported modes:
  - `from-env`: read at runtime; create k8s Secret in k8s modes; inject env in docker-local
  - `skip`: user manages secrets manually
- TUI shows “present/missing” only, never the secret value.

## 11. Acceptance Criteria (for Review)
- Reproducibility: same config yields the same deployment; non-interactive works.
- Usability: TUI covers key fields; mode switching behaves correctly.
- Idempotency: repeated apply/cleanup works predictably; cleanup defaults are safe.
- No secret persistence: searching config/state never reveals real keys.
