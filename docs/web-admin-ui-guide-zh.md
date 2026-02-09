# Web 管理页面使用指南

本文介绍 `spear-next` 管理页面的实际使用与交互细节，覆盖节点、文件与任务创建等功能。

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

## 文件（Files）

- 选择文件：点击 `Choose File` 打开系统文件选择器（隐藏原生 `<input type="file">`，前端使用按钮触发）
- 上传：点击 `Upload` 上传到内置对象服务；成功后收到 `Uploaded: <id>` 提示
- 列表操作：
  - `Download`：直接下载
  - `Copy URI`：复制 `smsfile://<id>` 到剪贴板
  - `Delete`：删除后立即刷新（React Query 失效+本地过滤）

## 任务创建（Tasks → Create Task）

- 可执行类型：`No Executable | Binary | Script | Container | WASM | Process`
- Scheme：`smsfile | s3 | minio | https`
  - 切到 `smsfile` 会自动预填 `smsfile://`
  - 从非 `smsfile` 切回时不会重置为占位项
- 选择本地 SMS 文件：
  - 点击 `Choose Local` 打开文件选择弹窗
  - 点击 `Use` 将 `Executable URI = smsfile://<id>` 与 `Executable Name` 带回表单
- 参数：`Capabilities`（逗号分隔）、`Args`（逗号分隔）、`Env`（每行 `key=value`）

## AI Models

- AI Models 页面分为 `Local` 与 `Remote`
- 列表支持搜索与可用性筛选（available/unavailable）
- 点击某一行进入详情页，查看该模型在各节点上的实例分布与状态

### Local：创建 deployment

- 入口：Local → AI Models → `Create`
- 表单：
  - Node：选择部署到哪个节点
  - Provider：默认 `LLaMA CPP`
  - Model name：展示用名称
  - Model URL：当 Provider=llamacpp 时必填，填写 `.gguf` 文件直链（http/https）
- 提交成功后会跳转并高亮对应的 deployment（URL query `deployment_id=...`），并在页面下方的 `Provisioning` 面板显示部署进度

### Local：删除 deployment（已 available 也可删除）

- 入口：Local → AI Models 列表右侧 `Actions` 列 → `Delete`
- 行为：
  - 会对该 `(provider, model)` 在关联节点上的 deployment 逐个删除
  - Spearlet 在下一轮 reconcile 中停止本地进程并从 backend registry 移除
- 删除后列表会自动刷新

## 常见问题

- 下拉长期悬浮：已移除强制 `open`，恢复默认交互
- Scheme 重置：当 URI 不包含 `://` 不再重置；`smsfile` 会预填 `smsfile://`
- 文件对话框溢出：设置了固定宽度与列宽，长文本省略显示

## 相关文档

- `docs/web-admin-overview-zh.md`
- `docs/ui-tests-guide-zh.md`
- `docs/ollama-discovery-zh.md`
