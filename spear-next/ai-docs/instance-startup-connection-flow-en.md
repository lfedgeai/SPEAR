# Instance Startup and Connection Establishment Flow

## Overview

This document describes the complete flow for instance startup and connection establishment in the Spear execution system. It covers the lifecycle from instance creation to successful communication channel establishment between spearlet and runtime instances.

## Architecture Components

### Core Components
- **ProcessRuntime**: Manages process-based runtime instances
- **ConnectionManager**: Handles TCP connections and authentication
- **TaskExecutionManager**: Coordinates task execution and instance management
- **MonitoringService**: Tracks connection events and performance metrics

### Communication Protocol
- **SpearMessage**: Unified message format for all communications
- **AuthRequest/AuthResponse**: Authentication handshake protocol
- **Secret Validation**: Token-based authentication mechanism

## Instance Startup Flow

### 1. Instance Creation Phase

```rust
// ProcessRuntime creates new instance
let instance = ProcessRuntime::create_instance(
    task_id,
    instance_config,
    resource_limits
).await?;
```

**Steps:**
1. **Resource Allocation**: Allocate CPU, memory, and network resources
2. **Port Assignment**: Find available port for communication
3. **Environment Setup**: Prepare environment variables and working directory
4. **Binary Preparation**: Ensure task binary is available and executable

### 2. Connection Manager Initialization

```rust
// Initialize connection manager with secret validator
let connection_manager = ConnectionManager::new_with_validator(
    config,
    Box::new(|secret: &str| -> bool {
        // Enhanced secret validation logic
        !secret.is_empty() && secret.len() >= 8
    })
);
```

**Features:**
- **Port Management**: Automatic port allocation and conflict resolution
- **Secret Validation**: Enhanced token-based authentication
- **Connection Pooling**: Efficient connection resource management
- **Event Monitoring**: Real-time connection event tracking

### 3. Process Launch

```rust
// Start process with communication parameters
let mut command = Command::new(&binary_path);
command
    .env("SPEAR_COMMUNICATION_PORT", port.to_string())
    .env("SPEAR_INSTANCE_ID", instance_id)
    .env("SPEAR_SECRET", secret)
    .env("SPEAR_COMMUNICATION_TYPE", "tcp");
```

**Environment Variables:**
- `SPEAR_COMMUNICATION_PORT`: TCP port for communication
- `SPEAR_INSTANCE_ID`: Unique instance identifier
- `SPEAR_SECRET`: Authentication token
- `SPEAR_COMMUNICATION_TYPE`: Communication protocol type

## Connection Establishment Flow

### 1. Server-Side Listening

```rust
// ConnectionManager starts listening
connection_manager.start_listening(port).await?;
```

**Process:**
1. **TCP Listener**: Bind to allocated port
2. **Event Handler**: Start connection event processing
3. **Authentication Setup**: Prepare secret validation
4. **Monitoring**: Initialize connection tracking

### 2. Client-Side Connection

The runtime instance initiates connection:

```rust
// Instance connects to spearlet
let stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
```

### 3. Authentication Handshake

#### Step 1: Authentication Request
```rust
let auth_request = AuthRequest {
    instance_id: "instance-123".to_string(),
    token: "secret-token".to_string(),
    client_version: "1.0.0".to_string(),
    client_type: "process".to_string(),
    extra_params: HashMap::new(),
};

let message = SpearMessage {
    message_type: MessageType::AuthRequest,
    request_id: 12345,
    timestamp: SystemTime::now(),
    payload: serde_json::to_vec(&auth_request)?,
    version: 1,
};
```

#### Step 2: Secret Validation
```rust
// Server validates the secret
let is_valid = secret_validator(&auth_request.token);
if !is_valid {
    return Err("Authentication failed");
}
```

#### Step 3: Authentication Response
```rust
let auth_response = AuthResponse {
    success: true,
    session_id: "session-456".to_string(),
    error_message: None,
};
```

### 4. Connection State Management

