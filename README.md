# SPEAR-Next

下一代 SPEAR 组件（Rust / async），包含元数据服务 SMS 与节点代理 SPEARlet。

Next-generation SPEAR components (Rust / async), including the SMS metadata server and the SPEARlet node agent.

## 组件 / Components

- **SMS (SPEAR Metadata Server)**：提供节点/任务/文件的注册与管理能力，并同时暴露 gRPC 与 HTTP API。
- **SMS (SPEAR Metadata Server)**: manages nodes/tasks/files and exposes both gRPC and HTTP APIs.
- **SPEARlet**：运行在节点侧的核心代理，负责与 SMS 交互、订阅任务事件，并执行运行时相关逻辑（包含 WASM 运行时能力）。
- **SPEARlet**: the node-side agent that talks to SMS, subscribes to task events, and executes runtime logic (including WASM runtime support).

## 快速开始 / Quick Start

### 0) 前置依赖 / Prerequisites

- Rust（建议使用最新 stable）
- Rust toolchain (latest stable recommended)

说明：本项目使用 `protoc-bin-vendored`，通常无需手动安装 `protoc`。

Note: this repo uses `protoc-bin-vendored`, so you typically don’t need to install `protoc` manually.

### 1) 构建 / Build

```bash
make build

# 发布构建 / release build
make build-release

# 指定 Rust features（例如 sled / rocksdb） / build with Rust features (e.g. sled / rocksdb)
make FEATURES=sled build

# 启用本机麦克风采集（mic_fd device source）/ enable local microphone capture (mic_fd device source)
make FEATURES=mic-device build

# macOS 便捷入口（等价于 FEATURES+=mic-device）/ macOS shortcut (equivalent to FEATURES+=mic-device)
make mac-build
```

说明 / Notes:

- `mic-device` 默认不启用；不启用时不会编译本机麦克风采集实现（仅保留 mic hostcall 的框架与 stub/fallback 逻辑）。
- `mic-device` is disabled by default; without it, the real device capture implementation is not compiled (only the mic hostcall + stub/fallback path remains).
- 详情 / More details: `docs/mic-device-feature-zh.md`, `docs/mic-device-feature-en.md`

### 2) 运行 SMS / Run SMS

```bash
# 默认启用 HTTP 网关与 Swagger（见下方端口）
# HTTP gateway and Swagger are enabled by default (see ports below)

./target/debug/sms

# 或启用 Web Admin / or enable Web Admin
./target/debug/sms --enable-web-admin --web-admin-addr 127.0.0.1:8081

# 查看完整参数 / show all options
./target/debug/sms --help
```

常用地址 / Useful endpoints:

- HTTP Gateway: `http://127.0.0.1:8080`
- Swagger UI: `http://127.0.0.1:8080/swagger-ui/`
- OpenAPI Spec: `http://127.0.0.1:8080/api/openapi.json`
- gRPC: `127.0.0.1:50051`
- Web Admin（启用后 / when enabled）: `http://127.0.0.1:8081/admin`

### 3) 运行 SPEARlet / Run SPEARlet

SPEARlet 默认不会主动连接 SMS；当你通过 CLI 或环境变量提供 `sms-grpc-addr` 时，会触发连接并（默认）启用自动注册。

SPEARlet does not connect to SMS by default; once you provide `sms-grpc-addr` via CLI or env, it will connect and (by default) auto-register.

```bash
./target/debug/spearlet --sms-grpc-addr 127.0.0.1:50051

# 查看完整参数 / show all options
./target/debug/spearlet --help
```

## 配置 / Configuration

### 配置文件路径 / Config file locations

- SMS：`~/.sms/config.toml`（或通过 `--config <path>` 指定）
- SMS: `~/.sms/config.toml` (or pass `--config <path>`)
- SPEARlet：`~/.spear/config.toml`（或通过 `--config <path>` 指定）
- SPEARlet: `~/.spear/config.toml` (or pass `--config <path>`)

仓库内提供可直接参考的配置示例 / Repo-shipped config examples:

- SMS: `config/sms/config.toml`（以及 `config-sled.toml` / `config-rocksdb.toml`）
- SPEARlet: `config/spearlet/config.toml`

### 配置优先级 / Priority

1. CLI `--config` 指定的文件（最高） / CLI `--config` file (highest)
2. 家目录配置（`~/.sms/config.toml` 或 `~/.spear/config.toml`） / Home config
3. 环境变量（如 `SMS_*`、`SPEARLET_*`） / Environment variables
4. 代码内置默认值 / Built-in defaults

### 日志 / Logging

日志可通过配置或环境变量指定输出文件，例如示例配置中 SPEARlet 使用：`file = "./logs/spearlet.log"`。

Logs can be configured to write to a file; for example, the sample SPEARlet config uses: `file = "./logs/spearlet.log"`.

### LLM 凭证（环境变量注入）/ LLM credentials (env injection)

不要把密钥写入配置文件或提交到仓库；使用 `api_key_env` 引用环境变量注入。

Do not put secrets into config files or commit them; use `api_key_env` to reference an environment variable.

## 文档 / Documentation

- 文档索引 / Docs index：`docs/INDEX.md`
- API 使用指南 / API usage guide：`docs/api-usage-guide-zh.md`, `docs/api-usage-guide-en.md`
- WASM 运行时 / WASM runtime：`docs/wasm-runtime-usage-zh.md`, `docs/wasm-runtime-usage-en.md`
- E2E 测试 / E2E testing：`docs/e2e-testing-zh.md`, `docs/e2e-testing-en.md`
- UI 测试 / UI tests：`docs/ui-tests-guide-zh.md`, `docs/ui-tests-guide-en.md`
- WASM 示例构建 / WASM samples build：`docs/samples-build-guide-zh.md`, `docs/samples-build-guide-en.md`

## 开发与测试 / Development & Testing

```bash
# 常用入口 / common entrypoints
make help

# 开发工作流（格式化 + lint + 测试） / dev workflow (format + lint + tests)
make dev

# 完整 CI 流水线 / full CI pipeline
make ci
```

UI 测试（Playwright）/ UI tests (Playwright):

```bash
make test-ui
```

mic-device 采集验证 / mic-device capture validation:

```bash
# 只跑 mic-device 采集测试（会输出默认输入设备与读帧信息）
SPEAR_TEST_REQUIRE_MIC_DEVICE=1 make test-mic-device

# 或运行探测示例（列出输入设备并读一帧）
cargo run --features mic-device --example mic_device_probe
```

WASM 示例（C → WASM）/ WASM samples (C → WASM):

```bash
make samples
```

端到端测试（Docker/Testcontainers）/ E2E tests (Docker/Testcontainers):

```bash
make e2e

# 非 Linux 主机上建议用 Linux 目标二进制 / on non-Linux hosts, build Linux binaries
make e2e-linux
```

## 贡献 / Contributing

欢迎提交 Issue/PR。

Issues and PRs are welcome.

建议在提交前本地运行 `make test`（以及必要时的 UI/E2E 测试）。

Please run `make test` locally before submitting (and UI/E2E tests when relevant).

## License

Apache License 2.0. See `LICENSE`.

Apache License 2.0，见 `LICENSE`。
