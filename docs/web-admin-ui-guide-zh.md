# Web 管理页面使用指南

本文介绍当前 `spear-next` 管理页面的实际使用与交互细节，覆盖节点、文件、任务、AI backend 与相关控制面操作。

## 访问与鉴权

- 启用：运行 SMS 时添加 `--enable-web-admin --web-admin-addr 127.0.0.1:8081`
- 地址：`http://127.0.0.1:8081/admin`
- 管理 Token：
  - Nodes 页工具栏或 Settings 页输入 Token 并点击 `Apply`
  - Token 会写入 `localStorage('ADMIN_TOKEN')`

## 顶部设置

- 主题：暗/亮切换使用 Ant Design 主题算法，所有控件自动适配
- 时区：设置页选择后，页面所有时间按所选时区显示（包括任务与文件）

## 节点（Nodes）

- 列表支持搜索、按时间排序、分页与详情弹窗
- SSE：后端 `GET /admin/api/nodes/stream`，用于实时刷新统计与列表
- 工具栏：包含搜索框、排序选择、Admin Token 输入框与 `Apply Token` 按钮

## 对象（Objects）

- 选择文件：点击 `Choose files` 打开系统文件选择器（隐藏原生 `<input type="file">`，前端使用按钮触发）
- 上传：点击 `Upload` 将本地文件上传为对象；当前兼容模式下对象 key 由后端适配层生成
- 列表操作：
  - `Details`：进入对象详情页查看 metadata
  - `Download`：直接下载对象内容
  - `Pin / Unpin`：切换对象的删除保护状态
  - `Retain / Release`：调整对象引用计数
  - `Delete`：删除后立即刷新
- 页面默认展示 metadata，不在列表页直接展示对象内容
- 当对象被引用或已被 pin 时，前端会禁用删除按钮

## 任务创建（Tasks → Create Task）

- 当前弹窗已按区块组织：
  - `Task Basics`
  - `MCP tools`
  - `Routing`
  - `Executable`
- 长表单对话框统一采用：
  - sticky header
  - 可滚动 body
  - 固定 footer 操作区
- 可执行类型：`No Executable | Binary | Script | Container | WASM | Process`
- Executable Source：`object | s3 | minio | https`
- 选择对象资源：
  - 点击 `Choose Object` 打开对象选择弹窗
  - 点击 `Use` 将对象 key 带回表单
- 提交任务时会写入结构化 `reference`，对象来源使用 `{ "kind": "object", "object_key": "<key>" }`
- 参数：`Capabilities`（逗号分隔）、`Args`（逗号分隔）、`Env`（每行 `key=value`）

## AI Models

- AI Models 页面分为 `Local` 与 `Remote`
- 列表支持搜索与可用性筛选（available/unavailable）
- 点击某一行进入详情页，查看该模型在各节点上的实例分布与状态
- AI Models 现为只读聚合页。创建、编辑、placement、启停和删除动作统一在 `AI Backends` 中执行。
- Remote AI Models 页面提供跳转到共享 credentials 页的入口：`AI Backends → Credentials`

## AI Backends

- 这是 backend 定义、placement 与 credentials 的控制面写入口。
- 页面顶部可切换：
  - `Backends`
  - `Model Views`
- 创建入口拆成两条显式流程：
  - `Create Remote Backend`
  - `Create Local Backend`
- backend 行内仍可继续编辑已有配置。

### Create Remote Backend

- 用于 OpenAI、Ollama 或其他外部 endpoint 类型的 adapter。
- 常见输入包括：
  - `Display name`
  - `Provider`
  - `Model`
  - `Backend kind`
  - `Base URL`
  - `Credential ref`
  - `Operations`
  - `Features`
  - `Transports`
  - `Placement`
- 对于 `openai`、`openai_compatible`、`ollama` 这类已支持的 remote provider，Web Admin 现在会在创建前执行节点侧 preflight。
- 这个 preflight 是从目标 SPEARlet 节点发起的，而不是从浏览器或 SMS 进程发起。
- 对于多节点 placement，默认策略是“创建前抽样严格验证 + 创建后通过 placement reconcile 做全量节点验证”。
- 如果抽样节点上的 endpoint 可达性、credential 验证或 model 访问失败，创建会立即终止，并返回节点侧错误。

### Create Local Backend

- 用于 `llamacpp`、`vllm` 等节点本地 runtime。
- placement 默认是 `Single Node`。
- 对于 `local + llamacpp`，UI 已直接暴露常用 runtime 字段，不再要求手写 JSON：
  - `Model URL`
  - `Model Path`
  - `Skip Download`
  - `Download Timeout (s)`
  - `Threads`
  - `Context Size`
- 这些字段保存时会自动写回 backend metadata，以兼容当前运行时实现。
- 当使用 `Model URL` 时，Web Admin 现在会在创建前执行一次节点侧 preflight。
- 这个 preflight 是从目标节点发起的，而不是从浏览器或 SMS 进程发起。
- 如果目标节点无法访问该 URL，则创建会立即失败，并直接返回节点侧错误。

### Placement

- backend 创建时即可配置 placement。
- 当前支持：
  - `All Nodes`
  - `Single Node`
  - `Selected Nodes`
- `All Nodes` 默认不会在创建前同步阻塞验证每一个节点，而是最多抽样 3 个节点做严格预检，剩余节点交给 assignment controller 在创建后异步完成验证。
- 还支持每条 placement 的 override：
  - `Weight override`
  - `Priority override`

### Backend 详情页

- 详情页可查看：
  - backend 摘要
  - placements
  - node status
  - read model views

## Credentials

- 入口：`AI Backends` → `Credentials`
- 用于创建、轮换、禁用和删除 AI backends 复用的 secret 引用
- Credentials 现作为共享控制面资源管理，不再挂在 AI Models 目录下

## 常见问题

- 下拉长期悬浮：已移除强制 `open`，恢复默认交互
- Scheme 重置：当 URI 不包含 `://` 不再重置；对象资源统一使用对象引用，不再依赖 legacy 文件 URI
- 对象选择弹窗：支持按对象 key 过滤、分页浏览与快速选择

## 相关文档

- [web-admin-overview-zh.md](./web-admin-overview-zh.md) 说明当前页面与 API 结构
- [backend-support-matrix-zh.md](./backend-support-matrix-zh.md) 说明当前支持的 backend kind、provider 与 operation
- [ai-backend-unified-control-plane-design-zh.md](./ai-backend-unified-control-plane-design-zh.md) 说明 AI Backends / AI Models 背后的控制面模型
- [ui-tests-guide-zh.md](./ui-tests-guide-zh.md) 说明前端测试方式
- [ollama-discovery-zh.md](./ollama-discovery-zh.md) 说明 Ollama 发现逻辑
