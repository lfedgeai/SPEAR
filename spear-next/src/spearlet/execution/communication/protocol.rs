// 统一的消息协议定义 / Unified message protocol definition
// 定义了 spearlet 与 agent 之间的通信协议 / Defines communication protocol between spearlet and agent

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// 统一的消息格式 / Unified message format
/// 所有 spearlet 与 agent 之间的通信都使用此格式 / All communication between spearlet and agent uses this format
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
    /// 消息版本，用于协议兼容性 / Message version for protocol compatibility
    pub version: u8,
}

/// 消息类型枚举 / Message type enumeration
/// 定义了所有支持的消息类型 / Defines all supported message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    /// 流数据 / Stream data
    StreamData,
    /// 连接关闭 / Connection close
    ConnectionClose,
}

/// 认证请求负载 / Authentication request payload
/// Agent 连接到 Spearlet 时发送的认证信息 / Authentication info sent when agent connects to spearlet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// 实例ID / Instance ID
    pub instance_id: String,
    /// 认证令牌 / Authentication token
    pub token: String,
    /// 客户端版本 / Client version
    pub client_version: String,
    /// 客户端类型（process, k8s等）/ Client type (process, k8s, etc.)
    pub client_type: String,
    /// 额外的认证参数 / Additional authentication parameters
    pub extra_params: HashMap<String, String>,
}

/// 认证响应负载 / Authentication response payload
/// Spearlet 对认证请求的响应 / Spearlet's response to authentication request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// 认证是否成功 / Whether authentication succeeded
    pub success: bool,
    /// 错误消息（如果认证失败）/ Error message (if authentication failed)
    pub error_message: Option<String>,
    /// 会话ID / Session ID
    pub session_id: Option<String>,
    /// 服务器版本 / Server version
    pub server_version: String,
    /// 支持的功能列表 / List of supported features
    pub supported_features: Vec<String>,
}

/// 执行请求负载 / Execution request payload
/// Spearlet 向 Agent 发送的执行请求 / Execution request sent from spearlet to agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// 任务ID / Task ID
    pub task_id: String,
    /// 执行命令或数据 / Execution command or data
    pub command: String,
    /// 参数 / Arguments
    pub args: Vec<String>,
    /// 环境变量 / Environment variables
    pub env: HashMap<String, String>,
    /// 工作目录 / Working directory
    pub working_dir: Option<String>,
    /// 超时时间（秒）/ Timeout in seconds
    pub timeout: Option<u64>,
    /// 执行模式 / Execution mode
    pub mode: ExecutionMode,
    /// 输入数据 / Input data
    pub input_data: Option<Vec<u8>>,
}

/// 执行响应负载 / Execution response payload
/// Agent 对执行请求的响应 / Agent's response to execution request
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
    /// 执行时长（毫秒）/ Execution duration in milliseconds
    pub duration_ms: Option<u64>,
    /// 资源使用情况 / Resource usage
    pub resource_usage: Option<ResourceUsage>,
}

/// 执行模式枚举 / Execution mode enumeration
/// 定义了不同的执行模式 / Defines different execution modes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionMode {
    /// 同步执行 / Synchronous execution
    Sync,
    /// 异步执行 / Asynchronous execution
    Async,
    /// 流式执行 / Streaming execution
    Stream,
    /// 交互式执行 / Interactive execution
    Interactive,
}

/// 执行状态枚举 / Execution status enumeration
/// 定义了任务的执行状态 / Defines task execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    /// 已接收 / Received
    Received,
    /// 开始执行 / Started execution
    Started,
    /// 执行中 / Running
    Running,
    /// 执行完成 / Completed
    Completed,
    /// 执行失败 / Failed
    Failed,
    /// 已取消 / Cancelled
    Cancelled,
    /// 超时 / Timeout
    Timeout,
}

/// 信号消息负载 / Signal message payload
/// 用于发送控制信号 / Used for sending control signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMessage {
    /// 信号类型 / Signal type
    pub signal_type: SignalType,
    /// 目标任务ID（可选）/ Target task ID (optional)
    pub task_id: Option<String>,
    /// 信号数据 / Signal data
    pub data: Option<Vec<u8>>,
}

/// 信号类型枚举 / Signal type enumeration
/// 定义了支持的信号类型 / Defines supported signal types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignalType {
    /// 终止信号 / Terminate signal
    Terminate,
    /// 暂停信号 / Pause signal
    Pause,
    /// 恢复信号 / Resume signal
    Resume,
    /// 重启信号 / Restart signal
    Restart,
    /// 自定义信号 / Custom signal
    Custom(String),
}

/// 心跳消息负载 / Heartbeat message payload
/// 用于保持连接活跃 / Used to keep connection alive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    /// 发送时间戳 / Send timestamp
    pub timestamp: SystemTime,
    /// 序列号 / Sequence number
    pub sequence: u64,
    /// 连接状态 / Connection status
    pub status: ConnectionStatus,
}

/// 连接状态枚举 / Connection status enumeration
/// 定义了连接的状态 / Defines connection status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    /// 活跃 / Active
    Active,
    /// 空闲 / Idle
    Idle,
    /// 忙碌 / Busy
    Busy,
    /// 错误 / Error
    Error,
}

/// 错误消息负载 / Error message payload
/// 用于传递错误信息 / Used for conveying error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    /// 错误代码 / Error code
    pub error_code: u32,
    /// 错误消息 / Error message
    pub message: String,
    /// 错误详情 / Error details
    pub details: Option<String>,
    /// 相关的请求ID / Related request ID
    pub related_request_id: Option<u64>,
}

