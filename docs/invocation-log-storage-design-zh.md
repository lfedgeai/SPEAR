# Invocation 日志持久化与查看设计

## 背景

目前 Web Admin 触发执行（`POST /admin/api/executions`）会返回 `invocation_id` / `execution_id`，但系统缺少“面向用户”的 invocation 日志持久化与查看能力（例如 stdout/stderr、运行时/系统事件）。这会显著增加排障、审计和复现成本。

本文给出一套业界通用（best practice）的设计：在 SMS Web Admin BFF + Web Admin UI 中提供“可持久化、可分页、可追尾、可下载”的 invocation 日志。

相关文档：

- 执行模型：[invocation-execution-model-refactor-zh.md](./invocation-execution-model-refactor-zh.md)
- Web Admin 总览：[web-admin-overview-zh.md](./web-admin-overview-zh.md)

## 目标

- 为每次 invocation 保存可查看的日志（支持 spillback/retry 产生的多次 execution）。
- Web Admin 可快速定位并查看：列表 → 详情 → 实时追尾（tail）→ 下载。
- 支持按 stream/level 过滤，并支持稳定 cursor 的分页/定位。
- 安全与成本的合理默认值：保留周期、大小限制、背压、脱敏。
- 明确区分：Spearlet 服务日志（控制面） vs 用户 invocation 日志（数据面）。

## 非目标

- 不替代企业级日志平台（Loki/Elastic/Splunk），但保留对接路径。
- 不在 MVP 阶段提供“跨年级别”的全量全文检索（可后续分期）。
- 不在本文覆盖交互式 console（另文档单独设计）。

## 术语

- **Invocation**：一次用户/客户端函数调用请求；`invocation_id` 稳定。
- **Execution**：invocation 的一次具体尝试（落在某个节点/实例）；`execution_id`。
- **Invocation 日志**：面向用户的、附着在 execution 上的日志流，包含：
  - 进程 stdout/stderr
  - 运行时/系统事件（可选）
  - 运行时 API 发出的结构化日志（可选）

## 需求

### 功能性需求

- 以 **execution** 为基本持久化单位；以 **invocation** 聚合多个 execution。
- 支持历史查看（分页）与实时追尾（tail）。
- 支持整份日志下载。
- 每条日志必须包含用于关联与排障的关键字段：
  - `invocation_id`、`execution_id`、`task_id`、`function_name`、`node_uuid`、`instance_id`。

### 非功能性需求

- **保留策略**：默认 TTL 可配置（例如 7–30 天）。
- **成本/体积控制**：单 execution 最大字节数、单行截断、分片 + 压缩。
- **背压**：不能因日志过量导致 Spearlet OOM；需要可控的丢弃策略。
- **安全**：鉴权/授权、多租户隔离（未来）、敏感信息脱敏、下载安全。
- **可靠性**：允许 SMS 短暂不可用；持久化尽力而为（best effort）并在 UI 中可见。

## 总体架构

### 组件

1. **Spearlet Log Collector（按 execution）**
   - 采集 stdout/stderr 与可选系统事件。
   - 有界缓冲（bounded queue），避免内存失控。
   - 按 chunk 刷写到中心存储。

2. **SMS Log Storage（中心存储）**
   - **Blob 存储**保存日志分片（append-only）：生产建议对象存储；开发/默认可复用现有 `sms+file://` 文件存储。
   - **元数据存储**保存索引与 cursor：复用项目已有 KV（RocksDB/Sled）。

3. **SMS Web Admin BFF**
   - 提供 execution/invocation 列表与日志读取接口。
   - 提供 SSE tail 接口用于“跟随模式”。

4. **Web Admin UI**
   - 增加 Execution 列表与日志查看器。
   - 从任务执行入口提供“查看日志”的深链接。

### 数据流

1. 用户在 Web Admin 触发执行 → SMS BFF 选点并 spillback 调用 Spearlet。
2. Spearlet 开始执行并把日志事件写入 collector。
3. collector 将 chunk + 元数据写入 SMS Log Storage。
4. UI 通过 SMS BFF 拉取历史、通过 SSE 追尾。

## 存储设计

### 日志行（逻辑）结构

建议使用结构化行存储，落盘为 NDJSON：

```json
{
  "ts_ms": 1730000000000,
  "seq": 42,
  "invocation_id": "...",
  "execution_id": "...",
  "task_id": "...",
  "function_name": "__default__",
  "node_uuid": "...",
  "instance_id": "...",
  "stream": "stdout",
  "level": "info",
  "message": "hello",
  "attrs": {"k": "v"}
}
```

说明：

- `seq` 为 execution 内单调递增序号，用于稳定分页。
- `stream` 建议固定枚举：`stdout|stderr|system`。
- `attrs` 可选，用于携带运行时特定字段。

### 分片（chunk）格式

- Chunk 文件路径：`invocations/{invocation_id}/executions/{execution_id}/chunks/{chunk_seq}.ndjson.zst`
- Chunk 大小：压缩后 1–4 MiB（可配置）。
- 压缩：优先 Zstd（或 gzip 兜底）。

### 元数据（KV）

逻辑 key 设计：

- `exec:{execution_id}:meta` → { task_id, invocation_id, node_uuid, instance_id, started_at, completed_at, status, byte_count, first_seq, last_seq }
- `exec:{execution_id}:chunks` → [{ chunk_seq, first_seq, last_seq, uri, byte_count, created_at }]
- `inv:{invocation_id}:executions` → execution_id 列表（attempts）

这样 UI 列表/定位不需要扫描 blob。

### 保留与清理

- SMS 周期性清理：
  - 当 `completed_at + ttl < now` 删除 chunk 与 KV 元数据。
  - 通过 `max_bytes` 限制单 execution 总日志；超过后截断并标记 `truncated=true`。

