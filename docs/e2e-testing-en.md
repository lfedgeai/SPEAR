# E2E Testing with Testcontainers (SMS ↔ SPEARlet)

## Overview
This document explains the end-to-end (E2E) integration tests that run SMS and SPEARlet in Docker using Testcontainers. The tests verify real gRPC connectivity and registration in a controlled container network.

## Test Type
- Integration Test (cross-module)
- End-to-End (E2E) System Test (cross-process, cross-network)

## Prerequisites
- Docker installed and running
- Project builds `sms` and `spearlet` binaries (`cargo build`)

## How It Works
- The test uses `testcontainers` to run `debian:bookworm-slim` containers
- Mounts host-built binaries (`target/debug/sms`, `target/debug/spearlet`) into containers
- Starts SMS, waits for “SMS gRPC server listening”
- Starts SPEARlet with `--sms-grpc-addr host.testcontainers.internal:<mapped_port> --auto-register`
- Waits for “Connected to SMS successfully” and “Successfully registered with SMS”

## Run the Test
The E2E test is marked `#[ignore]` by default. Enable explicitly:

```bash
cargo build
DOCKER=1 cargo test --test testcontainers_e2e -- --ignored --nocapture
```

## File Locations
- Test: `tests/e2e/testcontainers_e2e.rs`
- Dev dependency: `Cargo.toml` → `testcontainers = "0.15"`

## Best Practices
- Mark Docker-based tests `#[ignore]` to avoid running in default CI
- Prefer published images for repeatability in CI; current approach mounts host binaries for speed
- Use `WaitFor` to avoid race conditions during container startup
- Use `host.testcontainers.internal` to reach SMS via mapped host port from inside SPEARlet container

## Troubleshooting
- If Docker is not available, test skips automatically
- If logs do not contain expected messages, increase wait duration or verify container environment
