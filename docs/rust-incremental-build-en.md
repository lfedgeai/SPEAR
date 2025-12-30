# Rust Incremental Build

Incremental compilation is enabled for this repository via Cargo configuration: `.cargo/config.toml`.

## Goal
- Speed up local edit/compile cycles (especially `cargo test` / `cargo build`).

## Notes
- Incremental artifacts are stored under `target/*/incremental/`, which increases disk usage.
- To temporarily disable it, set: `CARGO_INCREMENTAL=0`.

