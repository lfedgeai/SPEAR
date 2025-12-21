# Spear-Next 运行时通信机制实现计划

## 概述

本文档提供了改进 spear-next 运行时通信机制的详细实现计划，包括具体的代码示例、实现步骤和时间安排。

## 实现方案：改进的监听模式

### 1. 核心架构设计

#### 1.1 消息协议定义

```rust
// spearlet/execution/communication/protocol.rs

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// 统一的消息格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpearMessage {
    /// 消息类型 / Message type
    pub message_type: MessageType,
    /// 请求ID，用于关联请求和响应 / Request ID for correlating requests and responses
    pub request_id: u64,
    /// 时间戳 / Timestamp
    pub timestamp: SystemTime,
    /// 消息负载 / Message payload
    pub payload: Vec<u8>,
}

/// 消息类型枚举 / Message type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// 认证请求 / Authentication request
    AuthRequest,
    /// 认证响应 / Authentication response
    AuthResponse,
    /// 执行请求 / Execution request
    ExecuteRequest,
    /// 执行响应 / Execution response
    ExecuteResponse,
    /// 信号消息 / Signal message
    Signal,
    /// 心跳消息 / Heartbeat message
    Heartbeat,
    /// 错误消息 / Error message
    Error,
}

/// 认证请求负载 / Authentication request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// 实例ID / Instance ID
    pub instance_id: String,
    /// 认证令牌 / Authentication token
    pub token: String,
    /// 客户端版本 / Client version
    pub client_version: String,
}

/// 认证响应负载 / Authentication response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// 认证是否成功 / Whether authentication succeeded
    pub success: bool,
    /// 错误消息（如果认证失败）/ Error message (if authentication failed)
    pub error_message: Option<String>,
    /// 会话ID / Session ID
    pub session_id: Option<String>,
}

/// 执行请求负载 / Execution request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// 任务ID / Task ID
    pub task_id: String,
    /// 执行命令或数据 / Execution command or data
    pub command: String,
    /// 参数 / Arguments
    pub args: Vec<String>,
    /// 环境变量 / Environment variables
    pub env: std::collections::HashMap<String, String>,
}

/// 执行响应负载 / Execution response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponse {
    /// 任务ID / Task ID
    pub task_id: String,
    /// 执行状态 / Execution status
    pub status: ExecutionStatus,
    /// 输出数据 / Output data
    pub output: Option<String>,
    /// 错误信息 / Error information
    pub error: Option<String>,
    /// 退出码 / Exit code
    pub exit_code: Option<i32>,
}

/// 执行状态枚举 / Execution status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// 开始执行 / Started execution
    Started,
    /// 执行中 / Running
    Running,
    /// 执行完成 / Completed
    Completed,
    /// 执行失败 / Failed
    Failed,
}
```

#### 1.2 连接管理器

