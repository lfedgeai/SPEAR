//! HTTP Adapter Module
//! HTTP 适配器模块
//!
//! This module provides adapters to convert between runtime execution responses and HTTP responses.
//! 该模块提供适配器，用于在运行时执行响应和 HTTP 响应之间进行转换。
//!
//! ## Design Philosophy / 设计理念
//! - **Separation of Concerns**: Runtime layer focuses on execution, HTTP layer focuses on transport
//! - **关注点分离**: 运行时层专注于执行，HTTP 层专注于传输
//! - **Protocol Agnostic**: Runtime responses can be adapted to different protocols (HTTP, gRPC, etc.)
//! - **协议无关**: 运行时响应可以适配到不同协议（HTTP、gRPC 等）

use crate::spearlet::execution::runtime::{
    ExecutionMode, RuntimeExecutionError, RuntimeExecutionResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// HTTP response structure / HTTP 响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    /// HTTP status code / HTTP 状态码
    pub status_code: u16,
    /// Response headers / 响应头
    pub headers: HashMap<String, String>,
    /// Response body / 响应体
    pub body: Vec<u8>,
    /// Response metadata for debugging / 用于调试的响应元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

/// HTTP error response / HTTP 错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpErrorResponse {
    /// Error code / 错误代码
    pub error_code: String,
    /// Error message / 错误消息
    pub error_message: String,
    /// Execution ID for tracking / 用于跟踪的执行 ID
    pub execution_id: String,
    /// Additional error details / 额外错误详情
    pub details: HashMap<String, serde_json::Value>,
}

/// Async execution status response / 异步执行状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncStatusResponse {
    /// Execution ID / 执行 ID
    pub execution_id: String,
    /// Task ID / 任务 ID
    pub task_id: Option<String>,
    /// Current status / 当前状态
    pub status: String,
    /// Status endpoint for polling / 用于轮询的状态端点
    pub status_endpoint: String,
    /// Estimated completion time in milliseconds / 预计完成时间（毫秒）
    pub estimated_completion_ms: Option<u64>,
    /// Whether execution is completed / 执行是否已完成
    pub is_completed: bool,
    /// Result data (only available when completed) / 结果数据（仅在完成时可用）
    pub result: Option<Vec<u8>>,
    /// Error information (if failed) / 错误信息（如果失败）
    pub error: Option<HttpErrorResponse>,
}

/// HTTP adapter for converting runtime responses / 用于转换运行时响应的 HTTP 适配器
pub struct HttpAdapter;

impl HttpAdapter {
    /// Create a new HTTP adapter / 创建新的 HTTP 适配器
    pub fn new() -> Self {
        Self
    }

    /// Convert RuntimeExecutionError to error code and message / 将RuntimeExecutionError转换为错误代码和消息
    fn error_to_code_and_message(error: &RuntimeExecutionError) -> (String, String) {
        match error {
            RuntimeExecutionError::InstanceNotFound { instance_id } => (
                "INSTANCE_NOT_FOUND".to_string(),
                format!("Instance not found: {}", instance_id),
            ),
            RuntimeExecutionError::InstanceNotReady { instance_id } => (
                "INSTANCE_NOT_READY".to_string(),
                format!("Instance not ready: {}", instance_id),
            ),
            RuntimeExecutionError::ExecutionTimeout { timeout_ms } => (
                "EXECUTION_TIMEOUT".to_string(),
                format!("Execution timeout: {}ms", timeout_ms),
            ),
            RuntimeExecutionError::ResourceLimitExceeded { resource, limit } => (
                "RESOURCE_LIMIT_EXCEEDED".to_string(),
                format!("Resource limit exceeded: {} (limit: {})", resource, limit),
            ),
            RuntimeExecutionError::ConfigurationError { message } => {
                ("CONFIGURATION_ERROR".to_string(), message.clone())
            }
            RuntimeExecutionError::RuntimeError { message } => {
                ("RUNTIME_ERROR".to_string(), message.clone())
            }
            RuntimeExecutionError::IoError { message } => ("IO_ERROR".to_string(), message.clone()),
            RuntimeExecutionError::SerializationError { message } => {
                ("SERIALIZATION_ERROR".to_string(), message.clone())
            }
            RuntimeExecutionError::UnsupportedOperation {
                operation,
                runtime_type,
            } => (
                "UNSUPPORTED_OPERATION".to_string(),
                format!(
                    "Operation '{}' not supported by runtime '{}'",
                    operation, runtime_type
                ),
            ),
        }
    }

    /// Convert RuntimeExecutionResponse to HTTP response / 将 RuntimeExecutionResponse 转换为 HTTP 响应
    pub fn to_http_response(runtime_response: &RuntimeExecutionResponse) -> HttpResponse {
        match runtime_response.execution_mode {
            ExecutionMode::Sync => Self::sync_to_http(runtime_response),
            ExecutionMode::Async => Self::async_to_http(runtime_response),
            ExecutionMode::Stream => Self::stream_to_http(runtime_response),
            ExecutionMode::Unknown => Self::unknown_to_http(runtime_response),
        }
    }

