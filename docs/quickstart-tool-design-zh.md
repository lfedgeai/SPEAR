# SPEAR Quickstart 工具设计文档（草案）

本文档描述一个“扩展版 quickstart 工具”的设计，用于替代/增强现有脚本，并提供类似 Linux `menuconfig` 的 TUI（文本界面）配置体验：先编辑配置文件，再执行部署或清理。

## 0. 当前实现状态（重要）

目前仓库里已落地一个 Rust 版本的 quickstart 工具，位置为 `tools/spear-quickstart/`（独立 Cargo 项目），并且已删除早期 Python 实现。与本文档的“目标能力”相比，当前实现状态大致如下：

- CLI 子命令：已支持 `configure / tui / plan / apply / status / cleanup`
- `plan`：已实现（对 `mode=k8s-kind` 输出分阶段 plan，并提示 legacy 脚本 fallback）
- TUI：menuconfig 风格编辑器 + 功能键已贯通（`Plan/Apply/Status/Cleanup`），并提供显式确认/结果弹窗
- `apply`：已实现 `mode=k8s-kind` 与 `mode=docker-local`（MVP）；默认走 Rust 内置编排（docker/kind/helm + Kubernetes API + Docker API）；`k8s-existing` 尚未实现
- `state`：尚未实现 state 落盘与基于 state 的安全 cleanup（目前 cleanup 主要按 scope 驱动，而非 state 驱动）
- Helm/values/secret：已贯通到 `mode=k8s-kind` apply（namespace/release/values、日志覆盖、镜像配置、可选从 env 创建 OpenAI Secret）
-  - docker-local（MVP）：通过 Docker network + docker run 拉起 `sms/spearlet`，按配置映射 HTTP 端口；OpenAI key 仅从 env 注入容器 env；可选启用 SMS 内置 Router Filter（预留插件扩展点）

相关现状参考：
- legacy 脚本（fallback）：`../scripts/kind-openai-quickstart.sh`
- Helm OpenAI values：`../deploy/helm/spear/values-openai.yaml`

## 1. 背景与现状

当前 `../scripts/kind-openai-quickstart.sh` 的核心流程是：
- 依赖检查：`docker/kind/kubectl/helm`
- kind 集群：创建/删除/复用
- 镜像构建：SMS / SPEARlet（可选启用 SMS 内置 Router Filter）
- kind load：把本机镜像加载到 kind 节点
- Helm 安装：`helm upgrade --install` + values 文件 + 部分 `--set` 覆盖
- 等待就绪：rollout/wait ready
- OpenAI Key：若本机设置 `OPENAI_API_KEY`，则创建 `openai-api-key` Secret；由 Helm values 把 key 注入 SPEARlet 环境变量

当前 Rust quickstart 工具已在 `mode=k8s-kind` 下实现同等的高层流程，并保留脚本作为兼容/排障用的可选 fallback。

该脚本通过环境变量控制行为，适合“单次快速验证”，但随着选项增多会逐渐难以维护和复现（尤其是跨不同部署目标时）。

## 2. 目标 / 非目标

### 目标
- 单一配置源：以 config 文件描述所有选项（构建、缓存、部署目标、组件开关、日志与超时等）。
- menuconfig 风格 TUI：交互式编辑配置；保存后可直接执行 apply。
- 支持三种互斥执行模式：
  - `k8s-kind`：创建/复用 kind 集群 + Helm 部署
  - `k8s-existing`：连接现有 Kubernetes 集群（kubeconfig/context）+ Helm 部署
  - `docker-local`：不依赖 Kubernetes，直接以 Docker 运行组件容器
- 支持 cleanup：可清理现有部署资源，且默认安全，避免误删用户环境。
- 支持非交互：`apply --config xxx --yes` 直接执行，适配 CI/脚本调用。

### 非目标（第一版不做）
- 不做复杂的 profile/继承系统（先采用 default + user config + CLI override 三层）。
- 不做生产级镜像发布（registry 登录、push 等）作为默认流程（预留扩展点即可）。
- 不把任何密钥写进 config 或 state（只在运行时从环境变量读取）。

## 3. 推荐实现形态（业界最佳实践）

