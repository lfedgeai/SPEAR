# 对象存储管理台 API

本文档说明当前 SMS Web Admin 为对象域提供的管理台接口。

这组接口是对象系统前端化改造的第一阶段实现，目标是：

- 让前端围绕 `Objects` 域工作
- 提供独立于旧 `files` 页面语义的管理台接口
- 在底层对象后端尚未完全重构前，先通过兼容层复用现有文件存储

## 设计定位

当前这组 API 是“对象管理台兼容层”，而不是最终版对象服务协议。

当前特征：

- 路径使用 `/admin/api/objects*`
- 返回模型已经对齐前端对象域
- 底层仍复用现有文件存储目录
- `key` 当前等于兼容层文件 ID
- `state/ref_count/pinned` 目前提供的是兼容默认值

## 路由

### 列出对象

- `GET /admin/api/objects`

查询参数：

- `prefix`
- `limit`
- `page_token`

响应示例：

```json
{
  "items": [
    {
      "key": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
      "name": "tool.wasm",
      "size": 12345,
      "updated_at": 1756656000,
      "state": "ready",
      "ref_count": 1,
      "pinned": false
    }
  ],
  "total_count": 1,
  "next_page_token": null
}
```

### 获取对象 metadata

- `GET /admin/api/objects/{key}`

响应示例：

```json
{
  "found": true,
  "key": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
  "size": 12345,
  "state": "ready",
  "ref_count": 1,
  "pinned": false,
  "name": "tool.wasm",
  "created_at": 1756656000,
  "updated_at": 1756656000
}
```

### 下载对象内容

- `GET /admin/api/objects/{key}/content`

返回对象二进制内容。

### 上传对象

- `POST /admin/api/objects`

请求体：

- 直接上传二进制内容
- 可通过 header 传递：
  - `Content-Type`
  - `x-file-name`

响应示例：

```json
{
  "success": true,
  "id": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
  "key": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
  "name": "tool.wasm",
  "uri": "object://8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d"
}
```

### 删除对象

- `DELETE /admin/api/objects/{key}`

响应示例：

```json
{
  "success": true
}
```

删除规则：

- `ref_count > 0` 时拒绝删除
- `pinned = true` 时拒绝删除

### Retain 对象

- `POST /admin/api/objects/{key}/retain`

响应示例：

```json
{
  "success": true,
  "key": "8d4b8f8d-0f33-4f82-a8e0-5b4e0f8b6f1d",
  "ref_count": 1,
  "pinned": false,
  "state": "ready"
}
```

### Release 对象

- `POST /admin/api/objects/{key}/release`

当引用计数已经为 `0` 时会返回失败消息。

### Pin 对象

- `POST /admin/api/objects/{key}/pin`

### Unpin 对象

- `POST /admin/api/objects/{key}/unpin`

### 获取对象存储状态

- `GET /admin/api/object-storage/status`

响应示例：

```json
{
  "backend": "local-fs-blobstore",
  "healthy": true,
  "message": "Using LocalFsBlobStore compatibility backend rooted at ./data/objects"
}
```

## 当前接口边界

当前对象管理台接口已经收敛为：

- `/admin/api/objects*` 是新的前端对象域接口
- 底层已经抽成 `BlobStore`，当前已经同时支持 `LocalFsBlobStore` 与 `RustFsBlobStore`

因此当前已经进入“协议对象化且由稳定 BlobStore 边界承载”的阶段，本地兼容存储与 RustFS 之间的切换不再需要重写对象 API handler。

## 当前限制

当前实现仍有以下限制：

- `key` 还不是最终的业务对象键，只是兼容层 ID
- `ref_count`、`pinned`、`state` 还没有接入真正的对象生命周期后端
- `retain/release/pin/unpin` 已经写入兼容 sidecar，但仍不是最终的 `ObjectCatalog`
- 任务创建页已经改为提交结构化对象引用 `{ "kind": "object", "object_key": "<key>" }`

## 后续演进

下一阶段建议：

1. 让 SMS 或 SPEARlet 提供真正的对象目录后端
2. 让对象 `key` 脱离兼容文件 ID
3. 为 `retain/release/pin/unpin` 增加真实 API
4. 让任务协议进一步从裸 URI 演进到结构化对象引用

## 相关文档

- [object-storage-frontend-design-zh.md](./object-storage-frontend-design-zh.md)
- [rustfs-object-storage-design-zh.md](./rustfs-object-storage-design-zh.md)
- [web-admin-ui-guide-zh.md](./web-admin-ui-guide-zh.md)
