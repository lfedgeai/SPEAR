# SPEAR Quickstart 工具（tools/spear-quickstart）

本目录包含 SPEAR quickstart 工具，用于本地与 Kubernetes 环境的快速部署/清理。

## Rust 版本

该工具是 `tools/spear-quickstart/` 下的独立 Cargo 项目。

构建（在 repo 根目录执行）：

```bash
cargo build --manifest-path tools/spear-quickstart/Cargo.toml
```

生成默认配置：

```bash
./tools/spear-quickstart/target/debug/spear-quickstart configure --config .tmp/spear-quickstart/config.toml
```

打开 menuconfig 风格 TUI 编辑配置：

```bash
./tools/spear-quickstart/target/debug/spear-quickstart tui --config .tmp/spear-quickstart/config.toml
```

打印计划命令（dry-run，不改环境）：

```bash
./tools/spear-quickstart/target/debug/spear-quickstart plan --config .tmp/spear-quickstart/config.toml
```

执行部署（当前仅支持 mode=k8s-kind，默认走 Rust 内置流程）：

```bash
export OPENAI_API_KEY=...
./tools/spear-quickstart/target/debug/spear-quickstart apply --config .tmp/spear-quickstart/config.toml --yes
```

如需回退到旧脚本（仅用于兼容/排障）：

```bash
export SPEAR_QUICKSTART_USE_SCRIPT=1
./tools/spear-quickstart/target/debug/spear-quickstart apply --config .tmp/spear-quickstart/config.toml --yes
```

执行清理：

```bash
./tools/spear-quickstart/target/debug/spear-quickstart cleanup --config .tmp/spear-quickstart/config.toml --scope release
```

## 说明

- 密钥只从环境变量读取，不会写入 config/state。
- TUI 内置快捷键：F2 保存配置、F3 Plan、F4 Apply、F6 Cleanup、F10 退出；清理范围在主菜单 `Cleanup scope / 清理范围` 配置。
- k8s-kind 模式依赖：docker daemon、kind、helm；kubectl 不再是硬依赖（但建议保留用于排障）。
- 完整帮助请执行 `./tools/spear-quickstart/target/debug/spear-quickstart -h`。
