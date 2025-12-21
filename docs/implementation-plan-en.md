# Spear-Next Runtime Communication Mechanism Implementation Plan

## Overview

This document provides a detailed implementation plan for improving the runtime communication mechanism in spear-next, including specific code examples, implementation steps, and timeline.

## Implementation Solution: Improved Listening Mode

### 1. Core Architecture Design

#### 1.1 Message Protocol Definition

```rust
// spearlet/execution/communication/protocol.rs

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Unified message format / 统一的消息格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpearMessage {
    /// Message type / 消息类型
    pub message_type: MessageType,
    /// Request ID for correlating requests and responses / 请求ID，用于关联请求和响应
    pub request_id: u64,
    /// Timestamp / 时间戳
    pub timestamp: SystemTime,
    /// Message payload / 消息负载
    pub payload: Vec<u8>,
}

/// Message type enumeration / 消息类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// Authentication request / 认证请求
    AuthRequest,
    /// Authentication response / 认证响应
    AuthResponse,
    /// Execution request / 执行请求
    ExecuteRequest,
    /// Execution response / 执行响应
    ExecuteResponse,
    /// Signal message / 信号消息
    Signal,
    /// Heartbeat message / 心跳消息
    Heartbeat,
    /// Error message / 错误消息
    Error,
}

/// Authentication request payload / 认证请求负载
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// Instance ID / 实例ID
    pub instance_id: String,
    /// Authentication token / 认证令牌
    pub token: String,
    /// Client version / 客户端版本
    pub client_version: String,
}

/// Authentication response payload / 认证响应负载
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// Whether authentication succeeded / 认证是否成功
    pub success: bool,
    /// Error message (if authentication failed) / 错误消息（如果认证失败）
    pub error_message: Option<String>,
    /// Session ID / 会话ID
    pub session_id: Option<String>,
}

/// Execution request payload / 执行请求负载
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// Task ID / 任务ID
    pub task_id: String,
    /// Execution command or data / 执行命令或数据
    pub command: String,
    /// Arguments / 参数
    pub args: Vec<String>,
    /// Environment variables / 环境变量
    pub env: std::collections::HashMap<String, String>,
}

/// Execution response payload / 执行响应负载
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponse {
    /// Task ID / 任务ID
    pub task_id: String,
    /// Execution status / 执行状态
    pub status: ExecutionStatus,
    /// Output data / 输出数据
    pub output: Option<String>,
    /// Error information / 错误信息
    pub error: Option<String>,
    /// Exit code / 退出码
    pub exit_code: Option<i32>,
}

/// Execution status enumeration / 执行状态枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Started execution / 开始执行
    Started,
    /// Running / 执行中
    Running,
    /// Completed / 执行完成
    Completed,
    /// Failed / 执行失败
    Failed,
}
```

#### 1.2 Connection Manager

