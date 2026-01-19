# SPEAR Next

SPEAR Next 是 SPEAR 核心服务的 Rust/async 实现：

- **SMS**：元数据/控制面服务。
- **SPEARlet**：节点侧代理与运行时。

English README: [README.md](./README.md)

## 目录结构

- `src/apps/sms`：SMS 二进制入口
- `src/apps/spearlet`：SPEARlet 二进制入口
- `web-admin/`：Web Admin 前端源码
- `assets/admin/`：构建后的 Web Admin 静态资源（由 SMS 内嵌/托管）
- `samples/wasm-c/`：基于 C 的 WASM 示例（WASI）
- `docs/`：设计与使用文档

## 架构示意图

![SPEAR 架构](docs/diagrams/spear-architecture.png)

## 快速开始

### 前置依赖

- Rust toolchain（建议使用最新 stable）

说明：本项目使用 `protoc-bin-vendored`，通常无需手动安装 `protoc`。

### 构建

```bash
make build

# release
make build-release

# 指定 Rust features（例如 sled / rocksdb）
make FEATURES=sled build

# 启用本机麦克风采集实现（可选）
make FEATURES=mic-device build

# macOS 便捷入口（等价于 FEATURES+=mic-device）
make mac-build
```

### 运行 SMS

```bash
./target/debug/sms

# 启用 Web Admin
./target/debug/sms --enable-web-admin --web-admin-addr 127.0.0.1:8081
```

常用地址：

- HTTP 网关：`http://127.0.0.1:8080`
- Swagger UI：`http://127.0.0.1:8080/swagger-ui/`
- OpenAPI：`http://127.0.0.1:8080/api/openapi.json`
- gRPC：`127.0.0.1:50051`
- Web Admin（启用后）：`http://127.0.0.1:8081/admin`

### 运行 SPEARlet

当提供 `--sms-grpc-addr` 后，SPEARlet 会连接 SMS 并默认自动注册。

```bash
./target/debug/spearlet --sms-grpc-addr 127.0.0.1:50051
```

## 配置

### 配置文件路径

- SMS：`~/.sms/config.toml`（或 `--config <path>`）
- SPEARlet：`~/.spear/config.toml`（或 `--config <path>`）

仓库内示例：

- SMS：`config/sms/config.toml`
- SPEARlet：`config/spearlet/config.toml`

### 配置优先级

1. CLI `--config`
2. 家目录配置（`~/.sms/config.toml` 或 `~/.spear/config.toml`）
3. 环境变量（`SMS_*`、`SPEARLET_*`）
4. 代码默认值

### 密钥/凭证

不要把密钥写入配置文件。使用 `llm.credentials[].api_key_env` 引用环境变量。

### Ollama 模型导入

SPEARlet 支持在启动时从本机 Ollama 导入模型并生成对应的 LLM backend。

- 文档：`docs/ollama-discovery-zh.md`

## 路由与排障

- **按模型路由**：当某些 backend 配置了 `model = "..."` 时，guest 只设置 `model` 也能完成路由（无需显式指定 `backend`）。
- **如何确认最终路由到哪个 backend**：
  - `cchat_recv` 返回 JSON 顶层包含 `_spear.backend` / `_spear.model`
  - Router 在选中 backend 后会输出 `router selected backend` 的 debug 日志

## Web Admin

Web Admin 提供 Nodes/Tasks/Files/Backends 等页面。

- Backends 提供跨节点聚合视图
- 点击某个 backend 行会弹出详情窗口（Raw JSON）

文档：

- `docs/web-admin-overview-zh.md`
- `docs/web-admin-ui-guide-zh.md`

## WASM 示例

```bash
make samples
```

产物输出到 `samples/build/`（C）与 `samples/build/rust/`（Rust）。

文档：

- `docs/samples-build-guide-zh.md`

## 开发

```bash
make help
make dev
make ci
```

UI 测试（Playwright）：

```bash
make test-ui
```

## 文档索引

- `docs/INDEX.md`

## License

Apache-2.0，见 `LICENSE`。
