# Rust Incremental Build 配置

本项目已通过 Cargo 配置开启 incremental build：`.cargo/config.toml`。

## 目的
- 加速本地反复编译（尤其是 `cargo test` / `cargo build` 的增量迭代）。

## 说明
- incremental build 的产物会写入 `target/*/incremental/`，会增加一定磁盘占用。
- 若你希望临时关闭，可在命令前设置环境变量：`CARGO_INCREMENTAL=0`。