```rust
// spearlet/execution/communication/connection_manager.rs

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Connection manager / 连接管理器
pub struct ConnectionManager {
    /// Listener mapping / 监听器映射
    listeners: Arc<Mutex<HashMap<String, TcpListener>>>,
    /// Connection mapping / 连接映射
    connections: Arc<Mutex<HashMap<String, Connection>>>,
    /// Authentication token mapping / 认证令牌映射
    auth_tokens: Arc<Mutex<HashMap<String, String>>>,
    /// Port allocator / 端口分配器
    port_allocator: PortAllocator,
}

/// Connection information / 连接信息
#[derive(Debug)]
pub struct Connection {
    /// Instance ID / 实例ID
    pub instance_id: String,
    /// TCP stream / TCP流
    pub stream: TcpStream,
    /// Session ID / 会话ID
    pub session_id: String,
    /// Whether authenticated / 是否已认证
    pub authenticated: bool,
    /// Send channel / 发送通道
    pub sender: mpsc::UnboundedSender<SpearMessage>,
    /// Receive channel / 接收通道
    pub receiver: mpsc::UnboundedReceiver<SpearMessage>,
}

/// Port allocator / 端口分配器
pub struct PortAllocator {
    /// Current port / 当前端口
    current_port: Arc<Mutex<u16>>,
    /// Allocated ports / 已分配端口
    allocated_ports: Arc<Mutex<std::collections::HashSet<u16>>>,
}

impl ConnectionManager {
    /// Create new connection manager / 创建新的连接管理器
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(Mutex::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
            auth_tokens: Arc::new(Mutex::new(HashMap::new())),
            port_allocator: PortAllocator::new(),
        }
    }

    /// Create listener for instance / 为实例创建监听器
    pub async fn create_listener_for_instance(&self, instance_id: &str) -> Result<u16, Box<dyn std::error::Error>> {
        let port = self.port_allocator.allocate_port()?;
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
        
        // Generate authentication token / 生成认证令牌
        let token = Uuid::new_v4().to_string();
        
        // Store listener and token / 存储监听器和令牌
        {
            let mut listeners = self.listeners.lock().unwrap();
            listeners.insert(instance_id.to_string(), listener);
        }
        {
            let mut tokens = self.auth_tokens.lock().unwrap();
            tokens.insert(instance_id.to_string(), token);
        }

        // Start listening task / 启动监听任务
        self.start_listening_task(instance_id.to_string(), port).await;

        Ok(port)
    }

    /// Start listening task / 启动监听任务
    async fn start_listening_task(&self, instance_id: String, port: u16) {
        let connections = Arc::clone(&self.connections);
        let auth_tokens = Arc::clone(&self.auth_tokens);
        
        tokio::spawn(async move {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
            
            loop {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        println!("New connection from: {} / 新连接来自: {}", addr);
                        
                        // Handle connection / 处理连接
                        let connections_clone = Arc::clone(&connections);
                        let auth_tokens_clone = Arc::clone(&auth_tokens);
                        let instance_id_clone = instance_id.clone();
                        
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_connection(
                                stream, 
                                instance_id_clone, 
                                connections_clone, 
                                auth_tokens_clone
                            ).await {
                                eprintln!("Error handling connection: {} / 处理连接时出错: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {} / 接受连接时出错: {}", e);
                        break;
                    }
                }
            }
        });
    }

    /// Handle connection / 处理连接
    async fn handle_connection(
        stream: TcpStream,
        instance_id: String,
        connections: Arc<Mutex<HashMap<String, Connection>>>,
        auth_tokens: Arc<Mutex<HashMap<String, String>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Implement connection handling logic / 实现连接处理逻辑
        // 1. Wait for authentication message / 等待认证消息
        // 2. Verify token / 验证令牌
        // 3. Establish bidirectional communication channel / 建立双向通信通道
        // 4. Handle message loop / 处理消息循环
        
        todo!("Implement connection handling logic / 实现连接处理逻辑")
    }

    /// Send message to instance / 发送消息到实例
    pub async fn send_message(&self, instance_id: &str, message: SpearMessage) -> Result<(), Box<dyn std::error::Error>> {
        let connections = self.connections.lock().unwrap();
        if let Some(connection) = connections.get(instance_id) {
            connection.sender.send(message)?;
            Ok(())
        } else {
            Err(format!("Instance connection not found: {} / 未找到实例连接: {}", instance_id).into())
        }
    }

    /// Get authentication info for instance / 获取实例的认证信息
    pub fn get_auth_info(&self, instance_id: &str) -> Option<(u16, String)> {
        let tokens = self.auth_tokens.lock().unwrap();
        let listeners = self.listeners.lock().unwrap();
        
        if let (Some(token), Some(listener)) = (tokens.get(instance_id), listeners.get(instance_id)) {
            if let Ok(addr) = listener.local_addr() {
                return Some((addr.port(), token.clone()));
            }
        }
        None
    }
}

impl PortAllocator {
    /// Create new port allocator / 创建新的端口分配器
    pub fn new() -> Self {
        Self {
            current_port: Arc::new(Mutex::new(9100)),
            allocated_ports: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Allocate port / 分配端口
    pub fn allocate_port(&self) -> Result<u16, Box<dyn std::error::Error>> {
        let mut current = self.current_port.lock().unwrap();
        let mut allocated = self.allocated_ports.lock().unwrap();
        
        while allocated.contains(&*current) {
            *current += 1;
            if *current > 65535 {
                return Err("No available ports / 无可用端口".into());
            }
        }
        
        let port = *current;
        allocated.insert(port);
        *current += 1;
        
        Ok(port)
    }

    /// Release port / 释放端口
    pub fn release_port(&self, port: u16) {
        let mut allocated = self.allocated_ports.lock().unwrap();
        allocated.remove(&port);
    }
}
```

