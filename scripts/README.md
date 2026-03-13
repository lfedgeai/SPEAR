# Scripts 说明

本目录包含仓库内常用的脚本工具，用于本地开发、覆盖率、以及端到端（E2E）验证。

English README: [README.en.md](./README.en.md)

## E2E / 集成验证

### e2e.sh

- 用途：E2E 总入口，根据宿主 OS 与 `E2E_SUITES` 选择执行 kind / docker 套件。
- 典型用法：`make e2e`
- 相关环境变量：
  - `E2E_SUITES=kind|docker|kind,docker`（默认 `auto`）
  - `E2E_LINUX=1`（在非 Linux 上通过 `make e2e-linux` 走 Linux 二进制 + Docker）
- 调用关系：被 Makefile 的 `e2e` target 调用。

### e2e-kind.sh

- 用途：使用 kind + Helm 在本机创建临时集群并跑 E2E 校验（包含构建镜像、load 到 kind、helm install、健康检查与简单验证）。
- 典型用法：`make e2e`（macOS 默认走 kind 套件）或 `make e2e-kind`
- 调用关系：被 `e2e.sh` 或 Makefile 的 `e2e-kind` target 调用。

### e2e-docker.sh

- 用途：在 Linux 上使用 Docker 运行 `testcontainers_e2e`（非 Linux 默认跳过；可通过 `E2E_LINUX=1` 走 `make e2e-linux`）。
- 典型用法：`make e2e-docker`
- 调用关系：被 `e2e.sh` 或 Makefile 的 `e2e-docker` target 调用。

### kind-openai-quickstart.sh

- 用途：一键创建 kind 测试集群并部署 Helm chart，同时可选把本机的 `OPENAI_API_KEY` 注入为 K8s Secret，方便快速验证 OpenAI backend。
- 典型用法：
  - `OPENAI_API_KEY=... ./scripts/kind-openai-quickstart.sh`
  - 或先在本机配置 `OPENAI_API_KEY`，然后直接运行脚本
- 依赖：docker、kind、kubectl、helm
- 配套 values：`deploy/helm/spear/values-openai.yaml`（不含明文 key，仅引用 Secret）
- 默认行为：脚本默认保留集群（`KEEP_CLUSTER=1`），并把 kubeconfig 写到仓库目录下的 `.tmp/`，避免污染你的全局 `KUBECONFIG`。

## 覆盖率

### coverage.sh

- 用途：使用 `cargo-tarpaulin` 生成覆盖率报告（HTML/LCOV/JSON）。
- 典型用法：`make coverage`
- 说明：脚本会在缺少 `cargo-tarpaulin` 时尝试安装。

### quick-coverage.sh

- 用途：更快的覆盖率运行（较短超时、更少输出）。
- 典型用法：`make quick-coverage`
- 说明：脚本会在缺少 `cargo-tarpaulin` 时尝试安装。
