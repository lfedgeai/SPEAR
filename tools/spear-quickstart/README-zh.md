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

执行部署（默认走 Rust 内置流程）：

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

## 支持的模式（mode）

### k8s-kind

- 适用：本机用 kind 创建集群并安装 SPEAR
- 依赖：docker daemon、kind、helm
- 说明：kubectl 不是硬依赖，但建议保留用于排障

### k8s-existing

- 适用：使用已有 Kubernetes 集群（不创建/销毁 kind）
- 依赖：helm（以及集群访问权限）

### docker-local

- 适用：纯 docker 本地运行（不需要 Kubernetes）
- 依赖：docker daemon
- 默认端口（可在 `docker_local.*` 配置中修改）：
  - SMS HTTP：`http://127.0.0.1:18080`
  - SMS Web Admin：`http://127.0.0.1:18082`（需要 `components.enable_web_admin=true`）
  - Spearlet HTTP：`http://127.0.0.1:18081`
- 常用入口：
  - SMS health：`http://127.0.0.1:18080/health`
  - SMS Swagger：`http://127.0.0.1:18080/swagger-ui/`
  - SMS Admin：`http://127.0.0.1:18082/`
- 注意：
  - 这些端口默认是纯 HTTP，没有 TLS，所以不要用 `https://` 访问。
  - 根路径 `/` 不一定有页面，看到 404 通常是正常现象；请用 `/health` 或 `/swagger-ui/` 验证服务。

## 设计细节（最近改动相关）

- **监听地址 vs 广播地址**：spearlet 在容器内监听 `0.0.0.0:50052`（bind），但向 SMS 注册时不能把 `0.0.0.0` 当作可达地址。docker-local 下通过 `SPEARLET_ADVERTISE_IP=spear-spearlet` 让 SMS 记录一个可解析/可达的地址（容器名在 user-defined bridge 网络里可 DNS 解析），避免 placement 时出现 `all candidates failed`。
- **docker-local 启动命令**：SMS/Spearlet 镜像的 entrypoint 需要子命令（`sms` / `spearlet`），否则容器会直接退出；docker-local apply 会显式传入子命令，保证容器常驻运行。
- **可写目录（容器内权限）**：docker-local 默认注入可写路径，避免 Web Admin 上传、执行日志、llama 本地模型下载因权限导致 500/写入失败：
  - SMS：`SMS_FILES_DIR=/tmp/sms-files`、`SMS_EXECUTION_LOGS_DIR=/tmp/sms-execution-logs`
  - Spearlet：`SPEARLET_LOCAL_MODELS_DIR=/tmp/spearlet-local-models`、`SPEARLET_STORAGE_DATA_DIR=/tmp/spearlet-storage`
  - 这些目录在容器重启后不保证持久化；如需持久化可考虑后续增加 bind-mount/volume 方案。
- **清理范围（Cleanup scope）**：
  - docker-local：支持清理容器（release）、网络（kind）以及镜像（images，危险操作需 `--yes` / TUI 确认）
  - k8s-kind：支持 release/secret/namespace/kind（namespace/kind 属危险操作需确认）
- **TUI 可用性**：标题栏显示面包屑导航，右侧 value 对 toggle 与普通值分别渲染，避免空 `[]` / 双层括号等视觉噪声。

## 说明

- 密钥只从环境变量读取，不会写入 config/state。
- TUI 内置快捷键：F2 保存配置、F3 Plan、F4 Apply、F6 Cleanup、F10 退出；清理范围在主菜单 `Cleanup scope / 清理范围` 配置。
- 完整帮助请执行 `./tools/spear-quickstart/target/debug/spear-quickstart -h`。
