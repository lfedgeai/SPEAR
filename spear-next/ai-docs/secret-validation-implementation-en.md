# Secret Validation Implementation

## Overview

This document describes the implementation of secret validation logic in the Spear execution system, specifically for authenticating connections between runtime processes and the connection manager.

## Architecture

### Components

1. **ConnectionManager** (`src/spearlet/execution/communication/connection_manager.rs`)
   - Manages WebSocket connections from runtime processes
   - Handles authentication requests using instance secrets
   - Supports both TaskExecutionManager-based and fallback validation

2. **TaskInstance** (`src/spearlet/execution/instance.rs`)
   - Contains `secret` field: `Arc<parking_lot::RwLock<Option<String>>>`
   - Stores the authentication secret for each task instance

3. **TaskExecutionManager** (`src/spearlet/execution/manager.rs`)
   - Provides `get_instance(instance_id)` method to retrieve TaskInstance
   - Central manager for task execution and instance lifecycle

## Implementation Details

### Secret Validation Flow

1. **Authentication Request**: Runtime process sends auth request with `instance_id` and `secret`
2. **Instance Lookup**: ConnectionManager attempts to find TaskInstance using TaskExecutionManager
3. **Secret Comparison**: Validates provided secret against stored instance secret
4. **Fallback Validation**: If TaskExecutionManager unavailable, uses basic secret validator

### Key Methods

#### `handle_auth_request_static`
```rust
pub fn handle_auth_request_static(
    connections: Arc<DashMap<String, Arc<ConnectionHandler>>>,
    instance_connections: Arc<DashMap<String, Vec<String>>>,
    secret_validator: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
    execution_manager: Option<Arc<TaskExecutionManager>>,
    msg: AuthRequest,
    connection_id: String,
) -> Result<AuthResponse, Box<dyn std::error::Error + Send + Sync>>
```

**Validation Logic:**
1. Try to get TaskInstance from execution_manager if available
2. Compare provided secret with instance secret
3. Fall back to secret_validator if execution_manager is None
4. Return appropriate AuthResponse

#### Constructor Methods
- `new_with_execution_manager`: Creates ConnectionManager with TaskExecutionManager reference
- `new_with_validator`: Creates ConnectionManager with custom secret validator (fallback)

### Security Features

1. **Instance-Specific Secrets**: Each TaskInstance has its own unique secret
2. **Secure Storage**: Secrets stored in thread-safe RwLock
3. **Validation Fallback**: Graceful degradation when TaskExecutionManager unavailable
4. **Basic Validation**: Minimum secret length and non-empty checks

## Integration Points

### ProcessRuntime Integration
- Uses `new_with_validator` with enhanced secret validation
- Implements basic security checks (length, non-empty)
- Maintains backward compatibility

### Future Enhancements
- Direct TaskExecutionManager integration in ProcessRuntime
- Enhanced secret generation and rotation
- Audit logging for authentication attempts

## Configuration

### ConnectionManager Setup
```rust
// With TaskExecutionManager (preferred)
let manager = ConnectionManager::new_with_execution_manager(config, execution_manager);

// With custom validator (fallback)
let validator = Box::new(|secret: &str| {
    !secret.is_empty() && secret.len() >= 8
});
let manager = ConnectionManager::new_with_validator(config, Some(validator));
```

## Error Handling

- Invalid instance_id: Returns authentication failure
- Missing secret: Returns authentication failure  
- Secret mismatch: Returns authentication failure
- System errors: Logged and return internal error response

## Testing Considerations

1. **Unit Tests**: Test secret validation logic with various scenarios
2. **Integration Tests**: Test full authentication flow
3. **Security Tests**: Test with invalid/malicious inputs
4. **Performance Tests**: Validate under concurrent connections

## Related Files

- `src/spearlet/execution/communication/connection_manager.rs`
- `src/spearlet/execution/instance.rs`
- `src/spearlet/execution/manager.rs`
- `src/spearlet/execution/runtime/process.rs`