```rust
// spearlet/execution/communication/connection_manager.rs

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;

/// 连接管理器 / Connection manager
pub struct ConnectionManager {
    /// 监听器映射 / Listener mapping
    listeners: Arc<Mutex<HashMap<String, TcpListener>>>,
    /// 连接映射 / Connection mapping
    connections: Arc<Mutex<HashMap<String, Connection>>>,
    /// 认证令牌映射 / Authentication token mapping
    auth_tokens: Arc<Mutex<HashMap<String, String>>>,
    /// 端口分配器 / Port allocator
    port_allocator: PortAllocator,
}

/// 连接信息 / Connection information
#[derive(Debug)]
pub struct Connection {
    /// 实例ID / Instance ID
    pub instance_id: String,
    /// TCP流 / TCP stream
    pub stream: TcpStream,
    /// 会话ID / Session ID
    pub session_id: String,
    /// 是否已认证 / Whether authenticated
    pub authenticated: bool,
    /// 发送通道 / Send channel
    pub sender: mpsc::UnboundedSender<SpearMessage>,
    /// 接收通道 / Receive channel
    pub receiver: mpsc::UnboundedReceiver<SpearMessage>,
}

/// 端口分配器 / Port allocator
pub struct PortAllocator {
    /// 当前端口 / Current port
    current_port: Arc<Mutex<u16>>,
    /// 已分配端口 / Allocated ports
    allocated_ports: Arc<Mutex<std::collections::HashSet<u16>>>,
}

impl ConnectionManager {
    /// 创建新的连接管理器 / Create new connection manager
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(Mutex::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
            auth_tokens: Arc::new(Mutex::new(HashMap::new())),
            port_allocator: PortAllocator::new(),
        }
    }

    /// 为实例创建监听器 / Create listener for instance
    pub async fn create_listener_for_instance(&self, instance_id: &str) -> Result<u16, Box<dyn std::error::Error>> {
        let port = self.port_allocator.allocate_port()?;
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
        
        // 生成认证令牌 / Generate authentication token
        let token = Uuid::new_v4().to_string();
        
        // 存储监听器和令牌 / Store listener and token
        {
            let mut listeners = self.listeners.lock().unwrap();
            listeners.insert(instance_id.to_string(), listener);
        }
        {
            let mut tokens = self.auth_tokens.lock().unwrap();
            tokens.insert(instance_id.to_string(), token);
        }

        // 启动监听任务 / Start listening task
        self.start_listening_task(instance_id.to_string(), port).await;

        Ok(port)
    }

    /// 启动监听任务 / Start listening task
    async fn start_listening_task(&self, instance_id: String, port: u16) {
        let connections = Arc::clone(&self.connections);
        let auth_tokens = Arc::clone(&self.auth_tokens);
        
        tokio::spawn(async move {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
            
            loop {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        println!("新连接来自: {} / New connection from: {}", addr);
                        
                        // 处理连接 / Handle connection
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
                                eprintln!("处理连接时出错: {} / Error handling connection: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("接受连接时出错: {} / Error accepting connection: {}", e);
                        break;
                    }
                }
            }
        });
    }

    /// 处理连接 / Handle connection
    async fn handle_connection(
        stream: TcpStream,
        instance_id: String,
        connections: Arc<Mutex<HashMap<String, Connection>>>,
        auth_tokens: Arc<Mutex<HashMap<String, String>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 实现连接处理逻辑 / Implement connection handling logic
        // 1. 等待认证消息 / Wait for authentication message
        // 2. 验证令牌 / Verify token
        // 3. 建立双向通信通道 / Establish bidirectional communication channel
        // 4. 处理消息循环 / Handle message loop
        
        todo!("实现连接处理逻辑 / Implement connection handling logic")
    }

    /// 发送消息到实例 / Send message to instance
    pub async fn send_message(&self, instance_id: &str, message: SpearMessage) -> Result<(), Box<dyn std::error::Error>> {
        let connections = self.connections.lock().unwrap();
        if let Some(connection) = connections.get(instance_id) {
            connection.sender.send(message)?;
            Ok(())
        } else {
            Err(format!("未找到实例连接: {} / Instance connection not found: {}", instance_id).into())
        }
    }

    /// 获取实例的认证信息 / Get authentication info for instance
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
    /// 创建新的端口分配器 / Create new port allocator
    pub fn new() -> Self {
        Self {
            current_port: Arc::new(Mutex::new(9100)),
            allocated_ports: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// 分配端口 / Allocate port
    pub fn allocate_port(&self) -> Result<u16, Box<dyn std::error::Error>> {
        let mut current = self.current_port.lock().unwrap();
        let mut allocated = self.allocated_ports.lock().unwrap();
        
        while allocated.contains(&*current) {
            *current += 1;
            if *current > 65535 {
                return Err("无可用端口 / No available ports".into());
            }
        }
        
        let port = *current;
        allocated.insert(port);
        *current += 1;
        
        Ok(port)
    }

    /// 释放端口 / Release port
    pub fn release_port(&self, port: u16) {
        let mut allocated = self.allocated_ports.lock().unwrap();
        allocated.remove(&port);
    }
}
```

#### 1.3 改进的 Process Runtime

