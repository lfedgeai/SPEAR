# 管理页面概览

本文概述 `spear-next` 新增的 Web 管理页面。

## 能力

- 独立端口（默认 `127.0.0.1:8081`），Axum 路由提供接口
- 节点列表（搜索、排序、分页）
- 统计卡片（总数、在线、离线、最近 60s 心跳）
- SSE 流 `GET /admin/api/nodes/stream`
  - 测试友好：`?once=true` 返回单次快照事件后结束
- 主题切换（暗/亮）与时区选择，时间按所选时区友好显示
- 可选鉴权：`SMS_WEB_ADMIN_TOKEN`（Bearer Token）

## 配置

- 启用：`--enable-web-admin`
- 地址：`--web-admin-addr 0.0.0.0:8081`
- 环境变量：`SMS_ENABLE_WEB_ADMIN`、`SMS_WEB_ADMIN_ADDR`

## 实现说明

- 前端通过内嵌静态资源提供（`index.html`、`react-app.js`、`style.css`）
- 使用 Ant Design 5 的主题算法与 token，确保暗/亮主题正确切换
- 顶部栏显示时区信息与 Profile 占位
- SSE 通过 `CancellationToken` 支持优雅关闭

## 接口

- `GET /admin/api/nodes` → 返回 `uuid`、`name`、`ip_address`、`port`、`status`、`last_heartbeat`、`registered_at`
- `GET /admin/api/nodes/:uuid` → 返回节点与（可选）资源信息
- `GET /admin/api/stats` → 统计总数/在线/离线/最近 60s
- `GET /admin/api/nodes/stream[?once=true]` → SSE 快照事件

### 任务接口

- `GET /admin/api/tasks` → 返回任务列表，包含字段：
  - `task_id`、`name`、`description`、`status`、`priority`、`node_uuid`、`endpoint`、`version`

## Secret/Key 管理建议

如果后续在 Web Admin 增加“API key 配置”相关组件，建议将其设计为“secret 引用管理”，而不是在 UI 中录入与存储明文 key。

- UI/控制面管理：backend instance 与 `api_key_env`（或 `api_key_envs`）的映射
- secret 值的落地：交由部署系统注入（Kubernetes Secret / Vault Agent / systemd drop-in）
- 可观测性：仅展示“是否存在/可用”（例如由 spearlet 心跳上报 `HAS_ENV:<ENV_NAME>=true`），不展示值
  - `execution_kind`（`short_running | long_running`）、`executable_type`、`executable_uri`、`executable_name`
  - `registered_at`、`last_heartbeat`、`metadata`、`config`
  - `result_uris`、`last_result_uri`、`last_result_status`、`last_completed_at`、`last_result_metadata`
- `GET /admin/api/tasks/{task_id}` → 返回任务详情（字段同上）
- `POST /admin/api/tasks` → 创建任务
  - 请求体包含 `name`、`description`、`priority`、`node_uuid`、`endpoint`、`version`、`capabilities`、`metadata`、`config`、可选 `executable`
  - `metadata.execution_kind` 映射为服务端的 `execution_kind` 枚举

## 测试

- SSE 集成测试使用 `?once=true` 避免阻塞
- 前端测试后续可引入 E2E；当前后端已覆盖列表/统计/SSE
