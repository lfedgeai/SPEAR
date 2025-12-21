# Task API Refactoring Documentation

## Overview

This document describes the comprehensive refactoring of the Task API in the SPEAR-Next project. The refactoring simplified the task management operations from a complex lifecycle model to a straightforward registration-based model.

## Changes Made

### 1. Proto Definition Simplification

**File**: `proto/sms/task.proto`

**Before**: Complex task lifecycle with submit, stop, kill operations
**After**: Simplified registration model with register, list, get, unregister operations

Key changes:
- Removed `SubmitTaskRequest`, `StopTaskRequest`, `KillTaskRequest`
- Added `RegisterTaskRequest`, `UnregisterTaskRequest`
- Simplified task states to focus on registration status
- Updated task structure to include endpoint, version, capabilities, and config fields

### 2. Service Layer Refactoring

**File**: `src/services/task.rs`

**Changes**:
- Removed `submit_task`, `stop_task`, `kill_task` methods
- Added `register_task`, `unregister_task` methods
- Simplified task storage model
- Updated task validation logic
- Maintained `list_tasks` and `get_task` methods with updated logic

**Key Features**:
- Task registration with endpoint and capability information
- Priority-based task management
- Simplified state management (registered/unregistered)

### 3. HTTP Handlers Update

**File**: `src/http/handlers/task.rs`

**Changes**:
- Updated `RegisterTaskParams` structure
- Removed submit/stop/kill handlers
- Added unregister handler
- Updated response structures
- Improved error handling

**API Endpoints**:
- `POST /api/v1/tasks` - Register a new task
- `GET /api/v1/tasks` - List tasks with filtering
- `GET /api/v1/tasks/:task_id` - Get task details
- `DELETE /api/v1/tasks/:task_id` - Unregister a task

### 4. Route Configuration

**File**: `src/http/routes.rs`

**Changes**:
- Updated task management routes
- Removed redundant route definitions
- Simplified route structure

### 5. Integration Tests Update

**File**: `tests/task_integration_tests.rs`

**Changes**:
- Updated test data generation
- Modified test scenarios to use new API
- Fixed priority value mappings
- Updated error handling tests
- All tests now pass successfully

## API Usage Examples

### Register a Task

```bash
curl -X POST http://localhost:8080/api/v1/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Test task",
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

### List Tasks

```bash
curl -X GET "http://localhost:8080/api/v1/tasks?status=registered&priority=normal"
```

### Get Task Details

```bash
curl -X GET http://localhost:8080/api/v1/tasks/{task_id}
```

### Unregister a Task

```bash
curl -X DELETE http://localhost:8080/api/v1/tasks/{task_id}
```

## Priority Levels

The system supports the following priority levels:
- `low` - Low priority tasks
- `normal` - Normal priority tasks (default)
- `high` - High priority tasks
- `urgent` - Urgent priority tasks

## Benefits of Refactoring

1. **Simplified API**: Reduced complexity from lifecycle management to registration model
2. **Better Performance**: Removed unnecessary state transitions
3. **Clearer Semantics**: Registration-based model is more intuitive
4. **Easier Testing**: Simplified test scenarios and better test coverage
5. **Maintainability**: Cleaner code structure and reduced complexity

## Migration Notes

For existing clients using the old API:
- Replace `submit_task` calls with `register_task`
- Replace `stop_task` and `kill_task` calls with `unregister_task`
- Update task data structures to include new fields (endpoint, version, capabilities, config)
- Update priority values to use lowercase strings (normal, high, etc.)

## Testing

All integration tests have been updated and are passing:
- `test_task_lifecycle` - Tests complete task registration and unregistration
- `test_task_list_with_filters` - Tests task listing with various filters
- `test_task_error_handling` - Tests error scenarios
- `test_task_sequential_operations` - Tests multiple task operations
- `test_task_content_types` - Tests different content types

Run tests with:
```bash
cargo test --test task_integration_tests
```