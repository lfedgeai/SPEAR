# Scripts

This directory contains common helper scripts used in this repo for local development, coverage, and end-to-end (E2E) validation.

Chinese README: [README.md](./README.md)

## E2E / Integration

### e2e.sh

- Purpose: E2E entrypoint. Selects the kind / docker suite based on the host OS and `E2E_SUITES`.
- Typical usage: `make e2e`
- Environment variables:
  - `E2E_SUITES=kind|docker|kind,docker` (default: `auto`)
  - `E2E_LINUX=1` (on non-Linux, run Linux binaries + Docker via `make e2e-linux`)
- Called by: Makefile `e2e` target.

### e2e-kind.sh

- Purpose: Run E2E on a local kind cluster (build images, load into kind, helm install, health checks, smoke validations).
- Typical usage: `make e2e` (macOS defaults to kind) or `make e2e-kind`
- Called by: `e2e.sh` or Makefile `e2e-kind` target.

### e2e-docker.sh

- Purpose: Run `testcontainers_e2e` with Docker on Linux (non-Linux skips by default; use `E2E_LINUX=1` via `make e2e-linux`).
- Typical usage: `make e2e-docker`
- Called by: `e2e.sh` or Makefile `e2e-docker` target.

### kind-openai-quickstart.sh

- Purpose: Create a kind test cluster and deploy the Helm chart. Optionally inject local `OPENAI_API_KEY` as a Kubernetes Secret for quick OpenAI backend validation.
- Typical usage:
  - `OPENAI_API_KEY=... ./scripts/kind-openai-quickstart.sh`
  - or export `OPENAI_API_KEY` in your shell and run the script
- Dependencies: docker, kind, kubectl, helm
- Values file: `deploy/helm/spear/values-openai.yaml` (does not contain the plaintext key; it only references the Secret)
- Default behavior: keep the cluster (`KEEP_CLUSTER=1`) and write kubeconfig into `.tmp/` under the repo to avoid polluting your global `KUBECONFIG`.

## Coverage

### coverage.sh

- Purpose: Generate coverage reports via `cargo-tarpaulin` (HTML/LCOV/JSON).
- Typical usage: `make coverage`
- Note: The script may attempt to install `cargo-tarpaulin` if it is missing.

### quick-coverage.sh

- Purpose: Faster coverage run (shorter timeout, less output).
- Typical usage: `make quick-coverage`
- Note: The script may attempt to install `cargo-tarpaulin` if it is missing.