/// 流数据消息负载 / Stream data message payload
/// 用于传输流式数据 / Used for transmitting streaming data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDataMessage {
    /// 流ID / Stream ID
    pub stream_id: String,
    /// 数据块 / Data chunk
    pub data: Vec<u8>,
    /// 是否为最后一块数据 / Whether this is the last chunk
    pub is_last: bool,
    /// 数据块序号 / Chunk sequence number
    pub sequence: u64,
}

/// 资源使用情况 / Resource usage information
/// 记录任务执行时的资源使用情况 / Records resource usage during task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU 使用率（百分比）/ CPU usage percentage
    pub cpu_percent: Option<f64>,
    /// 内存使用量（字节）/ Memory usage in bytes
    pub memory_bytes: Option<u64>,
    /// 磁盘读取字节数 / Disk read bytes
    pub disk_read_bytes: Option<u64>,
    /// 磁盘写入字节数 / Disk write bytes
    pub disk_write_bytes: Option<u64>,
    /// 网络接收字节数 / Network received bytes
    pub network_rx_bytes: Option<u64>,
    /// 网络发送字节数 / Network transmitted bytes
    pub network_tx_bytes: Option<u64>,
}

impl SpearMessage {
    /// 创建新的消息 / Create new message
    pub fn new(message_type: MessageType, request_id: u64, payload: Vec<u8>) -> Self {
        Self {
            message_type,
            request_id,
            timestamp: SystemTime::now(),
            payload,
            version: 1, // 当前协议版本 / Current protocol version
        }
    }

    /// 创建认证请求消息 / Create authentication request message
    pub fn auth_request(request_id: u64, auth_req: AuthRequest) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(&auth_req)?;
        Ok(Self::new(MessageType::AuthRequest, request_id, payload))
    }

    /// 创建认证响应消息 / Create authentication response message
    pub fn auth_response(
        request_id: u64,
        auth_resp: AuthResponse,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(&auth_resp)?;
        Ok(Self::new(MessageType::AuthResponse, request_id, payload))
    }

    /// 创建执行请求消息 / Create execution request message
    pub fn execute_request(
        request_id: u64,
        exec_req: ExecuteRequest,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(&exec_req)?;
        Ok(Self::new(MessageType::ExecuteRequest, request_id, payload))
    }

    /// 创建执行响应消息 / Create execution response message
    pub fn execute_response(
        request_id: u64,
        exec_resp: ExecuteResponse,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(&exec_resp)?;
        Ok(Self::new(MessageType::ExecuteResponse, request_id, payload))
    }

    /// 创建心跳消息 / Create heartbeat message
    pub fn heartbeat(
        request_id: u64,
        heartbeat: HeartbeatMessage,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(&heartbeat)?;
        Ok(Self::new(MessageType::Heartbeat, request_id, payload))
    }

    /// 创建错误消息 / Create error message
    pub fn error(request_id: u64, error_msg: ErrorMessage) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(&error_msg)?;
        Ok(Self::new(MessageType::Error, request_id, payload))
    }

    /// 解析消息负载 / Parse message payload
    pub fn parse_payload<T>(&self) -> Result<T, serde_json::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_slice(&self.payload)
    }

    /// 序列化消息 / Serialize message
    pub fn serialize(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// 反序列化消息 / Deserialize message
    pub fn deserialize(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

/// 协议常量 / Protocol constants
pub mod constants {
    /// 当前协议版本 / Current protocol version
    pub const PROTOCOL_VERSION: u8 = 1;

    /// 最大消息大小（字节）/ Maximum message size in bytes
    pub const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024; // 64MB

    /// 心跳间隔（秒）/ Heartbeat interval in seconds
    pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

    /// 连接超时（秒）/ Connection timeout in seconds
    pub const CONNECTION_TIMEOUT_SECS: u64 = 300; // 5 minutes

    /// 认证超时（秒）/ Authentication timeout in seconds
    pub const AUTH_TIMEOUT_SECS: u64 = 30;

    /// 默认端口范围开始 / Default port range start
    pub const DEFAULT_PORT_RANGE_START: u16 = 9100;

    /// 默认端口范围结束 / Default port range end
    pub const DEFAULT_PORT_RANGE_END: u16 = 9999;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        // 测试消息序列化和反序列化 / Test message serialization and deserialization
        let auth_req = AuthRequest {
            instance_id: "test-instance".to_string(),
            token: "test-token".to_string(),
            client_version: "1.0.0".to_string(),
            client_type: "process".to_string(),
            extra_params: HashMap::new(),
        };

        let message = SpearMessage::auth_request(123, auth_req.clone()).unwrap();
        let serialized = message.serialize().unwrap();
        let deserialized = SpearMessage::deserialize(&serialized).unwrap();

        assert_eq!(message.message_type, deserialized.message_type);
        assert_eq!(message.request_id, deserialized.request_id);
        assert_eq!(message.version, deserialized.version);

        let parsed_auth_req: AuthRequest = deserialized.parse_payload().unwrap();
        assert_eq!(auth_req.instance_id, parsed_auth_req.instance_id);
        assert_eq!(auth_req.token, parsed_auth_req.token);
    }

    #[test]
    fn test_execution_request() {
        // 测试执行请求 / Test execution request
        let exec_req = ExecuteRequest {
            task_id: "task-123".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string(), "world".to_string()],
            env: HashMap::new(),
            working_dir: Some("/tmp".to_string()),
            timeout: Some(60),
            mode: ExecutionMode::Sync,
            input_data: None,
        };

        let message = SpearMessage::execute_request(456, exec_req.clone()).unwrap();
        let parsed: ExecuteRequest = message.parse_payload().unwrap();

        assert_eq!(exec_req.task_id, parsed.task_id);
        assert_eq!(exec_req.command, parsed.command);
        assert_eq!(exec_req.args, parsed.args);
        assert_eq!(exec_req.mode, parsed.mode);
    }
}
