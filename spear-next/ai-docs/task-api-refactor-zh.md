# Task API 重构文档

## 概述

本文档描述了 SPEAR-Next 项目中 Task API 的全面重构。重构将任务管理操作从复杂的生命周期模型简化为直观的注册模型。

## 变更内容

### 1. Proto 定义简化

**文件**: `proto/sms/task.proto`

**重构前**: 复杂的任务生命周期，包含 submit、stop、kill 操作
**重构后**: 简化的注册模型，包含 register、list、get、unregister 操作

主要变更:
- 移除了 `SubmitTaskRequest`、`StopTaskRequest`、`KillTaskRequest`
- 新增了 `RegisterTaskRequest`、`UnregisterTaskRequest`
- 简化任务状态，专注于注册状态
- 更新任务结构，包含 endpoint、version、capabilities、config 字段

### 2. 服务层重构

**文件**: `src/services/task.rs`

**变更内容**:
- 移除了 `submit_task`、`stop_task`、`kill_task` 方法
- 新增了 `register_task`、`unregister_task` 方法
- 简化任务存储模型
- 更新任务验证逻辑
- 保留 `list_tasks` 和 `get_task` 方法并更新逻辑

**核心特性**:
- 任务注册包含端点和能力信息
- 基于优先级的任务管理
- 简化状态管理（已注册/未注册）

### 3. HTTP 处理器更新

**文件**: `src/http/handlers/task.rs`

**变更内容**:
- 更新 `RegisterTaskParams` 结构
- 移除 submit/stop/kill 处理器
- 新增 unregister 处理器
- 更新响应结构
- 改进错误处理

**API 端点**:
- `POST /api/v1/tasks` - 注册新任务
- `GET /api/v1/tasks` - 列出任务（支持过滤）
- `GET /api/v1/tasks/:task_id` - 获取任务详情
- `DELETE /api/v1/tasks/:task_id` - 注销任务

### 4. 路由配置

**文件**: `src/http/routes.rs`

**变更内容**:
- 更新任务管理路由
- 移除冗余路由定义
- 简化路由结构

### 5. 集成测试更新

**文件**: `tests/task_integration_tests.rs`

**变更内容**:
- 更新测试数据生成
- 修改测试场景以使用新 API
- 修复优先级值映射
- 更新错误处理测试
- 所有测试现在都能成功通过

## API 使用示例

### 注册任务

```bash
curl -X POST http://localhost:8080/api/v1/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "description": "测试任务",
    "priority": "normal",
    "endpoint": "http://worker:8080/execute",
    "version": "1.0.0",
    "capabilities": ["compute", "storage"],
    "config": {
      "timeout": 300,
      "retries": 3
    }
  }'
```

### 列出任务

```bash
curl -X GET "http://localhost:8080/api/v1/tasks?status=registered&priority=normal"
```

### 获取任务详情

```bash
curl -X GET http://localhost:8080/api/v1/tasks/{task_id}
```

### 注销任务

```bash
curl -X DELETE http://localhost:8080/api/v1/tasks/{task_id}
```

## 优先级级别

系统支持以下优先级级别:
- `low` - 低优先级任务
- `normal` - 普通优先级任务（默认）
- `high` - 高优先级任务
- `urgent` - 紧急优先级任务

## 重构的好处

1. **简化 API**: 从生命周期管理模型简化为注册模型，降低复杂性
2. **更好的性能**: 移除不必要的状态转换
3. **更清晰的语义**: 基于注册的模型更加直观
4. **更容易测试**: 简化测试场景，提高测试覆盖率
5. **可维护性**: 更清洁的代码结构，降低复杂性

## 迁移说明

对于使用旧 API 的现有客户端:
- 将 `submit_task` 调用替换为 `register_task`
- 将 `stop_task` 和 `kill_task` 调用替换为 `unregister_task`
- 更新任务数据结构以包含新字段（endpoint、version、capabilities、config）
- 更新优先级值以使用小写字符串（normal、high 等）

## 测试

所有集成测试已更新并通过:
- `test_task_lifecycle` - 测试完整的任务注册和注销
- `test_task_list_with_filters` - 测试带各种过滤器的任务列表
- `test_task_error_handling` - 测试错误场景
- `test_task_sequential_operations` - 测试多个任务操作
- `test_task_content_types` - 测试不同内容类型

运行测试:
```bash
cargo test --test task_integration_tests
```