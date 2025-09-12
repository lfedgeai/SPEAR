# Task和ObjectRef服务的gRPC API实现

## 概述

本文档描述了SMS（SPEAR元数据服务器）项目中基于gRPC的Task和ObjectRef服务的实现。该实现为管理计算任务和对象引用提供了gRPC和HTTP REST API接口。

## 架构

### 服务层架构

实现遵循分层架构：

```
HTTP REST API (网关)
    ↓
HTTP处理器
    ↓
gRPC服务 (TaskService, ObjectRefService)
    ↓
存储层 (KV存储)
```

### 核心组件

1. **Proto定义** (`proto/sms/`)
   - `task.proto`: 定义Task相关消息和TaskService
   - `objectref.proto`: 定义ObjectRef相关消息和ObjectRefService
   - `sms.proto`: 主服务定义，组合所有服务

2. **服务实现** (`src/services/`)
   - `TaskService`: 管理任务生命周期（submit, list, get, stop, kill）
   - `ObjectRefService`: 管理对象引用（put, get, list, addref, removeref, pin, unpin）

3. **HTTP网关** (`src/http/`)
   - 将REST API调用转换为gRPC服务调用的HTTP处理器
   - REST端点的路由配置

## Task服务实现

### 核心操作

#### 提交任务
- **端点**: `POST /api/v1/tasks`
- **gRPC**: `SubmitTask(SubmitTaskRequest) -> SubmitTaskResponse`
- **功能**: 创建并提交新的计算任务

#### 列出任务
- **端点**: `GET /api/v1/tasks`
- **gRPC**: `ListTasks(ListTasksRequest) -> ListTasksResponse`
- **特性**:
  - 分页支持（limit, offset）
  - 状态过滤（pending, running, completed, failed等）
  - 任务类型过滤
  - 节点UUID过滤

#### 获取任务
- **端点**: `GET /api/v1/tasks/{task_id}`
- **gRPC**: `GetTask(GetTaskRequest) -> GetTaskResponse`
- **功能**: 检索特定任务的详细信息

#### 停止任务
- **端点**: `POST /api/v1/tasks/{task_id}/stop`
- **gRPC**: `StopTask(StopTaskRequest) -> StopTaskResponse`
- **功能**: 优雅地停止正在运行的任务

#### 杀死任务
- **端点**: `POST /api/v1/tasks/{task_id}/kill`
- **gRPC**: `KillTask(KillTaskRequest) -> KillTaskResponse`
- **功能**: 强制终止正在运行的任务

### 任务状态管理

任务支持以下状态值：
- `UNKNOWN`: 默认/未初始化状态
- `PENDING`: 任务已提交但尚未开始
- `RUNNING`: 任务正在执行
- `COMPLETED`: 任务成功完成
- `FAILED`: 任务执行失败
- `CANCELLED`: 任务被用户取消
- `STOPPED`: 任务被优雅停止
- `KILLED`: 任务被强制终止

## ObjectRef服务实现

### 核心操作

#### 存储对象
- **端点**: `POST /api/v1/objectrefs`
- **gRPC**: `PutObject(PutObjectRequest) -> PutObjectResponse`
- **功能**: 存储对象并返回引用

#### 获取对象
- **端点**: `GET /api/v1/objectrefs/{object_id}`
- **gRPC**: `GetObject(GetObjectRequest) -> GetObjectResponse`
- **功能**: 通过引用检索对象

#### 列出对象
- **端点**: `GET /api/v1/objectrefs`
- **gRPC**: `ListObjects(ListObjectsRequest) -> ListObjectsResponse`
- **特性**:
  - 分页支持
  - 按对象类型过滤
  - 节点UUID过滤

#### 引用管理
- **添加引用**: `POST /api/v1/objectrefs/{object_id}/addref`
- **移除引用**: `POST /api/v1/objectrefs/{object_id}/removeref`
- **固定对象**: `POST /api/v1/objectrefs/{object_id}/pin`
- **取消固定**: `POST /api/v1/objectrefs/{object_id}/unpin`

## HTTP网关实现

### 查询参数处理

HTTP网关正确处理过滤和分页的查询参数：

```rust
#[derive(Debug, Deserialize)]
pub struct ListTasksParams {
    pub node_uuid: Option<String>,
    pub status: Option<String>,
    pub task_type: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}
```

### 错误处理

实现包含全面的错误处理：
- gRPC状态码映射到适当的HTTP状态码
- 以JSON格式提供详细的错误消息
- 实现日志记录用于调试和监控

### 测试策略

实现包含全面的集成测试：
- 任务生命周期测试（submit, get, stop, kill）
- 使用各种过滤器的列表操作
- 错误处理场景
- 内容类型验证
- 顺序操作测试

## 配置

### gRPC服务器设置

gRPC服务器配置为服务多个服务：

```rust
let sms_service = SmsServiceImpl::with_kv_config(ttl_seconds, kv_config).await;
let task_service = TaskServiceServer::new(sms_service.clone());
let objectref_service = ObjectRefServiceServer::new(sms_service.clone());

Server::builder()
    .add_service(SmsServiceServer::new(sms_service))
    .add_service(task_service)
    .add_service(objectref_service)
    .serve(addr)
    .await?;
```

### HTTP网关设置

HTTP网关配置了适当的路由：

```rust
let app = Router::new()
    .route("/api/v1/tasks", post(submit_task))
    .route("/api/v1/tasks", get(list_tasks))
    .route("/api/v1/tasks/:task_id", get(get_task))
    .route("/api/v1/tasks/:task_id/stop", post(stop_task))
    .route("/api/v1/tasks/:task_id/kill", post(kill_task))
    // ObjectRef路由...
    .with_state(gateway_state);
```

## 关键实现细节

### 过滤器处理

实现使用`-1`作为哨兵值来表示"无过滤器"：
- 当未提供状态过滤器时，`status_filter`设置为`-1`
- 当未提供优先级过滤器时，`priority_filter`设置为`-1`
- 服务层检查`-1`以确定是否应用过滤器

### Axum查询参数集成

HTTP网关使用Axum的`Query`提取器进行适当的参数处理：
- 查询参数自动反序列化为结构体
- 可选参数得到优雅处理
- 支持多个查询参数（例如，`?limit=10&offset=0&status=pending`）

### 存储集成

服务与现有的KV存储抽象集成：
- Task和ObjectRef作为JSON存储在KV存储中
- 使用适当的键前缀以避免冲突
- 为临时对象提供TTL支持

## 未来增强

1. **流式支持**: 为大型结果集添加流式端点
2. **身份验证**: 实现身份验证和授权
3. **指标**: 添加Prometheus指标用于监控
4. **速率限制**: 为API端点实现速率限制
5. **缓存**: 为频繁访问的对象添加缓存层

## 结论

gRPC API实现为SMS系统中的任务和对象管理提供了强大的基础。双HTTP/gRPC接口确保了不同客户端类型的灵活性，同时保持一致性和性能。