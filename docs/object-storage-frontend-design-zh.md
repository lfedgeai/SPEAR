# 对象存储前端重构设计

本文档描述在接受 breaking change 的前提下，`web-admin` 应如何围绕新的对象存储架构进行重构。

后端目标架构已经调整为：

- `ObjectCatalog` 负责 metadata
- `BlobStore` 负责 blob
- `ObjectService` 负责对象生命周期编排
- `ObjectJanitor` 负责后台清理与失败收敛

前端不应继续围绕旧的 `Files + legacy 文件 URI` 语义演进，而应直接对齐新的对象域模型，使 UI、API 和用户心智保持一致。

相关文档：

- [rustfs-object-storage-design-zh.md](./rustfs-object-storage-design-zh.md)
- [web-admin-ui-guide-zh.md](./web-admin-ui-guide-zh.md)
- [web-admin-overview-zh.md](./web-admin-overview-zh.md)
- [task-api-refactor-zh.md](./task-api-refactor-zh.md)

## 设计目标

本次前端重构的目标如下：

- 让对象成为前端中的一等资源，而不是隐藏在实现细节里的后端能力
- 让前端信息架构直接映射新的对象系统模型
- 移除旧的 `Files` 心智模型与 legacy 文件 URI 依赖
- 让任务创建、对象浏览、对象操作共享一致的对象引用方式
- 不把 RustFS 这类后端实现细节泄漏到普通业务 UI
- 保持交互清晰、直观、易维护，而不是堆叠兼容层

## 非目标

本次设计不以以下事项为目标：

- 不在前端暴露 RustFS 的全部底层运维参数
- 不在第一阶段实现复杂对象内容在线预览体系
- 不保留 `Files` 与 `Objects` 双轨长期共存
- 不通过前端兼容旧对象 API 的布尔开关语义

## 核心结论

### 1. 前端资源模型要从 `Files` 切换到 `Objects`

接受 breaking change 后，前端不应继续保留“文件是主资源、对象只是后端实现”的结构。

推荐直接改为：

- 删除 `Files` 作为主导航概念
- 增加 `Objects` 作为新的主资源页面
- 将任务中的本地资源选择器从文件选择器改为对象选择器

### 2. 前端不直接配置 RustFS

前端面向对象系统，而不是面向 blob backend 实现细节。

因此：

- 普通管理页面不直接编辑 RustFS bucket/path/timeout
- 前端最多展示对象存储的健康状态和只读摘要
- RustFS 属于运行时后端实现，应主要通过后端配置与部署系统管理

### 3. 任务资源不再使用 legacy 文件 URI

任务创建页中的资源引用应直接对齐新对象系统，而不是继续沿用旧文件 URI。

推荐做法：

- 废弃 legacy 文件 URI scheme
- 任务表单不再暴露“手写 URI”作为主交互
- 引入结构化的对象引用模型

## 前端信息架构

### 新的顶层导航

建议 `web-admin` 主导航调整为：

- Dashboard
- Nodes
- Tasks
- Executions
- Objects
- AI Backends
- MCP
- Settings

其中：

- `Files` 页面被移除
- `Objects` 页面成为对象域主入口

### `Objects` 模块职责

`Objects` 模块应承担以下职责：

- 浏览对象列表
- 查看对象摘要
- 上传对象
- 下载对象
- 删除对象
- retain / release
- pin / unpin
- 进入对象详情页

### `Settings` 中的对象存储状态

建议在 `Settings` 或节点详情页中增加一个只读的 `Object Storage` 区块，用于展示：

- 当前对象存储后端类型
- 健康状态
- endpoint 摘要
- bucket / root 摘要
- 最近错误状态

但不在普通管理 UI 中开放底层后端配置编辑。

## 前端领域模型

前端应围绕对象域建立独立类型，而不是继续复用 `files.ts` 中的文件模型。

建议前端核心类型如下：

```ts
export type ObjectState = "ready" | "deleting";

export interface ObjectSummary {
  key: string;
  size: number;
  checksum: string;
  refCount: number;
  pinned: boolean;
  state: ObjectState;
  updatedAt: string;
  metadata: Record<string, string>;
}

export interface ObjectDetail extends ObjectSummary {
  createdAt: string;
}

export interface ListObjectsRequest {
  prefix?: string;
  limit?: number;
  pageToken?: string;
}

export interface ListObjectsResponse {
  items: ObjectSummary[];
  nextPageToken?: string;
}
```