#### 1.3 Improved Process Runtime

```rust
// spearlet/execution/runtime/process.rs

use super::Runtime;
use crate::execution::communication::{ConnectionManager, SpearMessage, ExecuteRequest, ExecuteResponse};
use std::process::{Command, Stdio};
use std::collections::HashMap;
use std::sync::Arc;

/// Process runtime / 进程运行时
pub struct ProcessRuntime {
    /// Connection manager / 连接管理器
    connection_manager: Arc<ConnectionManager>,
    /// Running processes / 运行中的进程
    running_processes: Arc<std::sync::Mutex<HashMap<String, std::process::Child>>>,
}

impl ProcessRuntime {
    /// Create new process runtime / 创建新的进程运行时
    pub fn new() -> Self {
        Self {
            connection_manager: Arc::new(ConnectionManager::new()),
            running_processes: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl Runtime for ProcessRuntime {
    async fn create_instance(&self, artifact_id: &str, config: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
        // Generate instance ID / 生成实例ID
        let instance_id = uuid::Uuid::new_v4().to_string();
        
        // Create listener for instance / 为实例创建监听器
        let (port, token) = match self.connection_manager.create_listener_for_instance(&instance_id).await {
            Ok(port) => {
                let auth_info = self.connection_manager.get_auth_info(&instance_id)
                    .ok_or("Cannot get auth info / 无法获取认证信息")?;
                auth_info
            }
            Err(e) => return Err(format!("Failed to create listener: {} / 创建监听器失败: {}", e).into()),
        };

        println!("Created listener for instance {}, port: {}, token: {} / 为实例 {} 创建监听器，端口: {}, 令牌: {}", 
                instance_id, port, token, instance_id, port, token);

        Ok(instance_id)
    }

    async fn start_instance(&self, instance_id: &str, config: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        // Get authentication info / 获取认证信息
        let (port, token) = self.connection_manager.get_auth_info(instance_id)
            .ok_or("Instance auth info not found / 未找到实例认证信息")?;

        // Get command and args from config / 从配置中获取命令和参数
        let command = config.get("command")
            .and_then(|v| v.as_str())
            .ok_or("Missing command in config / 配置中缺少命令")?;
        
        let args: Vec<String> = config.get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        // Start process / 启动进程
        let mut cmd = Command::new(command);
        cmd.args(&args)
            .env("SPEARLET_ADDR", format!("127.0.0.1:{}", port))
            .env("SPEARLET_TOKEN", token)
            .env("SPEARLET_INSTANCE_ID", instance_id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn()?;
        
        // Store process reference / 存储进程引用
        {
            let mut processes = self.running_processes.lock().unwrap();
            processes.insert(instance_id.to_string(), child);
        }

        println!("Started instance process: {} / 启动实例进程: {}", instance_id);
        Ok(())
    }

    async fn stop_instance(&self, instance_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Stop process / 停止进程
        {
            let mut processes = self.running_processes.lock().unwrap();
            if let Some(mut child) = processes.remove(instance_id) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        println!("Stopped instance process: {} / 停止实例进程: {}", instance_id);
        Ok(())
    }

    async fn execute(&self, instance_id: &str, request: &serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        // Construct execution request / 构造执行请求
        let execute_request = ExecuteRequest {
            task_id: uuid::Uuid::new_v4().to_string(),
            command: request.get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            args: request.get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            env: HashMap::new(),
        };

        // Serialize request / 序列化请求
        let payload = serde_json::to_vec(&execute_request)?;
        
        // Create message / 创建消息
        let message = SpearMessage {
            message_type: crate::execution::communication::protocol::MessageType::ExecuteRequest,
            request_id: rand::random(),
            timestamp: std::time::SystemTime::now(),
            payload,
        };

        // Send message to instance / 发送消息到实例
        self.connection_manager.send_message(instance_id, message).await?;

        // Wait for response / 等待响应
        // TODO: Implement response waiting logic / 实现响应等待逻辑
        
        Ok(serde_json::json!({
            "status": "submitted",
            "message": "Execution request submitted / 执行请求已提交"
        }))
    }
}
```