```rust
// Update connection state after successful authentication
let connection_state = ConnectionState {
    connection_id: "conn-789".to_string(),
    instance_id: Some("instance-123".to_string()),
    remote_addr: peer_addr,
    connected_at: Instant::now(),
    last_activity: Instant::now(),
    authenticated: true,
    client_type: Some("process".to_string()),
    client_version: Some("1.0.0".to_string()),
    session_id: Some("session-456".to_string()),
    status: ConnectionStatus::Active,
    heartbeat_sequence: 0,
};
```

## Event Flow and Monitoring

### Connection Events

1. **Connected Event**
```rust
ConnectionEvent::Connected {
    connection_id: "conn-789".to_string(),
    remote_addr: peer_addr,
}
```

2. **Authenticated Event**
```rust
ConnectionEvent::Authenticated {
    connection_id: "conn-789".to_string(),
    instance_id: "instance-123".to_string(),
    client_type: "process".to_string(),
}
```

3. **Message Events**
```rust
ConnectionEvent::MessageReceived {
    connection_id: "conn-789".to_string(),
    message: spear_message,
}
```

### Monitoring Integration

```rust
// Record connection events for monitoring
monitoring_service.record_connection_event(
    connection_id.clone(),
    ConnectionEvent::Connected { connection_id, remote_addr }
).await;

// Record message events
monitoring_service.record_message_event(
    connection_id,
    MessageType::AuthRequest,
    MessageDirection::Incoming,
    message_size,
    processing_time
).await;
```

## Error Handling and Recovery

### Connection Failures

1. **Port Conflicts**: Automatic port reallocation
2. **Authentication Failures**: Detailed error reporting
3. **Network Issues**: Connection retry mechanisms
4. **Timeout Handling**: Configurable timeout values

### Recovery Mechanisms

```rust
// Automatic reconnection logic
if connection_lost {
    for attempt in 1..=max_retries {
        match establish_connection().await {
            Ok(_) => break,
            Err(e) => {
                warn!("Connection attempt {} failed: {}", attempt, e);
                tokio::time::sleep(retry_delay).await;
            }
        }
    }
}
```

## Security Considerations

### Authentication Security
- **Token Validation**: Minimum length and complexity requirements
- **Session Management**: Unique session IDs for each connection
- **Timeout Protection**: Automatic session expiration

### Network Security
- **Local Binding**: Default to localhost for security
- **Port Isolation**: Instance-specific port allocation
- **Message Validation**: Protocol-level message verification

## Performance Optimization

### Connection Pooling
- **Reusable Connections**: Minimize connection overhead
- **Resource Limits**: Configurable connection limits
- **Load Balancing**: Distribute connections across instances

### Monitoring Optimization
- **Async Processing**: Non-blocking event processing
- **Batch Operations**: Efficient metric aggregation
- **Memory Management**: Bounded event queues

## Configuration

### ConnectionManagerConfig
```rust
pub struct ConnectionManagerConfig {
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub heartbeat_interval: Duration,
    pub max_message_size: usize,
    pub enable_monitoring: bool,
}
```

### MonitoringConfig
```rust
pub struct MonitoringConfig {
    pub enabled: bool,
    pub enable_connection_tracking: bool,
    pub enable_message_tracking: bool,
    pub enable_performance_tracking: bool,
    pub metrics_buffer_size: usize,
}
```

## Testing and Validation

### Unit Tests
- **Connection Manager**: Port allocation, authentication, state management
- **Monitoring Service**: Event recording, metric aggregation
- **Protocol Handling**: Message serialization, validation

### Integration Tests
- **End-to-End Flow**: Complete instance startup to communication
- **Error Scenarios**: Network failures, authentication errors
- **Performance Tests**: Connection throughput, latency measurements

## Conclusion

The instance startup and connection establishment flow provides a robust foundation for communication between spearlet and runtime instances. The implementation includes:

- **Reliable Connection Management**: Automatic port allocation and connection handling
- **Secure Authentication**: Token-based authentication with validation
- **Comprehensive Monitoring**: Real-time event tracking and performance metrics
- **Error Recovery**: Robust error handling and recovery mechanisms
- **Performance Optimization**: Efficient resource utilization and connection pooling

This architecture ensures reliable, secure, and performant communication channels for the Spear execution system.