# Debug Server Cargo Mirror Fix

## Summary
- The local Docker Compose stack now builds `debug-server` with the same Cargo mirror strategy used by `sms` and `spearlet`.
- The builder stage also installs `ca-certificates` before running `cargo build`.
- This prevents local `docker compose up -d --build` from failing when the environment cannot validate the TLS chain used by `crates.io`.

## What Changed
- `deploy/docker/debug-server/Dockerfile`
  - Adds `USE_CARGO_MIRROR` and `CARGO_MIRROR_REGISTRY` build arguments.
  - Writes `/usr/local/cargo/config.toml` with the mirror configuration when `USE_CARGO_MIRROR=1`.
  - Installs `ca-certificates` in the builder stage before Cargo downloads dependencies.
- `deploy/docker/compose.local.yaml`
  - Passes `USE_CARGO_MIRROR` to the `debug-server` image build for consistency with the other local Rust services.

## Operational Notes
- The default local behavior keeps `USE_CARGO_MIRROR=1`.
- To opt out and use upstream Cargo directly, set `USE_CARGO_MIRROR=0` before running Compose.
- The fix is intentionally limited to the local Docker workflow and does not change runtime behavior of `debug-server`.