    /// Convert sync execution to HTTP response / 将同步执行转换为 HTTP 响应
    fn sync_to_http(runtime_response: &RuntimeExecutionResponse) -> HttpResponse {
        let status_code = if runtime_response.has_failed() {
            500
        } else {
            200
        };

        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        );
        headers.insert(
            "X-Execution-ID".to_string(),
            runtime_response.execution_id.clone(),
        );
        headers.insert("X-Execution-Mode".to_string(), "sync".to_string());
        headers.insert(
            "X-Duration-Ms".to_string(),
            runtime_response.duration_ms.to_string(),
        );

        let body = if let Some(error) = &runtime_response.error {
            let (error_code, error_message) = Self::error_to_code_and_message(error);

            serde_json::to_vec(&HttpErrorResponse {
                error_code,
                error_message,
                execution_id: runtime_response.execution_id.clone(),
                details: HashMap::new(),
            })
            .unwrap_or_default()
        } else {
            runtime_response.data.clone()
        };

        HttpResponse {
            status_code,
            headers,
            body,
            metadata: runtime_response.metadata.clone(),
        }
    }

    /// Convert async execution to HTTP response / 将异步执行转换为 HTTP 响应
    fn async_to_http(runtime_response: &RuntimeExecutionResponse) -> HttpResponse {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert(
            "X-Execution-ID".to_string(),
            runtime_response.execution_id.clone(),
        );
        headers.insert("X-Execution-Mode".to_string(), "async".to_string());

        let status_response = AsyncStatusResponse {
            execution_id: runtime_response.execution_id.clone(),
            task_id: runtime_response.task_id.clone(),
            status: format!("{:?}", runtime_response.execution_status).to_lowercase(),
            status_endpoint: runtime_response.status_endpoint.clone().unwrap_or_default(),
            estimated_completion_ms: runtime_response.estimated_completion_ms,
            is_completed: runtime_response.is_completed(),
            result: if runtime_response.is_completed() && runtime_response.error.is_none() {
                Some(runtime_response.data.clone())
            } else {
                None
            },
            error: runtime_response.error.as_ref().map(|e| {
                let (error_code, error_message) = Self::error_to_code_and_message(e);
                HttpErrorResponse {
                    error_code,
                    error_message,
                    execution_id: runtime_response.execution_id.clone(),
                    details: HashMap::new(),
                }
            }),
        };

        let status_code = if runtime_response.has_failed() {
            500
        } else if runtime_response.is_completed() {
            200
        } else {
            202 // Accepted
        };

        HttpResponse {
            status_code,
            headers,
            body: serde_json::to_vec(&status_response).unwrap_or_default(),
            metadata: runtime_response.metadata.clone(),
        }
    }

    /// Convert stream execution to HTTP response / 将流式执行转换为 HTTP 响应
    fn stream_to_http(runtime_response: &RuntimeExecutionResponse) -> HttpResponse {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/event-stream".to_string());
        headers.insert("Cache-Control".to_string(), "no-cache".to_string());
        headers.insert("Connection".to_string(), "keep-alive".to_string());
        headers.insert(
            "X-Execution-ID".to_string(),
            runtime_response.execution_id.clone(),
        );
        headers.insert("X-Execution-Mode".to_string(), "stream".to_string());

        let status_code = if runtime_response.has_failed() {
            500
        } else {
            200
        };

        HttpResponse {
            status_code,
            headers,
            body: runtime_response.data.clone(),
            metadata: runtime_response.metadata.clone(),
        }
    }

    /// Convert unknown execution mode to HTTP response / 将未知执行模式转换为 HTTP 响应
    fn unknown_to_http(runtime_response: &RuntimeExecutionResponse) -> HttpResponse {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert(
            "X-Execution-ID".to_string(),
            runtime_response.execution_id.clone(),
        );
        headers.insert("X-Execution-Mode".to_string(), "unknown".to_string());

        let error_response = HttpErrorResponse {
            error_code: "UNKNOWN_EXECUTION_MODE".to_string(),
            error_message: "Unknown execution mode".to_string(),
            execution_id: runtime_response.execution_id.clone(),
            details: HashMap::new(),
        };

        HttpResponse {
            status_code: 400,
            headers,
            body: serde_json::to_vec(&error_response).unwrap_or_default(),
            metadata: runtime_response.metadata.clone(),
        }
    }

    /// Create a standard/// Convert ExecutionResponse to sync HTTP response / 将 ExecutionResponse 转换为同步 HTTP 响应
    pub fn to_sync_response(&self, execution_response: &super::ExecutionResponse) -> HttpResponse {
        let status_code = if execution_response.is_successful() {
            200
        } else {
            500
        };

        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        );
        headers.insert(
            "X-Execution-ID".to_string(),
            execution_response.execution_id.clone(),
        );
        headers.insert("X-Execution-Mode".to_string(), "sync".to_string());
        headers.insert(
            "X-Duration-Ms".to_string(),
            execution_response.execution_time_ms.to_string(),
        );

        let body = if execution_response.is_successful() {
            execution_response.output_data.clone()
        } else {
            serde_json::to_vec(&HttpErrorResponse {
                error_code: "EXECUTION_FAILED".to_string(),
                error_message: execution_response.error_message.clone().unwrap_or_default(),
                execution_id: execution_response.execution_id.clone(),
                details: HashMap::new(),
            })
            .unwrap_or_default()
        };

        HttpResponse {
            status_code,
            headers,
            body,
            metadata: execution_response
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect(),
        }
    }

    /// Convert to async status response / 转换为异步状态响应
    pub fn to_async_status_response(
        &self,
        runtime_response: &RuntimeExecutionResponse,
    ) -> AsyncStatusResponse {
        AsyncStatusResponse {
            execution_id: runtime_response.execution_id.clone(),
            task_id: runtime_response.task_id.clone(),
            status: format!("{:?}", runtime_response.execution_status).to_lowercase(),
            status_endpoint: runtime_response.status_endpoint.clone().unwrap_or_default(),
            estimated_completion_ms: runtime_response.estimated_completion_ms,
            is_completed: runtime_response.is_completed(),
            result: if runtime_response.is_completed() && runtime_response.error.is_none() {
                Some(runtime_response.data.clone())
            } else {
                None
            },
            error: runtime_response.error.as_ref().map(|e| {
                let (error_code, error_message) = Self::error_to_code_and_message(e);
                HttpErrorResponse {
                    error_code,
                    error_message,
                    execution_id: runtime_response.execution_id.clone(),
                    details: HashMap::new(),
                }
            }),
        }
    }

    /// Create error response / 创建错误响应
    pub fn create_error_response(
        execution_id: String,
        error_code: String,
        error_message: String,
        status_code: u16,
    ) -> HttpResponse {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Execution-ID".to_string(), execution_id.clone());

        let error_response = HttpErrorResponse {
            error_code,
            error_message,
            execution_id,
            details: HashMap::new(),
        };

        HttpResponse {
            status_code,
            headers,
            body: serde_json::to_vec(&error_response).unwrap_or_default(),
            metadata: HashMap::new(),
        }
    }
}