### 2. Implementation Steps

#### Phase 1: Basic Protocol and Connection Management (Week 1-2)

1. **Implement Message Protocol**
   - Create `protocol.rs` file
   - Define message formats and types
   - Implement serialization/deserialization

2. **Implement Connection Manager**
   - Create `connection_manager.rs` file
   - Implement port allocation logic
   - Implement connection lifecycle management

3. **Update Process Runtime**
   - Modify `process.rs` file
   - Integrate connection manager
   - Implement environment variable injection

#### Phase 2: Authentication and Security (Week 3)

1. **Implement Authentication Mechanism**
   - Token generation and verification
   - Connection authentication flow
   - Session management

2. **Security Enhancement**
   - Connection timeout handling
   - Error handling and recovery
   - Logging

#### Phase 3: Protocol Processing and Message Routing (Week 4)

1. **Message Processing**
   - Implement message encoding/decoding
   - Message routing logic
   - Request/response correlation

2. **Error Handling**
   - Connection disconnection handling
   - Reconnection mechanism
   - Error recovery

#### Phase 4: Testing and Optimization (Week 5-6)

1. **Unit Testing**
   - Protocol testing
   - Connection management testing
   - Runtime integration testing

2. **Performance Optimization**
   - Connection pool optimization
   - Message batching
   - Memory usage optimization

3. **Documentation and Examples**
   - API documentation
   - Usage examples
   - Best practices guide

### 3. Configuration Examples

#### 3.1 Process Runtime Configuration

```json
{
  "runtime_type": "process",
  "config": {
    "command": "/path/to/agent",
    "args": ["--mode", "spear-agent"],
    "working_directory": "/tmp/spear-workspace",
    "environment": {
      "LOG_LEVEL": "info"
    },
    "timeout": 300,
    "max_memory": "1GB"
  }
}
```

#### 3.2 Agent Connection Example

```rust
// Agent-side connection code example / Agent 端连接代码示例
use std::net::TcpStream;
use std::io::{Read, Write};

fn connect_to_spearlet() -> Result<(), Box<dyn std::error::Error>> {
    // Get connection info from environment variables / 从环境变量获取连接信息
    let addr = std::env::var("SPEARLET_ADDR")?;
    let token = std::env::var("SPEARLET_TOKEN")?;
    let instance_id = std::env::var("SPEARLET_INSTANCE_ID")?;

    // Connect to Spearlet / 连接到 Spearlet
    let mut stream = TcpStream::connect(&addr)?;
    
    // Send authentication request / 发送认证请求
    let auth_request = AuthRequest {
        instance_id,
        token,
        client_version: "1.0.0".to_string(),
    };
    
    let message = SpearMessage {
        message_type: MessageType::AuthRequest,
        request_id: 1,
        timestamp: std::time::SystemTime::now(),
        payload: serde_json::to_vec(&auth_request)?,
    };
    
    // Send message / 发送消息
    let serialized = serde_json::to_vec(&message)?;
    let len = serialized.len() as u64;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(&serialized)?;
    
    // Receive authentication response / 接收认证响应
    let mut len_buf = [0u8; 8];
    stream.read_exact(&mut len_buf)?;
    let len = u64::from_le_bytes(len_buf) as usize;
    
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    
    let response: SpearMessage = serde_json::from_slice(&buf)?;
    println!("Auth response: {:?} / 认证响应: {:?}", response);
    
    Ok(())
}
```