### 推荐：Rust 单目录工具 + ratatui/crossterm TUI + TOML 配置
- TUI：`ratatui + crossterm`（纯 Rust 生态，不依赖 ncurses）
- 配置：TOML（易读，Rust 侧用 `toml` + `serde`）
- 结构化：把 validate/plan/apply/cleanup/state 模块化，避免 Bash 脚本不可控膨胀

说明：如果强制继续 Bash，也可用 `whiptail/dialog` 实现 TUI，但会引入额外依赖且在 macOS 上可移植性较差。

## 4. 目录组织（单独目录）

建议新增目录（示例）：
- `tools/spear-quickstart/`
  - `Cargo.toml`
  - `src/`
    - `main.rs`（入口；子命令：configure/tui/plan/apply/status/cleanup）
    - `config.rs`（schema/default/merge/validate）
    - `tui.rs`（menuconfig 风格编辑器）
    - `deploy.rs`（docker/kind/helm 编排 + Kubernetes/Docker API）
    - `state.rs`（状态落盘与读取，后续补齐）
    - `modes/`（后续可拆分：k8s_kind/k8s_existing/docker_local）

兼容策略：保留 `../scripts/kind-openai-quickstart.sh`，后续可改为 wrapper 调用新工具，避免破坏现有用法。

## 5. CLI 形态（子命令）

统一入口（示例）：
- `./tools/spear-quickstart/target/debug/spear-quickstart configure --config .tmp/spear-quickstart/config.toml`
- `./tools/spear-quickstart/target/debug/spear-quickstart plan --config .tmp/spear-quickstart/config.toml`
- `./tools/spear-quickstart/target/debug/spear-quickstart apply --config .tmp/spear-quickstart/config.toml --yes`
- `./tools/spear-quickstart/target/debug/spear-quickstart status --config .tmp/spear-quickstart/config.toml`
- `./tools/spear-quickstart/target/debug/spear-quickstart cleanup --config .tmp/spear-quickstart/config.toml --scope release,namespace,kind --yes`

行为约定：
- `configure`：只编辑 config，不改动环境。
- `plan`：打印将执行的步骤与关键命令（便于 review）。
- `apply`：执行部署；成功后写入 state（用于精确 cleanup）。
- `cleanup`：默认基于 state 清理；无 state 时仅执行安全清理（如仅卸载 Helm release）。

## 6. 配置文件规范（TOML，v1）

设计原则：模式互斥、默认安全、不存密钥。

```toml
version = 1

[mode]
# one of: "k8s-kind" | "k8s-existing" | "docker-local"
name = "k8s-kind"

[paths]
workdir = "."
state_dir = ".tmp/spear-quickstart"

[build]
enabled = true
pull_base = true
no_cache = false
debian_suite = "trixie"

[images]
tag = "local"
sms_repo = "spear-sms"
spearlet_repo = "spear-spearlet"

[components]
enable_web_admin = true
enable_router_filter = true
enable_e2e = false
spearlet_with_node = true
spearlet_with_llama_server = true

[logging]
debug = true
log_level = "info"
log_format = "json"

[timeouts]
rollout = "300s"

[k8s]
namespace = "spear"
release_name = "spear"
chart_path = "deploy/helm/spear"
values_files = ["deploy/helm/spear/values-openai.yaml"]

[k8s.kind]
cluster_name = "spear-openai"
reuse_cluster = false
keep_cluster = true
kubeconfig_file = ".tmp/kubeconfig-kind-spear-openai"

[k8s.existing]
# when mode.name = "k8s-existing"
kubeconfig = ""      # optional
context = ""         # optional, else current context

[secrets.openai]
# one of: "skip" | "from-env"
source = "from-env"
env_name = "OPENAI_API_KEY"
k8s_secret_name = "openai-api-key"
k8s_secret_key = "OPENAI_API_KEY"

[docker_local]
# when mode.name = "docker-local"
network_name = "spear-quickstart"
sms_name = "spear-sms"
spearlet_name = "spear-spearlet"
publish_sms_http = "18080:8080"
publish_spearlet_http = "18081:8081"
```

说明：
- `mode.name` 决定执行路径；非当前 mode 的字段在 validate 时应提示并忽略或报错，防止混用。
- `k8s.values_files` 支持数组，便于逐步扩展。
- `secrets.openai` 只支持从环境变量注入或跳过，不允许把 key 写入配置。

## 7. TUI（menuconfig 风格）交互设计