设计要求：

- 列表模型和详情模型围绕 metadata 组织
- 不在 `list` 响应中包含对象内容
- 所有状态字段与后端新模型直接对齐

## API 设计

### 前端 API 直接对齐新后端协议

既然允许 breaking change，前端不应再为旧对象 API 做兼容包装。

建议新增独立模块：

- `web-admin/src/api/objects.ts`

建议导出的方法：

- `listObjects`
- `headObject`
- `getObject`
- `putObject`
- `deleteObject`
- `retainObject`
- `releaseObject`
- `pinObject`
- `unpinObject`
- `getObjectStorageStatus`

### 明确拒绝的旧语义

前端新设计中，不再保留以下旧语义：

- `include_value`
- `include_values`
- `start_after`
- `add_ref`
- `remove_ref`

这些语义要从前端 API 层彻底删除，而不是在页面层规避使用。

### 查询与缓存策略

建议使用 React Query 统一管理对象域缓存。

建议 query key：

- `["objects", prefix, pageToken, limit]`
- `["object", key]`
- `["object-storage-status"]`

建议 mutation 成功后的刷新策略：

- `putObject` 失效对象列表和对象详情
- `deleteObject` 失效对象列表并清理详情缓存
- `retain/release/pin/unpin` 优先局部更新，再失效详情和列表

## 页面设计

### Objects 列表页

建议新增：

- `web-admin/src/features/objects/ObjectsPage.tsx`

页面职责：

- 按 prefix 查询对象
- 展示对象摘要列表
- 支持上传与对象级操作

推荐列表字段：

- `Key`
- `Size`
- `Checksum`
- `Refs`
- `Pinned`
- `State`
- `Updated At`

推荐操作：

- `Details`
- `Download`
- `Pin` / `Unpin`
- `Retain`
- `Release`
- `Delete`

设计原则：

- 列表只显示 metadata
- 不直接渲染对象内容
- 删除操作仅在允许时可点击，或点击后给出明确失败提示

### Objects 详情页

建议新增：

- `web-admin/src/features/objects/ObjectDetailPage.tsx`

页面结构建议分为三个区块：

1. Summary
2. Metadata
3. Actions

推荐展示内容：

- key
- size
- checksum
- ref count
- pinned
- state
- created at
- updated at
- metadata kv 列表

推荐操作：

- `Download`
- `Pin` / `Unpin`
- `Retain`
- `Release`
- `Delete`

设计要求：

- 默认只调用 `headObject`
- 下载或读取内容时才调用 `getObject`
- 不要在详情页加载时自动拉取大对象内容

### Upload 对话框

建议新增对象上传对话框，而不是继续复用旧文件上传交互。

推荐输入项：

- `Object key`
- `Local file`
- `Custom metadata`
- `Overwrite`

设计原则：

- 用户上传的是“对象”，而不是“文件条目”
- 上传成功后返回对象 key
- 支持在上传后跳转到对象详情页

## 任务创建页重构

### 目标

任务创建页应从“手写 URI + 选择 SMS 文件”的模型，改为“选择结构化对象资源”的模型。

### 推荐方向

不再把资源输入设计成主要依赖字符串 URI 的表单。

推荐将任务中的可执行资源建模为结构化数据，例如：

```ts
type ExecutableSource =
  | { kind: "none" }
  | { kind: "object"; key: string }
  | { kind: "url"; url: string }
  | { kind: "s3"; bucket: string; key: string }
  | { kind: "minio"; bucket: string; key: string };
```

如果后端任务协议也接受 breaking change，则建议直接升级为结构化资源模型，而不是继续传裸 URI。

### 任务表单改造

当前 Tasks 页中的：

- `Executable URI`
- `Choose Local`
- legacy 文件 URI 自动填充

都应被替换。

新的交互建议：

- `Executable Source Type`
  - `None`
  - `Object`
  - `URL`
  - `S3`
  - `MinIO`
- 当选择 `Object` 时：
  - 打开 `ObjectPickerModal`
  - 选择后回填对象 key
  - 页面显示对象摘要

### `ObjectPickerModal`

建议新增通用组件：

- `web-admin/src/features/objects/ObjectPickerModal.tsx`

该组件可复用于：

- Tasks 创建
- 未来其他依赖对象输入的表单

推荐能力：

