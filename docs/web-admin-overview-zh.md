# 管理页面概览

本文概述 `spear-next` 新增的 Web 管理页面。

## 能力

- 独立端口（默认 `127.0.0.1:8081`），Axum 路由提供接口
- 节点列表（搜索、排序、分页）
- 后端列表（Backends，聚合视图；支持详情弹窗 Raw JSON）
- 统计卡片（总数、在线、离线、最近 60s 心跳）
- SSE 流 `GET /admin/api/nodes/stream`
  - 测试友好：`?once=true` 返回单次快照事件后结束
- 主题切换（暗/亮）
- 可选鉴权：`SMS_WEB_ADMIN_TOKEN`（Bearer Token）

## 配置

- 启用：`--enable-web-admin`
- 地址：`--web-admin-addr 0.0.0.0:8081`
- 环境变量：`SMS_ENABLE_WEB_ADMIN`、`SMS_WEB_ADMIN_ADDR`

## 实现说明

- 前端通过内嵌静态资源提供（`index.html`、`main.js`、`main.css`）
- 前端源码位于 `web-admin/`，构建后覆盖输出到 `assets/admin/*`
- UI 采用 Radix primitives + Tailwind（shadcn/ui 风格），以企业控制台风为主
- SSE 通过 `CancellationToken` 支持优雅关闭

## 接口

- `GET /admin/api/nodes` → 返回 `uuid`、`name`、`ip_address`、`port`、`status`、`last_heartbeat`、`registered_at`
- `GET /admin/api/nodes/:uuid` → 返回节点与（可选）资源信息
- `GET /admin/api/stats` → 统计总数/在线/离线/最近 60s
- `GET /admin/api/nodes/stream[?once=true]` → SSE 快照事件
- `GET /admin/api/backends` → 返回聚合后的后端列表（按名称/类型/能力汇总，并包含各节点可用性）

### 任务接口

- `GET /admin/api/tasks` → 返回任务列表，包含字段：
  - `task_id`、`name`、`description`、`status`、`priority`、`node_uuid`、`endpoint`、`version`

#### 创建与执行（两条链路）

Web Admin 将“创建任务（注册 Task）”与“执行任务（调度 + 运行）”拆为两步：

- 第一步：创建/注册任务
  - `POST /admin/api/tasks`
  - `node_uuid` 有两种用法：
    - 指定节点：`node_uuid=<uuid>`
    - 自动调度：`node_uuid=""`（空字符串）
- 第二步：触发执行（可选）
  - `POST /admin/api/executions`
  - 由 SMS placement 选择候选节点，并按顺序调用 Spearlet 执行（spillback）

两种模式的差异：

- 指定节点（node_uuid 非空）：
  - 表示 pinned node（任务归属/固定节点，便于观察与过滤；不等同于“本次/最近一次执行落点”）
  - 是否执行由 `POST /admin/api/executions` 决定；UI 的 `Run after create` 会在创建成功后直接对该 node 发起执行
- 自动调度（node_uuid 为空）：
  - 表示“任务不固定节点”
  - 是否执行由 `POST /admin/api/executions` 决定；UI 的 `Run after create` 会在创建成功后调用该接口，让 BFF 通过 SMS placement 选择节点并运行

## Secret/Key 管理建议

如果后续在 Web Admin 增加“API key 配置”相关组件，建议将其设计为“secret 引用管理”，而不是在 UI 中录入与存储明文 key。

- UI/控制面管理：backend instance 与 `credential_ref`（或 `credential_refs`）的映射
- secret 值的落地：交由部署系统注入（Kubernetes Secret / Vault Agent / systemd drop-in）
- 可观测性：仅展示“是否存在/可用”（例如由 spearlet 心跳上报 `HAS_ENV:<ENV_NAME>=true`），不展示值
  - `executable_type`、`executable_uri`、`executable_name`
  - `registered_at`、`last_heartbeat`、`metadata`、`config`
  - `result_uris`、`last_result_uri`、`last_result_status`、`last_completed_at`、`last_result_metadata`
- `GET /admin/api/tasks/{task_id}` → 返回任务详情（字段同上）
- `POST /admin/api/tasks` → 创建任务
  - 请求体包含 `name`、`description`、`priority`、`node_uuid`、`endpoint`、`version`、`capabilities`、`metadata`、`config`、可选 `executable`

## 测试

- SSE 集成测试使用 `?once=true` 避免阻塞
- 前端已包含 Playwright UI 测试（`make test-ui`）
