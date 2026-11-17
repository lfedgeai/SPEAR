# Debug Server Cargo 镜像修复

## 概要
- 本地 Docker Compose 现在会让 `debug-server` 复用 `sms` 与 `spearlet` 已使用的 Cargo 镜像策略。
- builder 阶段也会在执行 `cargo build` 前先安装 `ca-certificates`。
- 这样可以避免本地执行 `docker compose up -d --build` 时，因为当前环境无法校验 `crates.io` 的 TLS 证书链而失败。

## 变更内容
- `deploy/docker/debug-server/Dockerfile`
  - 新增 `USE_CARGO_MIRROR` 与 `CARGO_MIRROR_REGISTRY` 构建参数。
  - 当 `USE_CARGO_MIRROR=1` 时，写入 `/usr/local/cargo/config.toml` 以启用镜像配置。
  - 在 Cargo 下载依赖前，于 builder 阶段安装 `ca-certificates`。
- `deploy/docker/compose.local.yaml`
  - 为 `debug-server` 构建传递 `USE_CARGO_MIRROR`，与其他本地 Rust 服务保持一致。

## 使用说明
- 本地默认行为仍然是 `USE_CARGO_MIRROR=1`。
- 如果需要直接访问官方 Cargo 源，可在执行 Compose 前设置 `USE_CARGO_MIRROR=0`。
- 这次修复只调整本地 Docker 构建流程，不改变 `debug-server` 的运行时行为。