- prefix 搜索
- 分页浏览
- 展示对象摘要
- 选择后返回 `ObjectSummary`

## 路由与页面结构

建议在 `AppShell` 中新增或替换路由：

- `/objects`
- `/objects/:key`

并移除：

- `/files`
- `/files/:id`

如需平滑迁移，可在过渡期对旧路由做前端重定向，但不建议长期保留。

## 模块组织建议

为了保持对象域代码边界清晰，建议新增以下模块：

- `web-admin/src/api/objects.ts`
- `web-admin/src/features/objects/ObjectsPage.tsx`
- `web-admin/src/features/objects/ObjectDetailPage.tsx`
- `web-admin/src/features/objects/ObjectPickerModal.tsx`
- `web-admin/src/features/objects/object-types.ts`
- `web-admin/src/features/objects/object-format.ts`

Tasks 页中与对象选择相关的部分应复用对象域组件，而不是复制一套本地选择逻辑。

## 交互设计原则

### 1. 元数据优先

前端默认围绕 metadata 交互：

- 列表只看 metadata
- 详情默认只看 metadata
- 内容拉取是显式动作

### 2. 对象操作即时反馈

对象操作成功后，UI 应立即反映：

- `Pin` / `Unpin` 更新标记
- `Retain` / `Release` 更新引用计数
- 删除后从列表移除或进入 `Deleting`

### 3. 不把后端实现细节暴露给用户

普通用户不需要知道对象在 RustFS 的内部路径、分片或 bucket 细节。

前端展示应以对象语义为中心，而不是底层 blob 存储细节。

### 4. 错误信息语义化

前端应将常见对象错误映射成清晰文案，例如：

- 对象不存在
- 对象正在删除
- 对象仍被引用，无法删除
- 对象已被 pin，无法删除
- 对象存储当前不可用

不要只展示通用的 `Request failed`。

## 状态与可观测性

前端建议额外增加对象域状态展示：

- 对象存储健康状态
- 后端连接错误提示
- `Deleting` 状态对象的可见标记

这有助于在对象系统发生后台重试或暂时性错误时，让用户理解当前状态，而不是误以为 UI 卡住。

## 测试策略

建议增加以下前端测试：

### 单元测试

- 对象状态格式化
- 对象操作按钮可用性判断
- `ObjectPickerModal` 选择逻辑

### 组件测试

- `ObjectsPage`
  - 查询、分页、操作按钮状态
- `ObjectDetailPage`
  - metadata 展示与操作触发
- Tasks 表单中的对象选择流程

### 端到端测试

- 上传对象并在列表可见
- 打开对象详情并下载
- retain / release / pin / unpin
- 任务创建时选择对象作为可执行资源

## 迁移方案

### Phase 1: 新对象模块落地

- 新增 `objects.ts`
- 新增对象列表页与详情页
- 在导航中增加 `Objects`

### Phase 2: Tasks 改造

- 用对象选择器替换文件选择器
- 去掉 legacy 文件 URI 主交互
- 将任务资源输入改成结构化对象选择

### Phase 3: Files 模块移除

- 删除 `FilesPage`
- 删除 `FileDetailPage`
- 删除 `api/files.ts`
- 删除与 legacy 文件 URI 相关的前端逻辑

### Phase 4: 文档与测试收敛

- 更新 Web Admin 使用指南
- 更新任务创建文档
- 更新 UI 测试

## 风险与权衡

### 优点

- 前后端对象模型一致
- 任务、对象浏览、对象操作共享统一资源语义
- 前端不再维护旧文件心智与兼容层
- 页面结构更清晰，后续扩展更自然

### 代价

- 导航、页面、任务创建表单都会发生 breaking change
- 现有用户对 `Files` 与 legacy 文件 URI 的使用习惯需要迁移
- 需要同步修改前端 API、后端协议与相关文档

### 判断

如果后端已经决定重构为新的对象系统，这些 breaking change 是值得的。继续保留 `Files + legacy 文件 URI` 只会让前端长期背负旧语义。

## 结论

推荐前端直接围绕新的对象系统重构：

- 用 `Objects` 替代 `Files`
- 用对象选择器替代本地文件选择器
- 用结构化对象引用替代 legacy 文件 URI
- 用新的对象 API 替代旧兼容语义

这套前端设计与新的 `ObjectCatalog + BlobStore + ObjectService` 架构直接对齐，边界清晰，长期更容易维护。