## API 设计（SMS Web Admin BFF）

UI 仅调用 SMS（避免浏览器直接连 Spearlet）。

### 1）执行创建返回值

建议更新 `POST /admin/api/executions` 返回值，补齐 `invocation_id` 与日志链接：

```json
{
  "success": true,
  "invocation_id": "...",
  "execution_id": "...",
  "node_uuid": "...",
  "message": "ok",
  "log_url": "/admin/executions/{execution_id}"
}
```

### 2）执行列表

`GET /admin/api/executions?task_id=&invocation_id=&limit=&cursor=`

返回包含 `execution_id`、`invocation_id`、`status`、时间戳、`node_uuid` 的 summary 列表。

### 3）执行详情

`GET /admin/api/executions/{execution_id}`

返回 execution 元数据与 `log_state`：

- `ready`：可读历史日志
- `pending`：执行中但尚未刷写
- `unavailable`：存储失败或暂不可用

### 4）读取历史日志（分页）

`GET /admin/api/executions/{execution_id}/logs?limit=500&cursor=...&stream=stdout,stderr&level=info,warn,error`

响应：

```json
{
  "execution_id": "...",
  "lines": [ {"ts_ms":..., "seq":..., "stream":"stdout", "message":"..."} ],
  "next_cursor": "...",
  "truncated": false
}
```

cursor 建议：

- 使用不透明 cursor，内部编码 `(seq, chunk_seq, offset)`。
- 支持 `direction=backward|forward` 以满足“向上翻历史”的体验。

### 5）实时追尾（SSE）

`GET /admin/api/executions/{execution_id}/logs/stream`

SSE payload：

- `event: log` + `data: {line...}`
- 当 execution 结束且 flush 完成后发送 `event: eof`

### 6）下载

`GET /admin/api/executions/{execution_id}/logs/download`

- Content-Type：`text/plain` 或 `application/x-ndjson`
- 可选 `?format=text|ndjson`

## Spearlet 侧设计

### 采集来源

- **Process runtime**：接管子进程 stdout/stderr 管道并写入 execution log。
- **WASM runtime**：增加 hostcall “log” API，将 WASM 内日志映射到 invocation log。
- **系统事件**：可选记录生命周期里程碑（scheduled/started/completed、资源指标）。

### 背压策略

- 按 execution 设置有界 ring buffer（按 bytes/行数限制）。
- flush 触发条件：
  - 达到大小阈值
  - 达到时间阈值（例如每 1s）
  - 执行结束

丢弃策略（可配置）：

- `drop_oldest`（默认）：保留最新上下文。
- `drop_newest`：保留最早上下文。

### 失败处理

- 当 SMS 日志存储不可达：
  - 保留少量本地缓冲；
  - 将 `log_state=unavailable` 写入元数据；
  - UI 展示“日志暂不可用，可稍后重试”。

## Web Admin 前端改造建议

目标工程：`web-admin/`（React + TanStack Query）。当前 UI 通过 [executions.ts](../web-admin/src/api/executions.ts) 调用 `POST /admin/api/executions`。

### 新增页面与导航

- 增加 “Executions” 页面：
  - 路由：`/executions`
  - 列表：支持过滤（task_id、status、时间范围、node）
  - 点击进入详情：`/executions/:execution_id`

### Execution 详情页

- 顶部信息：status、task_id、node_uuid、invocation_id、时间戳。
- 日志查看器：
  - Follow（自动滚动）开关
  - stdout/stderr/system 筛选
  - level 筛选（如支持）
  - 视窗内搜索（客户端）
  - 下载按钮
  - 选中复制

### API client 增补

- 新增 `src/api/logs.ts`：
  - `getExecution(execution_id)`
  - `getExecutionLogs(execution_id, cursor, limit, filters)`
  - `streamExecutionLogs(execution_id)`（EventSource）

### 交互细节（best practice）

- 大日志使用虚拟列表渲染。
- 达到 `max_bytes` 时展示 “truncated” 提示条。
- stdout/stderr 视觉区分（颜色或标签）。
- Follow/filters 通过 query 参数持久化，便于分享链接与复现。

## 安全考虑

- 不在 UI 中暴露敏感信息：
  - 在 SMS 侧对常见模式进行脱敏（API key、Bearer token 等）。
  - 默认不存储环境变量。
- 鉴权：
  - 复用 `SMS_WEB_ADMIN_TOKEN` 的 Bearer token 机制。
  - 未来：支持租户 RBAC 与审计日志。
- 下载安全：
  - 使用 Content-Disposition 附件下载；限制最大体积；仅用 ID 路由避免路径穿越。

## 可观测性

- 每行带关联字段，保证端到端定位。
- 指标：
  - 写入字节数、丢弃行数、flush 延迟、存储失败率。
- flush 操作加入 tracing span，便于定位性能瓶颈。

## 分期落地

1. **Phase 0（MVP）**
   - 持久化 stdout/stderr（按 execution）。
   - 实现历史读取 + 下载。
   - UI：Execution 详情页 + 日志查看器。

2. **Phase 1**
   - SSE 追尾。
   - Execution 列表页。

3. **Phase 2**
   - 丰富的结构化 level 与运行时事件。
   - 可选全文检索（配置开关）。

## 备选方案

- **对接外部日志平台**：通过 OTEL/Fluent Bit 输出日志，并在 Grafana/Kibana 查询。
  - 优点：可扩展检索、长周期保留。
  - 缺点：依赖外部基础设施，不够自包含。

该方案强调先做“产品内可用的最小闭环”，同时保留与企业日志平台的对接路径。
