# SPEARlet Swagger UI 实现文档

## 概述

本文档描述了 SPEARlet HTTP 网关的 Swagger UI 实现和使用方法，提供了类似于 SMS 模块的交互式 API 文档功能。

## 实现详情

### 新增组件

1. **增强的 API 文档函数** (`api_docs`)
   - 完整的 OpenAPI 3.0.0 规范
   - 所有 API 端点的详细描述
   - 支持多语言（中英文）
   - 正确的 HTTP 状态码和响应模式

2. **Swagger UI HTML 页面** (`swagger_ui`)
   - 用于 API 探索的交互式 Web 界面
   - 基于 CDN 的 Swagger UI 资源 (v5.9.0)
   - 自定义样式和品牌
   - 适应不同屏幕尺寸的响应式设计

3. **新的 HTTP 路由**
   - `/api/openapi.json` - OpenAPI 规范端点
   - `/swagger-ui` - 主要的 Swagger UI 界面
   - `/docs` - Swagger UI 的替代访问路径

### 配置

Swagger UI 功能由 HTTP 设置中的 `swagger_enabled` 配置选项控制：

```toml
[http]
swagger_enabled = true
```

启用后，以下路由将可用：
- 原有的 `/api-docs` 路由（JSON 格式）
- 新的 `/api/openapi.json` 路由（增强的 OpenAPI 规范）
- 新的 `/swagger-ui` 和 `/docs` 路由（交互式 UI）

## API 端点文档

Swagger UI 包含以下内容的完整文档：

### 健康状态检查
- `GET /health` - 基本健康检查
- `GET /status` - 详细的节点状态信息

### 对象存储
- `PUT /objects/{key}` - 存储对象数据
- `GET /objects/{key}` - 检索对象数据
- `DELETE /objects/{key}` - 删除对象数据
- `GET /objects` - 列出所有对象

### 引用管理
- `POST /objects/{key}/refs` - 添加对象引用
- `DELETE /objects/{key}/refs` - 移除对象引用

### 固定操作
- `POST /objects/{key}/pin` - 固定对象以防止清理
- `DELETE /objects/{key}/pin` - 取消固定对象

### 函数执行
- `POST /functions/invoke` - 调用函数并指定参数
- `GET /functions/executions/{execution_id}/status` - 获取执行状态
- `POST /functions/executions/{execution_id}/cancel` - 取消函数执行
- `GET /functions/stream` - 流式获取函数执行结果

### 任务管理
- `GET /tasks` - 列出所有任务（支持可选过滤）
- `GET /tasks/{task_id}` - 获取详细任务信息
- `DELETE /tasks/{task_id}` - 删除指定任务
- `GET /tasks/{task_id}/executions` - 获取任务的执行历史

### 监控与统计
- `GET /functions/health` - 获取函数服务健康状态
- `GET /functions/stats` - 获取函数服务综合统计信息

## 使用说明

### 访问 Swagger UI

1. **启动 SPEARlet** 并启用 Swagger：
   ```bash
   cargo run --bin spearlet
   ```

2. **在浏览器中打开 Swagger UI**：
   - 主要 URL：`http://localhost:8081/swagger-ui`
   - 替代 URL：`http://localhost:8081/docs`

3. **访问 OpenAPI 规范**：
   - JSON 格式：`http://localhost:8081/api/openapi.json`
   - 传统格式：`http://localhost:8081/api-docs`

### 使用界面

1. **浏览 API 端点**：所有端点按标签组织（health、objects、references、pinning）

2. **尝试 API 调用**：点击任何端点上的"Try it out"来：
   - 填写必需的参数
   - 直接从浏览器执行请求
   - 查看响应数据和状态码

3. **查看模式**：展开响应模式以了解数据结构

4. **下载规范**：使用 OpenAPI JSON URL 导入到其他工具

## 技术实现

### 文件修改

- **`spearlet/http_gateway.rs`**：
  - 添加了 `Html` 和 `IntoResponse` 导入
  - 增强了 `api_docs()` 函数，包含完整的 OpenAPI 规范
  - 添加了返回 HTML 页面的 `swagger_ui()` 函数
  - 更新了路由配置以包含新端点

### 依赖项

实现使用了：
- **Axum** 用于 HTTP 路由和响应
- **Swagger UI** (v5.9.0) 通过 CDN 提供交互式界面
- **OpenAPI 3.0.0** 规范格式

### 安全考虑

- 所有资源从可信 CDN (unpkg.com) 加载
- API 文档中不暴露敏感信息
- 可通过配置禁用 Swagger UI

## 与 SMS 模块的比较

SPEARlet Swagger UI 实现遵循与 SMS 模块相同的模式：

| 功能 | SMS | SPEARlet |
|------|-----|----------|
| OpenAPI 规范 | ✅ | ✅ |
| 交互式 UI | ✅ | ✅ |
| 多个路由 | ✅ | ✅ |
| 配置控制 | ✅ | ✅ |
| 双语支持 | ✅ | ✅ |

## 未来增强

Swagger UI 实现的潜在改进：

1. **自定义主题**：添加 SPEARlet 特定的品牌和颜色
2. **身份验证**：如果添加了身份验证机制，则与之集成
3. **示例**：添加更全面的请求/响应示例
4. **验证**：包含请求验证模式
5. **离线模式**：本地打包 Swagger UI 资源

## 故障排除

### 常见问题

1. **Swagger UI 无法加载**：
   - 验证配置中 `swagger_enabled = true`
   - 检查 SPEARlet 是否在正确端口上运行
   - 确保网络连接到 CDN 资源

2. **API 调用失败**：
   - 验证 SPEARlet gRPC 服务器正在运行
   - 检查 HTTP 网关连接状态
   - 查看服务器日志中的错误消息

3. **缺少端点**：
   - 确保使用正确的 URL（`/swagger-ui` 或 `/docs`）
   - 刷新页面以重新加载 OpenAPI 规范
   - 检查浏览器控制台中的 JavaScript 错误

### 日志和调试

监控 SPEARlet 日志中的：
- HTTP 网关启动消息
- gRPC 连接状态
- 请求处理错误
- 配置加载问题

## 结论

SPEARlet Swagger UI 实现提供了一个全面的、交互式的 API 文档系统，与 SMS 模块中可用的功能相匹配。用户现在可以通过现代 Web 界面轻松探索和测试 SPEARlet API。