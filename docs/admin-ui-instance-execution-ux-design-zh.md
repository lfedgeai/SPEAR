# Admin UI：Instance / Execution 展示与导航设计（建议稿）

## 1. 背景与目标

当前 Admin UI 已覆盖 Nodes、Tasks、Backends、MCP Servers，以及“发起一次 Invocation/Execution”的操作入口，但缺少对运行时层面的可观测视图：**当前有哪些实例在跑（Instances）**、**每个实例上有哪些执行（Executions）**、以及**如何从 Task 快速定位到某次 Execution 并查看其日志/元信息**。

本设计目标是在不引入过多概念负担的前提下，给用户一个符合业界习惯的“从部署到运行到单次执行”的心智路径：

- Task：一个可被调度/调用的“服务/函数/任务定义”（部署维度）
- Instance：Task 在某个 Node 上的一个运行载体（运行承载维度）
- Execution：一次实际的运行/调用（请求维度）

### 目标（P0）
- 从 Task 出发，能看到“活跃实例列表（Active Instances）”。
- 从 Instance 出发，能看到“最近执行列表（Recent Executions）”。
- 能通过 Execution ID 打开 Execution 详情（包括状态、时间戳、log_ref 等）。
- 支持可分享的深链接（URL）与基本筛选/排序。

### 非目标（暂不做）
- 全局 Execution 搜索（跨 Task/Instance 的时间范围检索、复杂过滤）。
- 聚合指标（SLO、p95、错误分布等）的大盘（可作为后续阶段）。
- 直接在 UI 内“tail 日志/下载日志”的完整闭环（可先展示 log_ref，后续接日志后端）。

## 2. 约束与现状（基于当前可用接口）

目前后端可用的关键查询接口（HTTP）：
- `GET /api/v1/tasks/{task_id}/instances`：列出某 Task 的活跃实例（分页）
- `GET /api/v1/instances/{instance_id}/executions`：列出某 Instance 的最近执行（分页）
- `GET /api/v1/executions/{execution_id}`：获取某次执行详情

这组接口天然形成一条推荐的 UI 浏览路径：

`Task -> Instances -> Instance -> Executions -> Execution`

因此在 P0 阶段，UI 的信息架构应围绕“在 Task/Instance 上下文中查看 Executions”，而不是一上来就提供“全局 Executions 列表”（因为没有后端索引支持，前端聚合会很重且不可靠）。

## 3. 信息架构（IA）与导航建议

### 3.1 推荐方案：Task 详情页内 Tab + 深链接

业界 Console（K8s Dashboard、Cloud Run、Lambda、Ray Dashboard）常见做法是：
- 左侧导航保留高层资源：Nodes、Tasks、Backends、MCP…
- **Task 作为入口**，在 Task 详情页提供二级信息（Instances/Executions/Logs/Config…）

建议结构：
- Sidebar（一级）
  - Dashboard（现有 Stats）
  - Nodes
  - Tasks
  - Backends
  - MCP Servers
  - Files（如现有）
- Tasks 列表页（/admin/tasks）
  - 点击某条 Task 进入 Task 详情页（/admin/tasks/:taskId）
- Task 详情页（/admin/tasks/:taskId）
  - Tabs：
    - Overview（默认）
    - Instances（P0 新增）
    - Executions（P1+，见下文）/ 或先不提供

在 P0 阶段：**Task 详情页只加 Instances Tab**；Executions 先通过 Instance 详情进入，避免 UI 提供“看似能列出 task 的 executions 但实际做不到”的体验落差。

### 3.2 为何不建议“Tasks 下挂 Instances/Executions 作为侧边二级菜单”

你的想法（Tasks 可展开，显示 Instances/Executions 两个二级项）在“全局 Instances/Executions 可浏览”的产品里是成立的；但在当前阶段：
- 没有“全局列 Instances/Executions”的查询能力（尤其 Execution）
- 用户点击后期望看到“全量列表”，却只能看到某个 Task/Instance 视角，会产生认知断裂

更符合用户理解的方式是：**把 Instances/Executions 放在 Task 的上下文里**，用 Tab 或二级页面表达“这是 Task 的组成部分”。

### 3.3 进阶方案（P1+）：增加全局 Executions 页面（Search-first）

当后端补齐类似以下能力后，再引入“全局 Executions”入口会更自然：
- `GET /api/v1/executions?task_id=&node_uuid=&status=&since=&until=&q=`
- 或基于 event stream / 索引支持更强过滤

届时 Sidebar 可新增：
- Executions（全局检索页，默认是“搜索/过滤器 + 最近执行”）

这更符合业界 best practice：**全局 Executions 是“查问题的搜索入口”，不是“资源树的一层”**。

## 4. 页面与组件设计（P0 可落地）

### 4.1 Tasks 列表页（/admin/tasks）

新增/调整建议：
- 列表列（建议）：Task ID / Name / Version / Node / Last Result / Updated / Actions
- 增加一列 “Active Instances”（可选）
  - 展示一个数字 + 状态点（例如：`3`，颜色跟随“是否有活跃实例”）
  - 点击数字直接跳到 Task 详情的 Instances Tab

用户价值：
- 让用户在“部署维度”快速判断这个 Task 是否在跑、跑了多少个实例。