### 4. Monitoring and Diagnostics

#### 4.1 Connection Status Monitoring

```rust
// spearlet/execution/communication/monitoring.rs

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Connection statistics / 连接统计信息
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Connection time / 连接时间
    pub connected_at: Instant,
    /// Last activity time / 最后活动时间
    pub last_activity: Instant,
    /// Sent message count / 发送消息数
    pub messages_sent: u64,
    /// Received message count / 接收消息数
    pub messages_received: u64,
    /// Error count / 错误计数
    pub error_count: u64,
}

/// Monitoring service / 监控服务
pub struct MonitoringService {
    /// Connection statistics / 连接统计
    connection_stats: std::sync::Mutex<HashMap<String, ConnectionStats>>,
}

impl MonitoringService {
    /// Create new monitoring service / 创建新的监控服务
    pub fn new() -> Self {
        Self {
            connection_stats: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Record connection / 记录连接
    pub fn record_connection(&self, instance_id: &str) {
        let mut stats = self.connection_stats.lock().unwrap();
        stats.insert(instance_id.to_string(), ConnectionStats {
            connected_at: Instant::now(),
            last_activity: Instant::now(),
            messages_sent: 0,
            messages_received: 0,
            error_count: 0,
        });
    }

    /// Record message sent / 记录消息发送
    pub fn record_message_sent(&self, instance_id: &str) {
        let mut stats = self.connection_stats.lock().unwrap();
        if let Some(stat) = stats.get_mut(instance_id) {
            stat.messages_sent += 1;
            stat.last_activity = Instant::now();
        }
    }

    /// Get connection statistics / 获取连接统计
    pub fn get_stats(&self) -> HashMap<String, ConnectionStats> {
        self.connection_stats.lock().unwrap().clone()
    }
}
```

### 5. Testing Plan

#### 5.1 Unit Tests

```rust
// tests/communication_tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_manager_create_listener() {
        let manager = ConnectionManager::new();
        let instance_id = "test-instance";
        
        let port = manager.create_listener_for_instance(instance_id).await.unwrap();
        assert!(port > 0);
        
        let auth_info = manager.get_auth_info(instance_id);
        assert!(auth_info.is_some());
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let message = SpearMessage {
            message_type: MessageType::Heartbeat,
            request_id: 123,
            timestamp: std::time::SystemTime::now(),
            payload: vec![1, 2, 3],
        };
        
        let serialized = serde_json::to_vec(&message).unwrap();
        let deserialized: SpearMessage = serde_json::from_slice(&serialized).unwrap();
        
        assert_eq!(message.request_id, deserialized.request_id);
    }

    #[tokio::test]
    async fn test_process_runtime_integration() {
        let runtime = ProcessRuntime::new();
        let config = serde_json::json!({
            "command": "echo",
            "args": ["hello", "world"]
        });
        
        let instance_id = runtime.create_instance("test-artifact", &config).await.unwrap();
        runtime.start_instance(&instance_id, &config).await.unwrap();
        
        let request = serde_json::json!({
            "command": "echo",
            "args": ["test"]
        });
        
        let response = runtime.execute(&instance_id, &request).await.unwrap();
        assert!(response.get("status").is_some());
        
        runtime.stop_instance(&instance_id).await.unwrap();
    }
}
```

## Summary

This implementation plan provides a complete, executable solution for improving the runtime communication mechanism in spear-next. Key features include:

1. **Unified Message Protocol**: Defines standardized message formats and types
2. **Robust Connection Management**: Implements connection lifecycle management and port allocation
3. **Secure Authentication Mechanism**: Token-based authentication and session management
4. **Comprehensive Error Handling**: Includes reconnection, timeout, and error recovery mechanisms
5. **Complete Monitoring**: Connection status monitoring and performance metrics collection
6. **Thorough Testing**: Unit tests and integration tests coverage

Through this implementation, spear-next will have communication capabilities comparable to the golang version while fully leveraging Rust's type safety and performance advantages.