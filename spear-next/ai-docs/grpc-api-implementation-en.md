# gRPC API Implementation for Task and ObjectRef Services

## Overview

This document describes the implementation of gRPC-based Task and ObjectRef services in the SMS (SPEAR Metadata Server) project. The implementation provides both gRPC and HTTP REST API interfaces for managing computational tasks and object references.

## Architecture

### Service Layer Architecture

The implementation follows a layered architecture:

```
HTTP REST API (Gateway)
    ↓
HTTP Handlers
    ↓
gRPC Services (TaskService, ObjectRefService)
    ↓
Storage Layer (KV Store)
```

### Key Components

1. **Proto Definitions** (`proto/sms/`)
   - `task.proto`: Defines Task-related messages and TaskService
   - `objectref.proto`: Defines ObjectRef-related messages and ObjectRefService
   - `sms.proto`: Main service definition combining all services

2. **Service Implementations** (`src/services/`)
   - `TaskService`: Manages task lifecycle (submit, list, get, stop, kill)
   - `ObjectRefService`: Manages object references (put, get, list, addref, removeref, pin, unpin)

3. **HTTP Gateway** (`src/http/`)
   - HTTP handlers that translate REST API calls to gRPC service calls
   - Route configuration for REST endpoints

## Task Service Implementation

### Core Operations

#### Submit Task
- **Endpoint**: `POST /api/v1/tasks`
- **gRPC**: `SubmitTask(SubmitTaskRequest) -> SubmitTaskResponse`
- **Functionality**: Creates and submits a new computational task

#### List Tasks
- **Endpoint**: `GET /api/v1/tasks`
- **gRPC**: `ListTasks(ListTasksRequest) -> ListTasksResponse`
- **Features**:
  - Pagination support (limit, offset)
  - Status filtering (pending, running, completed, failed, etc.)
  - Task type filtering
  - Node UUID filtering

#### Get Task
- **Endpoint**: `GET /api/v1/tasks/{task_id}`
- **gRPC**: `GetTask(GetTaskRequest) -> GetTaskResponse`
- **Functionality**: Retrieves detailed information about a specific task

#### Stop Task
- **Endpoint**: `POST /api/v1/tasks/{task_id}/stop`
- **gRPC**: `StopTask(StopTaskRequest) -> StopTaskResponse`
- **Functionality**: Gracefully stops a running task

#### Kill Task
- **Endpoint**: `POST /api/v1/tasks/{task_id}/kill`
- **gRPC**: `KillTask(KillTaskRequest) -> KillTaskResponse`
- **Functionality**: Forcefully terminates a running task

### Task Status Management

Tasks support the following status values:
- `UNKNOWN`: Default/uninitialized state
- `PENDING`: Task submitted but not yet started
- `RUNNING`: Task is currently executing
- `COMPLETED`: Task finished successfully
- `FAILED`: Task finished with an error
- `CANCELLED`: Task was cancelled by user
- `STOPPED`: Task was gracefully stopped
- `KILLED`: Task was forcefully terminated

## ObjectRef Service Implementation

### Core Operations

#### Put Object
- **Endpoint**: `POST /api/v1/objectrefs`
- **gRPC**: `PutObject(PutObjectRequest) -> PutObjectResponse`
- **Functionality**: Stores an object and returns a reference

#### Get Object
- **Endpoint**: `GET /api/v1/objectrefs/{object_id}`
- **gRPC**: `GetObject(GetObjectRequest) -> GetObjectResponse`
- **Functionality**: Retrieves an object by its reference

#### List Objects
- **Endpoint**: `GET /api/v1/objectrefs`
- **gRPC**: `ListObjects(ListObjectsRequest) -> ListObjectsResponse`
- **Features**:
  - Pagination support
  - Filtering by object type
  - Node UUID filtering

#### Reference Management
- **Add Reference**: `POST /api/v1/objectrefs/{object_id}/addref`
- **Remove Reference**: `POST /api/v1/objectrefs/{object_id}/removeref`
- **Pin Object**: `POST /api/v1/objectrefs/{object_id}/pin`
- **Unpin Object**: `POST /api/v1/objectrefs/{object_id}/unpin`

## HTTP Gateway Implementation

### Query Parameter Handling

The HTTP gateway properly handles query parameters for filtering and pagination:

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

### Error Handling

The implementation includes comprehensive error handling:
- gRPC status codes are mapped to appropriate HTTP status codes
- Detailed error messages are provided in JSON format
- Logging is implemented for debugging and monitoring

### Testing Strategy

The implementation includes comprehensive integration tests:
- Task lifecycle testing (submit, get, stop, kill)
- List operations with various filters
- Error handling scenarios
- Content type validation
- Sequential operations testing

## Configuration

### gRPC Server Setup

The gRPC server is configured to serve multiple services:

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

### HTTP Gateway Setup

The HTTP gateway is configured with proper routing:

```rust
let app = Router::new()
    .route("/api/v1/tasks", post(submit_task))
    .route("/api/v1/tasks", get(list_tasks))
    .route("/api/v1/tasks/:task_id", get(get_task))
    .route("/api/v1/tasks/:task_id/stop", post(stop_task))
    .route("/api/v1/tasks/:task_id/kill", post(kill_task))
    // ObjectRef routes...
    .with_state(gateway_state);
```

## Key Implementation Details

### Filter Handling

The implementation uses `-1` as a sentinel value to indicate "no filter":
- When no status filter is provided, `status_filter` is set to `-1`
- When no priority filter is provided, `priority_filter` is set to `-1`
- The service layer checks for `-1` to determine whether to apply filters

### Axum Query Parameter Integration

The HTTP gateway uses Axum's `Query` extractor with proper parameter handling:
- Query parameters are automatically deserialized into structs
- Optional parameters are handled gracefully
- Multiple query parameters are supported (e.g., `?limit=10&offset=0&status=pending`)

### Storage Integration

The services integrate with the existing KV store abstraction:
- Tasks and ObjectRefs are stored as JSON in the KV store
- Proper key prefixing is used to avoid conflicts
- TTL support is available for temporary objects

## Future Enhancements

1. **Streaming Support**: Add streaming endpoints for large result sets
2. **Authentication**: Implement authentication and authorization
3. **Metrics**: Add Prometheus metrics for monitoring
4. **Rate Limiting**: Implement rate limiting for API endpoints
5. **Caching**: Add caching layer for frequently accessed objects

## Conclusion

The gRPC API implementation provides a robust foundation for task and object management in the SMS system. The dual HTTP/gRPC interface ensures flexibility for different client types while maintaining consistency and performance.