实现建议：
- P0 可以不加“Active Instances”列（需要额外请求），先通过 Task 详情页提供。
- 若要加，建议按行懒加载/批量加载，避免 Tasks 列表 N+1 请求（可后端加批量接口再做）。

### 4.2 Task 详情页（/admin/tasks/:taskId）

#### 顶部概要（Overview 区）
- Task 基本信息：task_id / name / version / endpoint / capabilities / runtime
- “运行态摘要”卡片（P0 可选）：
  - Active instances count（由 Instances 接口返回 length 或额外 count 字段）
  - 最近一次完成时间（现有字段如 last_completed_at）

#### Instances Tab（/admin/tasks/:taskId?tab=instances 或 /admin/tasks/:taskId/instances）

表格字段（建议）：
- Instance ID（可复制）
- Node UUID（可点击跳 Node 详情）
- Status（badge：running/idle/terminating/terminated/unknown）
- Last Seen（相对时间 + hover 显示绝对时间）
- Current Execution（如果存在：显示 execution_id，点击进入 Execution 详情）
- Actions：View Executions（跳转到 Instance 详情）

交互建议：
- 自动刷新：仅在页面可见时，以 5s~15s interval 刷新（running/idle 才刷新）
- 分页：采用 page_token，支持“Load more”或传统分页
- 空态：如果没有实例，显示明确解释：“当前没有活跃实例；可能尚未触发、或实例已过期下线”

### 4.3 Instance 详情页（/admin/instances/:instanceId）

建议新增一个 Instance 详情页（因为 Executions 是 Instance 的直接子资源）：

顶部概要：
- Instance ID、所属 Task ID（可点击回 Task）、Node UUID（可点击）、Status、Last Seen、Current Execution

Executions 列表（/admin/instances/:instanceId?tab=executions）：
- Execution ID（可复制）
- Status（pending/running/completed/failed/cancelled/timeout）
- Started / Completed（时间）
- Duration（completed-started）
- Function（function_name）
- Actions：Open（进入 Execution 详情）

交互建议：
- running/pending 记录在列表中优先置顶（或按 started_at 倒序）
- 支持状态过滤（dropdown：all/running/failed/completed）
- 支持“只看失败”快捷筛选

### 4.4 Execution 详情页（/admin/executions/:executionId）

展示内容（P0）：
- 基本：execution_id / invocation_id / task_id / instance_id / node_uuid / function_name
- 状态：status + started_at_ms / completed_at_ms / updated_at_ms
- metadata：以 key-value 表格展示（可折叠）
- log_ref：展示为链接/可复制信息
  - backend / uri_prefix / content_type / compression
  - 若未来接入日志下载/预览，此处作为入口按钮区

信息组织建议：
- 用 “Summary / Metadata / Logs” 三段，默认展开 Summary
- 所有关联 ID 均可点击跳转（Task、Instance、Node）

## 5. 用户心智与命名（减少困惑的关键）

### 5.1 术语一致性
- 页面上优先显示易懂名词，鼠标 hover 再解释底层概念
  - “Instance（实例）”：一个运行承载（类似 Pod/Worker）
  - “Execution（执行）”：一次运行（类似 Request/Invocation）

### 5.2 Breadcrumb 与“从哪里来”
每个详情页提供面包屑，避免用户迷路：
- `Tasks / {taskId} / Instances`
- `Tasks / {taskId} / Instances / {instanceId}`
- `... / Executions / {executionId}`

并在详情页提供 “Back” 默认回到上一级列表（保持用户上下文）。

### 5.3 “Executions 在哪里看”的统一答案
P0 阶段建议对用户强调一条路径：
- “想看某个 Task 的运行情况” → 打开 Task → Instances → 选 Instance → Executions

等后端支持全局检索后，再补充第二条路径：
- “只知道 execution_id 或想全局查失败/慢请求” → 全局 Executions 搜索页

## 6. 分期路线（推荐）

### Phase 0（现在即可做）
- Task 详情页新增 Instances Tab
- 新增 Instance 详情页 + Executions 列表
- 新增 Execution 详情页（展示元信息 + log_ref）
- 所有页面支持复制 ID、跳转关联资源、基础分页与刷新

### Phase 1（体验增强）
- Tasks 列表增加 Active Instances 快捷入口（需要避免 N+1）
- Execution 详情页展示结构化错误（若 metadata 里有 error_message 等）
- “最近失败执行”快捷入口（可在 Instance 页上做筛选）

### Phase 2（业界最佳：Search-first Observability）
- 全局 Executions 页面（按 task/node/status/time 搜索）
- 日志预览/下载（基于 log_ref + 后端日志服务）

## 7. 与后端接口的映射（P0）

- Task Instances：
  - UI：Task 详情 / Instances Tab
  - API：`GET /api/v1/tasks/{task_id}/instances?limit=&page_token=`
- Instance Executions：
  - UI：Instance 详情 / Executions 列表
  - API：`GET /api/v1/instances/{instance_id}/executions?limit=&page_token=`
- Execution 详情：
  - UI：Execution 详情页
  - API：`GET /api/v1/executions/{execution_id}`

备注：当前 Admin UI 既有数据接口多为 `/admin/api/...`。P0 推荐直接复用 `/api/v1/...` 这组新接口（同源调用更直接）；如需统一风格，可在后端增加 `/admin/api/...` 的薄封装路由，返回相同 JSON 结构。