impl Default for HttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self {
            status_code: 200,
            headers: HashMap::new(),
            body: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::runtime::{ExecutionMode, ExecutionStatus};

    #[test]
    fn test_sync_response_conversion() {
        let runtime_response = RuntimeExecutionResponse::new_sync(
            "test-exec-123".to_string(),
            b"Hello, World!".to_vec(),
            150,
        );

        let http_response = HttpAdapter::to_http_response(&runtime_response);

        assert_eq!(http_response.status_code, 200);
        assert_eq!(http_response.body, b"Hello, World!");
        assert_eq!(
            http_response.headers.get("X-Execution-Mode"),
            Some(&"sync".to_string())
        );
    }

    #[test]
    fn test_async_response_conversion() {
        let runtime_response = RuntimeExecutionResponse::new_async(
            "test-exec-456".to_string(),
            Some("task-789".to_string()),
            "http://localhost:8080/status/test-exec-456".to_string(),
            Some(5000),
        );

        let http_response = HttpAdapter::to_http_response(&runtime_response);

        assert_eq!(http_response.status_code, 202);
        assert_eq!(
            http_response.headers.get("X-Execution-Mode"),
            Some(&"async".to_string())
        );

        let status_response: AsyncStatusResponse =
            serde_json::from_slice(&http_response.body).unwrap();
        assert_eq!(status_response.execution_id, "test-exec-456");
        assert_eq!(status_response.task_id, Some("task-789".to_string()));
    }

    #[test]
    fn test_error_response_conversion() {
        let runtime_response = RuntimeExecutionResponse::new_failed(
            "test-exec-error".to_string(),
            ExecutionMode::Sync,
            RuntimeExecutionError::RuntimeError {
                message: "Something went wrong".to_string(),
            },
            100,
        );

        let http_response = HttpAdapter::to_http_response(&runtime_response);

        assert_eq!(http_response.status_code, 500);

        let error_response: HttpErrorResponse =
            serde_json::from_slice(&http_response.body).unwrap();
        assert_eq!(error_response.error_code, "RUNTIME_ERROR");
        assert_eq!(error_response.error_message, "Something went wrong");
    }
}
