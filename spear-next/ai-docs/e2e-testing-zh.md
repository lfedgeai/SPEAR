# 使用 Testcontainers 的端到端（E2E）测试（SMS ↔ SPEARlet）

## 概述
本文档说明如何在 Docker 中使用 Testcontainers 启动 SMS 和 SPEARlet，验证它们之间的 gRPC 连接与注册行为。这是跨进程、跨网络的系统级集成测试。

## 测试类型
- 集成测试（跨模块）
- 端到端（E2E）系统测试（跨进程、跨网络）

## 前置条件
- 已安装并运行 Docker
- 项目可编译生成 `sms` 与 `spearlet` 二进制（`cargo build`）

## 工作原理
- 测试通过 `testcontainers` 运行 `debian:bookworm-slim` 容器
- 将宿主机构建的二进制（`target/debug/sms`、`target/debug/spearlet`）挂载到容器
- 启动 SMS，等待日志包含 “SMS gRPC server listening”
- 启动 SPEARlet，使用 `--sms-addr host.testcontainers.internal:<映射端口> --auto-register` 连接并注册
- 断言日志包含 “Connected to SMS successfully” 与 “Successfully registered with SMS”

## 运行测试
该 E2E 测试默认使用 `#[ignore]` 标记，需要显式启用：

```bash
cargo build
DOCKER=1 cargo test --test testcontainers_e2e -- --ignored --nocapture
```

## 文件位置
- 测试代码：`tests/e2e/testcontainers_e2e.rs`
- 开发依赖：`Cargo.toml` → `testcontainers = "0.15"`

## 最佳实践
- 将基于 Docker 的测试标记为 `#[ignore]`，避免默认 CI 测试运行
- 生产 CI 建议先构建并发布镜像，测试直接拉取镜像，提升一致性与可复用性
- 使用 `WaitFor` 避免容器启动竞态
- 在容器内通过 `host.testcontainers.internal` 访问宿主机映射端口，实现容器间通信

## 故障排查
- 若本机没有 Docker，测试会自动跳过
- 若日志不含预期信息，可以增加等待时间或检查容器环境配置