```rust
// spearlet/execution/runtime/process.rs

use super::Runtime;
use crate::execution::communication::{ConnectionManager, SpearMessage, ExecuteRequest, ExecuteResponse};
use std::process::{Command, Stdio};
use std::collections::HashMap;
use std::sync::Arc;

/// 进程运行时 / Process runtime
pub struct ProcessRuntime {
    /// 连接管理器 / Connection manager
    connection_manager: Arc<ConnectionManager>,
    /// 运行中的进程 / Running processes
    running_processes: Arc<std::sync::Mutex<HashMap<String, std::process::Child>>>,
}

impl ProcessRuntime {
    /// 创建新的进程运行时 / Create new process runtime
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
        // 生成实例ID / Generate instance ID
        let instance_id = uuid::Uuid::new_v4().to_string();
        
        // 为实例创建监听器 / Create listener for instance
        let (port, token) = match self.connection_manager.create_listener_for_instance(&instance_id).await {
            Ok(port) => {
                let auth_info = self.connection_manager.get_auth_info(&instance_id)
                    .ok_or("无法获取认证信息 / Cannot get auth info")?;
                auth_info
            }
            Err(e) => return Err(format!("创建监听器失败: {} / Failed to create listener: {}", e).into()),
        };

        println!("为实例 {} 创建监听器，端口: {}, 令牌: {} / Created listener for instance {}, port: {}, token: {}", 
                instance_id, port, token, instance_id, port, token);

        Ok(instance_id)
    }

    async fn start_instance(&self, instance_id: &str, config: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        // 获取认证信息 / Get authentication info
        let (port, token) = self.connection_manager.get_auth_info(instance_id)
            .ok_or("未找到实例认证信息 / Instance auth info not found")?;

        // 从配置中获取命令和参数 / Get command and args from config
        let command = config.get("command")
            .and_then(|v| v.as_str())
            .ok_or("配置中缺少命令 / Missing command in config")?;
        
        let args: Vec<String> = config.get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        // 启动进程 / Start process
        let mut cmd = Command::new(command);
        cmd.args(&args)
            .env("SPEARLET_ADDR", format!("127.0.0.1:{}", port))
            .env("SPEARLET_TOKEN", token)
            .env("SPEARLET_INSTANCE_ID", instance_id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn()?;
        
        // 存储进程引用 / Store process reference
        {
            let mut processes = self.running_processes.lock().unwrap();
            processes.insert(instance_id.to_string(), child);
        }

        println!("启动实例进程: {} / Started instance process: {}", instance_id);
        Ok(())
    }

    async fn stop_instance(&self, instance_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 停止进程 / Stop process
        {
            let mut processes = self.running_processes.lock().unwrap();
            if let Some(mut child) = processes.remove(instance_id) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        println!("停止实例进程: {} / Stopped instance process: {}", instance_id);
        Ok(())
    }

    async fn execute(&self, instance_id: &str, request: &serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        // 构造执行请求 / Construct execution request
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

        // 序列化请求 / Serialize request
        let payload = serde_json::to_vec(&execute_request)?;
        
        // 创建消息 / Create message
        let message = SpearMessage {
            message_type: crate::execution::communication::protocol::MessageType::ExecuteRequest,
            request_id: rand::random(),
            timestamp: std::time::SystemTime::now(),
            payload,
        };

        // 发送消息到实例 / Send message to instance
        self.connection_manager.send_message(instance_id, message).await?;

        // 等待响应 / Wait for response
        // TODO: 实现响应等待逻辑 / Implement response waiting logic
        
        Ok(serde_json::json!({
            "status": "submitted",
            "message": "执行请求已提交 / Execution request submitted"
        }))
    }
}
```

### 2. 实现步骤

#### 阶段一：基础协议和连接管理（第1-2周）

1. **实现消息协议**
   - 创建 `protocol.rs` 文件
   - 定义消息格式和类型
   - 实现序列化/反序列化

2. **实现连接管理器**
   - 创建 `connection_manager.rs` 文件
   - 实现端口分配逻辑
   - 实现连接生命周期管理

3. **更新 Process Runtime**
   - 修改 `process.rs` 文件
   - 集成连接管理器
   - 实现环境变量注入

#### 阶段二：认证和安全（第3周）

1. **实现认证机制**
   - 令牌生成和验证
   - 连接认证流程
   - 会话管理

