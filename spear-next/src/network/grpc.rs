//! Shared gRPC error handling utilities
//! 共享的gRPC错误处理工具

use tonic::{Status, Code};

/// Error handling utilities / 错误处理工具
pub mod errors {
    use super::*;
    
    /// Convert common errors to gRPC status / 将常见错误转换为gRPC状态
    pub fn to_grpc_status(error: &anyhow::Error) -> Status {
        // Check for specific error types / 检查特定的错误类型
        if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
            match io_error.kind() {
                std::io::ErrorKind::NotFound => {
                    Status::new(Code::NotFound, "Resource not found")
                }
                std::io::ErrorKind::PermissionDenied => {
                    Status::new(Code::PermissionDenied, "Permission denied")
                }
                std::io::ErrorKind::TimedOut => {
                    Status::new(Code::DeadlineExceeded, "Operation timed out")
                }
                _ => Status::new(Code::Internal, "Internal server error"),
            }
        } else {
            // Default to internal error / 默认为内部错误
            Status::new(Code::Internal, format!("Internal error: {}", error))
        }
    }
    
    /// Create a not found status / 创建未找到状态
    pub fn not_found(message: &str) -> Status {
        Status::new(Code::NotFound, message)
    }
    
    /// Create an invalid argument status / 创建无效参数状态
    pub fn invalid_argument(message: &str) -> Status {
        Status::new(Code::InvalidArgument, message)
    }
    
    /// Create an already exists status / 创建已存在状态
    pub fn already_exists(message: &str) -> Status {
        Status::new(Code::AlreadyExists, message)
    }
    
    /// Create an unavailable status / 创建不可用状态
    pub fn unavailable(message: &str) -> Status {
        Status::new(Code::Unavailable, message)
    }
}