主菜单建议：
1) Mode：选择 `k8s-kind / k8s-existing / docker-local`
2) Build：是否 build、pull_base、no_cache、debian_suite、tag/repo
3) Components：web admin / router filter / e2e / spearlet target
4) K8s/Helm（仅 k8s 模式）：namespace、release、values files、timeout、日志策略
5) Kind（仅 k8s-kind）：cluster name、reuse/keep、kubeconfig 路径
6) Existing Cluster（仅 k8s-existing）：kubeconfig/context
7) Secrets：显示 OPENAI_API_KEY 是否存在（只显示 presence，不回显内容）
8) Plan & Apply：展示 plan；确认后执行 apply
9) Cleanup：清理子菜单（scope 多选 + 二次确认）
10) Save & Exit

最佳实践要点：
- 默认安全：检测到 kind 集群已存在时，必须明确提示 “Reuse / Recreate”，默认 Reuse。
- cleanup 强确认：涉及删除 kind cluster、docker network/volume 时，强制二次确认（除非 `--yes`）。
- 即刻校验：离开页面时做轻量校验（values 文件、chart 路径等是否存在）。

## 8. 执行流程（apply）

分 5 个阶段，plan 输出应覆盖每阶段关键动作。

### Phase A: Validate
- 依赖检查（按 mode 决定）：docker daemon、kind、helm（以及 kubeconfig 可达性：通过 Kubernetes API）
- 配置校验：路径存在、模式互斥、字段合法性

### Phase B: Prepare
- 创建 `state_dir`
- 加载/初始化 state：若已部署，提示升级或先 cleanup（TUI/CLI 均可控制）

### Phase C: Build Images（可选）
- 根据 build 选项拼接 docker flags（与现脚本对齐：`--pull`、`--no-cache`）
- 产出镜像列表写入 state（供 kind load / docker-local 复用）

### Phase D: Deploy（按 mode）

#### k8s-kind
- 创建/复用 kind cluster
- export kubeconfig 到指定文件
- kind load docker-image（必要时）
- 创建 namespace（如不存在）
- secrets：若 `from-env` 且 env 存在，则创建/更新 Secret；否则跳过
- helm upgrade --install（values_files + 必要 `--set` 覆盖）
- rollout/wait ready（timeout 可配置）

#### k8s-existing
- 使用 kubeconfig/context（不改写用户默认 kubeconfig）
- 其余步骤与 k8s-kind 类似（不包含 kind 操作）

#### docker-local
- docker network create（若不存在）
- docker run sms + spearlet（基于 `docker_local.*` 的网络、容器名与端口映射）
- openai key：从 env 注入容器 env（不落盘）
- 做基础 health/连通性检查

### Phase E: Post
- 输出下一步指引（kubectl get pods、port-forward、docker logs 等）
- 写入 state（便于 status/cleanup）

## 9. 状态文件（state）与 cleanup 设计

核心原则：cleanup 不依赖猜测，依赖 state，确保可控与安全。

state 建议包含：
- mode、config_path、apply_time
- k8s：namespace、release、kubeconfig/context、是否创建过 namespace/secret
- kind：cluster_name、是否本次创建/是否复用、kubeconfig_file
- docker-local：network、container names/ids、ports
- images：构建出的镜像列表

cleanup 策略：
- 默认读取 state 并逆序清理：卸载 Helm release →（可选）删除 secret/namespace →（可选）删除 kind cluster → 停止并删除容器/网络。
- 支持 scope：`--scope release,namespace,kind,images,containers,network`
- 保护策略：
  - 不删除非工具创建的集群（除非显式确认）
  - 默认不删除 namespace（避免误删用户其他资源）

## 10. 安全与密钥处理
- 禁止把 `OPENAI_API_KEY` 写入 config 或 state。
- 只支持：
  - `from-env`：运行时读取；k8s 模式创建 Secret；docker-local 模式注入容器 env
  - `skip`：不处理密钥，用户自备
- TUI 只展示 “present / missing”，不回显内容。

## 11. 验收标准（用于评审）
- 配置可复现：同一份 config 能稳定复现同样部署（非交互可跑）。
- TUI 可用：能覆盖关键字段；mode 切换显示/校验合理。
- apply/cleanup 幂等：多次执行不会造成不可预期副作用；cleanup 默认安全。
- 密钥不落盘：grep state/config 不出现真实 key。
