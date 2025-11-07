# Spearlet Function Invocation Logic Flow Design

## Overview

This document provides a detailed description of the logic processing flow for the `InvokeFunction` interface in Spearlet, with emphasis on the distinction logic between new task creation and existing task invocation.

## Core Logic Flow

### 1. Request Preprocessing Phase

```
InvokeFunctionRequest Received
    ↓
Parameter Validation and Normalization
    ↓
Route Processing Based on invocation_type
    ├── INVOCATION_TYPE_NEW_TASK → New Task Creation Flow
    ├── INVOCATION_TYPE_EXISTING_TASK → Existing Task Invocation Flow
    └── INVOCATION_TYPE_UNKNOWN → Return Error
```

### 2. New Task Creation Flow (INVOCATION_TYPE_NEW_TASK)

```
Start New Task Creation
    ↓
Validate Required Parameters
├── task_name (required)
├── artifact_spec (required)
└── function_name (required)
    ↓
Check if Task Already Exists
├── Exists → Handle Based on Policy
│   ├── If force_new_instance=true → Create New Instance
│   └── Otherwise → Use Existing Task
└── Not Exists → Continue Creation Flow
    ↓
Artifact Processing
├── Download Artifact (if remote location)
├── Verify Artifact Integrity (checksum)
├── Parse Artifact Metadata
└── Cache Artifact Locally
    ↓
Task Instance Creation
├── Assign Unique task_id
├── Create Task Runtime Environment
├── Load Artifact into Runtime
└── Initialize Task Instance
    ↓
Optional: Register to SMS
├── If SMS Integration Configured
└── Send RegisterTask Request
    ↓
Execute Function Call → Go to Execution Phase
```

### 3. Existing Task Invocation Flow (INVOCATION_TYPE_EXISTING_TASK)

```
Start Existing Task Invocation
    ↓
Validate Required Parameters
├── task_id (required)
└── function_name (required)
    ↓
Find Task Instance
├── Search in Local Task Pool
├── If Not Found and SMS Configured
│   └── Query Task Info from SMS
└── If Still Not Found
    ↓
Handle Task Not Found
├── If create_if_not_exists=true
│   ├── Check if Sufficient Info to Create Task
│   ├── If Has artifact_spec → Create New Task
│   └── Otherwise → Return Error
└── Otherwise → Return Task Not Found Error
    ↓
Task Instance Acquisition/Creation
├── If Task Exists but No Available Instance
│   ├── Get Instance from Instance Pool
│   ├── If No Instance in Pool → Create New Instance
│   └── If force_new_instance=true → Force Create New Instance
└── If Available Instance → Use Directly
    ↓
Execute Function Call → Go to Execution Phase
```

### 4. Function Execution Phase

```
Prepare Execution Environment
├── Set Execution Context
├── Configure Environment Variables
├── Set Timeout and Retry Parameters
└── Generate Unique execution_id
    ↓
Route Based on Execution Mode
├── EXECUTION_MODE_SYNC → Synchronous Execution
├── EXECUTION_MODE_ASYNC → Asynchronous Execution
└── EXECUTION_MODE_STREAM → Streaming Execution
    ↓
Execute Function Call
├── Invoke Specified Function on Task Instance
├── Pass Parameters
├── Monitor Execution Status
└── Collect Execution Metrics
    ↓
Process Execution Result
├── Success → Return Result
├── Failure → Error Handling and Retry
└── Timeout → Cancel Execution and Return Timeout Error
    ↓
Cleanup and Resource Reclamation
├── Update Instance Status
├── Record Execution Logs
└── Release Resources (if needed)
```

## Detailed Logic Implementation Pseudocode

### Main Processing Function

```rust
async fn invoke_function(request: InvokeFunctionRequest) -> Result<InvokeFunctionResponse> {
    // 1. Parameter validation
    validate_request(&request)?;
    
    // 2. Route based on invocation type
    let task_instance = match request.invocation_type {
        InvocationType::NewTask => {
            handle_new_task_creation(&request).await?
        },
        InvocationType::ExistingTask => {
            handle_existing_task_invocation(&request).await?
        },
        _ => return Err("Invalid invocation type"),
    };
    
    // 3. Execute function call
    let execution_result = execute_function(
        &task_instance,
        &request.function_name,
        &request.parameters,
        &request.execution_mode,
        &request.context,
    ).await?;
    
    // 4. Construct response
    Ok(InvokeFunctionResponse {
        success: true,
        execution_id: execution_result.execution_id,
        task_id: task_instance.task_id,
        instance_id: task_instance.instance_id,
        result: execution_result.result,
        // ... other fields
    })
}
```

### New Task Creation Handler

```rust
async fn handle_new_task_creation(request: &InvokeFunctionRequest) -> Result<TaskInstance> {
    // Validate required parameters for new task creation
    if request.task_name.is_empty() || request.artifact_spec.is_none() {
        return Err("Missing required parameters for new task creation");
    }
    
    let task_name = &request.task_name;
    let artifact_spec = request.artifact_spec.as_ref().unwrap();
    
    // Check if task already exists
    if let Some(existing_task) = task_manager.find_task_by_name(task_name) {
        if !request.force_new_instance {
            // Use existing task
            return task_manager.get_or_create_instance(&existing_task.task_id).await;
        }
    }
    
    // Process artifact
    let artifact = artifact_manager.download_and_verify_artifact(artifact_spec).await?;
    
    // Create new task
    let task_id = generate_unique_task_id();
    let task = Task {
        task_id: task_id.clone(),
        name: task_name.clone(),
        description: request.task_description.clone(),
        artifact_spec: artifact_spec.clone(),
        status: TaskStatus::Active,
        created_at: current_timestamp(),
        // ... other fields
    };
    
    // Register task
    task_manager.register_task(task).await?;
    
    // Optional: Register to SMS
    if config.sms_integration_enabled {
        sms_client.register_task(&task).await?;
    }
    
    // Create task instance
    let instance = task_manager.create_instance(&task_id, &artifact).await?;
    
    Ok(instance)
}
```

