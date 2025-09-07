//! Common types and utilities for HTTP handlers
//! HTTP处理器的通用类型和工具

use serde::Serialize;

/// Standard error response structure for all HTTP handlers
/// 所有HTTP处理器的标准错误响应结构
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}