2. **安全增强**
   - 连接超时处理
   - 错误处理和恢复
   - 日志记录

#### 阶段三：协议处理和消息路由（第4周）

1. **消息处理**
   - 实现消息编码/解码
   - 消息路由逻辑
   - 请求/响应关联

2. **错误处理**
   - 连接断开处理
   - 重连机制
   - 错误恢复

#### 阶段四：测试和优化（第5-6周）

1. **单元测试**
   - 协议测试
   - 连接管理测试
   - Runtime 集成测试

2. **性能优化**
   - 连接池优化
   - 消息批处理
   - 内存使用优化

3. **文档和示例**
   - API 文档
   - 使用示例
   - 最佳实践指南

### 3. 配置示例

#### 3.1 Process Runtime 配置

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

#### 3.2 Agent 连接示例

```rust
// Agent 端连接代码示例 / Agent-side connection code example
use std::net::TcpStream;
use std::io::{Read, Write};

fn connect_to_spearlet() -> Result<(), Box<dyn std::error::Error>> {
    // 从环境变量获取连接信息 / Get connection info from environment variables
    let addr = std::env::var("SPEARLET_ADDR")?;
    let token = std::env::var("SPEARLET_TOKEN")?;
    let instance_id = std::env::var("SPEARLET_INSTANCE_ID")?;

    // 连接到 Spearlet / Connect to Spearlet
    let mut stream = TcpStream::connect(&addr)?;
    
    // 发送认证请求 / Send authentication request
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
    
    // 发送消息 / Send message
    let serialized = serde_json::to_vec(&message)?;
    let len = serialized.len() as u64;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(&serialized)?;
    
    // 接收认证响应 / Receive authentication response
    let mut len_buf = [0u8; 8];
    stream.read_exact(&mut len_buf)?;
    let len = u64::from_le_bytes(len_buf) as usize;
    
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    
    let response: SpearMessage = serde_json::from_slice(&buf)?;
    println!("认证响应: {:?} / Auth response: {:?}", response);
    
    Ok(())
}
```

### 4. 监控和诊断

#### 4.1 连接状态监控

```rust
// spearlet/execution/communication/monitoring.rs

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 连接统计信息 / Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// 连接时间 / Connection time
    pub connected_at: Instant,
    /// 最后活动时间 / Last activity time
    pub last_activity: Instant,
    /// 发送消息数 / Sent message count
    pub messages_sent: u64,
    /// 接收消息数 / Received message count
    pub messages_received: u64,
    /// 错误计数 / Error count
    pub error_count: u64,
}

/// 监控服务 / Monitoring service
pub struct MonitoringService {
    /// 连接统计 / Connection statistics
    connection_stats: std::sync::Mutex<HashMap<String, ConnectionStats>>,
}

impl MonitoringService {
    /// 创建新的监控服务 / Create new monitoring service
    pub fn new() -> Self {
        Self {
            connection_stats: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// 记录连接 / Record connection
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

    /// 记录消息发送 / Record message sent
    pub fn record_message_sent(&self, instance_id: &str) {
        let mut stats = self.connection_stats.lock().unwrap();
        if let Some(stat) = stats.get_mut(instance_id) {
            stat.messages_sent += 1;
            stat.last_activity = Instant::now();
        }
    }

    /// 获取连接统计 / Get connection statistics
    pub fn get_stats(&self) -> HashMap<String, ConnectionStats> {
        self.connection_stats.lock().unwrap().clone()
    }
}
```

### 5. 测试计划

#### 5.1 单元测试

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

## 总结

这个实现计划提供了一个完整的、可执行的方案来改进 spear-next 的运行时通信机制。主要特点包括：

1. **统一的消息协议**：定义了标准化的消息格式和类型
2. **健壮的连接管理**：实现了连接生命周期管理和端口分配
3. **安全的认证机制**：基于令牌的认证和会话管理
4. **完善的错误处理**：包含重连、超时和错误恢复机制
5. **全面的监控**：连接状态监控和性能指标收集
6. **详细的测试**：单元测试和集成测试覆盖

通过这个实现，spear-next 将具备与 golang 版本相当的通信能力，同时充分利用 Rust 的类型安全和性能优势。