### Existing Task Invocation Handler

```rust
async fn handle_existing_task_invocation(request: &InvokeFunctionRequest) -> Result<TaskInstance> {
    // Validate required parameters for existing task invocation
    if request.task_id.is_empty() {
        return Err("Missing task_id for existing task invocation");
    }
    
    let task_id = &request.task_id;
    
    // Find task
    let task = match task_manager.find_task_by_id(task_id) {
        Some(task) => task,
        None => {
            // Task not found, check if can create
            if request.create_if_not_exists {
                if let Some(artifact_spec) = &request.artifact_spec {
                    // Have sufficient info to create new task
                    return handle_new_task_creation_from_existing_request(request).await;
                } else {
                    return Err("Cannot create task without artifact specification");
                }
            } else {
                return Err("Task not found and create_if_not_exists is false");
            }
        }
    };
    
    // Get or create task instance
    let instance = if request.force_new_instance {
        task_manager.create_new_instance(&task.task_id).await?
    } else {
        task_manager.get_or_create_instance(&task.task_id).await?
    };
    
    Ok(instance)
}
```

### Function Execution Handler

```rust
async fn execute_function(
    instance: &TaskInstance,
    function_name: &str,
    parameters: &[FunctionParameter],
    execution_mode: &ExecutionMode,
    context: &ExecutionContext,
) -> Result<ExecutionResult> {
    let execution_id = generate_execution_id();
    
    // Set execution context
    let exec_context = ExecutionContext {
        execution_id: execution_id.clone(),
        timeout_ms: context.timeout_ms,
        max_retries: context.max_retries,
        environment: context.environment.clone(),
        // ... other fields
    };
    
    match execution_mode {
        ExecutionMode::Sync => {
            // Synchronous execution
            let result = instance.invoke_function_sync(
                function_name,
                parameters,
                &exec_context,
            ).await?;
            
            Ok(ExecutionResult {
                status: ExecutionStatus::Completed,
                result: Some(result),
                execution_id,
                // ... other fields
            })
        },
        ExecutionMode::Async => {
            // Asynchronous execution
            let execution_handle = instance.invoke_function_async(
                function_name,
                parameters,
                &exec_context,
            ).await?;
            
            // Store execution handle for later queries
            execution_manager.store_execution(execution_id.clone(), execution_handle);
            
            Ok(ExecutionResult {
                status: ExecutionStatus::Pending,
                execution_id,
                // ... other fields
            })
        },
        ExecutionMode::Stream => {
            // Streaming execution (handled by separate StreamFunction RPC)
            Err("Streaming mode should use StreamFunction RPC")
        },
        _ => Err("Unsupported execution mode"),
    }
}
```

## Error Handling Strategies

### 1. Parameter Validation Errors
```rust
fn validate_request(request: &InvokeFunctionRequest) -> Result<()> {
    match request.invocation_type {
        InvocationType::NewTask => {
            if request.task_name.is_empty() {
                return Err("task_name is required for new task creation");
            }
            if request.artifact_spec.is_none() {
                return Err("artifact_spec is required for new task creation");
            }
        },
        InvocationType::ExistingTask => {
            if request.task_id.is_empty() {
                return Err("task_id is required for existing task invocation");
            }
        },
        _ => return Err("Invalid invocation_type"),
    }
    
    if request.function_name.is_empty() {
        return Err("function_name is required");
    }
    
    Ok(())
}
```

### 2. Artifact Processing Errors
- Download Failure: Retry mechanism, return detailed error info on final failure
- Verification Failure: Immediate failure, no retry
- Parse Failure: Check artifact format, return format error info

### 3. Task Creation Errors
- Resource Shortage: Return resource shortage error, suggest retry later
- Permission Denied: Return permission error, suggest checking permission config
- Network Error: Retry mechanism, record detailed network error info

### 4. Execution Errors
- Function Not Found: Return function not found error
- Parameter Error: Return parameter validation error
- Execution Timeout: Cancel execution, return timeout error
- Runtime Error: Catch exception, return detailed error info

## Performance Optimization Considerations

### 1. Task Lookup Optimization
- Use memory cache to accelerate task lookup
- Implement task indexing (by name, ID, tags, etc.)
- Periodically clean expired task cache

### 2. Instance Pool Management
- Pre-warm instances for frequently used tasks
- Implement instance reuse strategies
- Dynamically adjust instance pool size

### 3. Artifact Caching
- Local artifact caching mechanism
- Artifact version management
- Cache cleanup strategies

### 4. Concurrency Control
- Limit concurrent task creation count
- Implement task creation queue
- Resource contention detection and avoidance

## Monitoring and Observability

### 1. Key Metrics
- Task creation success rate and latency
- Function invocation success rate and latency
- Instance pool utilization
- Artifact cache hit rate

### 2. Logging
- Complete request processing chain logs
- Error details and stack traces
- Performance metrics recording

### 3. Distributed Tracing
- Use trace_id to correlate entire call chain
- Record timestamps of key processing nodes
- Support cross-service trace tracking

## Summary

This logic flow design provides:

1. **Clear Processing Routing** - Distinct processing logic based on invocation type
2. **Flexible Task Management** - Support for both new task creation and existing task invocation
3. **Comprehensive Error Handling** - Coverage of various exception scenarios
4. **Efficient Resource Utilization** - Instance reuse and artifact caching
5. **Complete Observability** - Monitoring, logging, and tracing

This design ensures the reliability, performance, and maintainability of the `InvokeFunction` interface.