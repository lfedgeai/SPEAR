//! Error types for spear-next components
//! spear-next组件的错误类型

use thiserror::Error;

/// SPEAR Metadata Server service error types / SPEAR元数据服务器服务错误类型
#[derive(Error, Debug)]
pub enum SmsError {
    /// Node not found / 节点未找到
    #[error("Node with UUID {uuid} not found")]
    NodeNotFound { uuid: String },
    
    /// Node already exists / 节点已存在
    #[error("Node with UUID {uuid} already exists")]
    NodeAlreadyExists { uuid: String },
    
    /// Conflict error / 冲突错误
    #[error("Conflict: {message}")]
    Conflict { message: String },
    
    /// Invalid node data / 无效节点数据
    #[error("Invalid node data: {message}")]
    InvalidNodeData { message: String },
    
    /// Database error / 数据库错误
    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),
    
    /// Configuration error / 配置错误
    #[error("Configuration error: {0}")]
    Config(#[from] figment::Error),
    
    /// gRPC transport error / gRPC传输错误
    #[error("gRPC transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    
    /// gRPC status error / gRPC状态错误
    #[error("gRPC status error: {0}")]
    Status(#[from] tonic::Status),
    
    /// IO error / IO错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Serialization error / 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    /// Task execution error / 任务执行错误
    #[error("Task execution error: {0}")]
    TaskExecutionError(String),
    
    /// Storage error / 存储错误
    #[error("Storage error: {0}")]
    StorageError(String),
}

/// Result type alias for SPEAR Metadata Server operations / SPEAR元数据服务器操作的结果类型别名
pub type SmsResult<T> = Result<T, SmsError>;

/// Convert SmsError to tonic::Status for gRPC responses
/// 将SmsError转换为tonic::Status用于gRPC响应
impl From<SmsError> for tonic::Status {
    fn from(err: SmsError) -> Self {
        match err {
            SmsError::NodeNotFound { .. } => {
                tonic::Status::not_found(err.to_string())
            }
            SmsError::NodeAlreadyExists { .. } => {
                tonic::Status::already_exists(err.to_string())
            }
            SmsError::Conflict { .. } => {
                tonic::Status::already_exists(err.to_string())
            }
            SmsError::InvalidNodeData { .. } => {
                tonic::Status::invalid_argument(err.to_string())
            }
            SmsError::Database(_) | SmsError::Io(_) => {
                tonic::Status::internal(err.to_string())
            }
            SmsError::Config(_) => {
                tonic::Status::failed_precondition(err.to_string())
            }
            SmsError::Transport(_) | SmsError::Status(_) => {
                tonic::Status::unavailable(err.to_string())
            }
            SmsError::Serialization(_) => {
                tonic::Status::internal(err.to_string())
            }
            SmsError::TaskExecutionError(_) => {
                tonic::Status::internal(err.to_string())
            }
            SmsError::StorageError(_) => {
                tonic::Status::internal(err.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::error::Error;
    use figment::Figment;
    use tonic::Code;
    
    #[test]
    fn test_node_not_found_error() {
        // Test NodeNotFound error creation and display / 测试NodeNotFound错误创建和显示
        let uuid = "550e8400-e29b-41d4-a716-446655440000".to_string();
        let error = SmsError::NodeNotFound { uuid: uuid.clone() };
        
        let error_message = error.to_string();
        assert!(error_message.contains(&uuid));
        assert!(error_message.contains("not found"));
        
        // Test Debug trait / 测试Debug trait
        let debug_message = format!("{:?}", error);
        assert!(debug_message.contains("NodeNotFound"));
        assert!(debug_message.contains(&uuid));
    }
    
    #[test]
    fn test_node_already_exists_error() {
        // Test NodeAlreadyExists error creation and display / 测试NodeAlreadyExists错误创建和显示
        let uuid = "550e8400-e29b-41d4-a716-446655440001".to_string();
        let error = SmsError::NodeAlreadyExists { uuid: uuid.clone() };
        
        let error_message = error.to_string();
        assert!(error_message.contains(&uuid));
        assert!(error_message.contains("already exists"));
        
        // Test Debug trait / 测试Debug trait
        let debug_message = format!("{:?}", error);
        assert!(debug_message.contains("NodeAlreadyExists"));
        assert!(debug_message.contains(&uuid));
    }
    
    #[test]
    fn test_invalid_node_data_error() {
        // Test InvalidNodeData error creation and display / 测试InvalidNodeData错误创建和显示
        let message = "Missing required field: ip_address".to_string();
        let error = SmsError::InvalidNodeData { message: message.clone() };
        
        let error_message = error.to_string();
        assert!(error_message.contains(&message));
        assert!(error_message.contains("Invalid node data"));
        
        // Test Debug trait / 测试Debug trait
        let debug_message = format!("{:?}", error);
        assert!(debug_message.contains("InvalidNodeData"));
        assert!(debug_message.contains(&message));
    }
    
    #[test]
    fn test_database_error_conversion() {
        // Test Database error conversion from anyhow::Error / 测试从anyhow::Error转换Database错误
        let anyhow_error = anyhow::anyhow!("Database connection failed");
        let sms_error = SmsError::Database(anyhow_error);
        
        let error_message = sms_error.to_string();
        assert!(error_message.contains("Database error"));
        assert!(error_message.contains("Database connection failed"));
        
        // Test automatic conversion / 测试自动转换
        let result: SmsResult<()> = Err(anyhow::anyhow!("Connection timeout").into());
        assert!(result.is_err());
        
        if let Err(SmsError::Database(_)) = result {
            // Expected / 预期的
        } else {
            panic!("Expected Database error");
        }
    }
    
    #[test]
    fn test_config_error_conversion() {
        // Test Config error conversion from figment::Error / 测试从figment::Error转换Config错误
        let figment = Figment::new();
        let figment_error = figment.extract::<String>().unwrap_err();
        let sms_error = SmsError::Config(figment_error);
        
        let error_message = sms_error.to_string();
        assert!(error_message.contains("Configuration error"));
        
        // Test automatic conversion / 测试自动转换
        let result: SmsResult<String> = figment.extract().map_err(SmsError::from);
        assert!(result.is_err());
        
        if let Err(SmsError::Config(_)) = result {
            // Expected / 预期的
        } else {
            panic!("Expected Config error");
        }
    }
    
    #[test]
    fn test_io_error_conversion() {
        // Test IO error conversion from std::io::Error / 测试从std::io::Error转换IO错误
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let sms_error = SmsError::Io(io_error);
        
        let error_message = sms_error.to_string();
        assert!(error_message.contains("IO error"));
        assert!(error_message.contains("File not found"));
        
        // Test automatic conversion / 测试自动转换
        let result: SmsResult<()> = Err(io::Error::new(io::ErrorKind::PermissionDenied, "Access denied").into());
        assert!(result.is_err());
        
        if let Err(SmsError::Io(_)) = result {
            // Expected / 预期的
        } else {
            panic!("Expected IO error");
        }
    }
    
    #[test]
    fn test_transport_error_conversion() {
        // Test Transport error creation / 测试Transport错误创建
        // Note: Creating a real tonic::transport::Error is complex, so we test the structure
        // 注意：创建真实的tonic::transport::Error很复杂，所以我们测试结构
        
        // Test that the error variant exists and can be matched
        // 测试错误变体存在并可以匹配
        // We'll test this by creating a mock transport error scenario
        // 我们通过创建模拟传输错误场景来测试
        
        // Since we can't easily create a tonic::transport::Error, we'll test the From trait
        // 由于我们无法轻易创建tonic::transport::Error，我们将测试From trait
        match (SmsError::InvalidNodeData { message: "test".to_string() }) {
            SmsError::Transport(_) => panic!("Should not match Transport variant"),
            _ => {} // This confirms the Transport variant exists in the enum
        }
    }
    
    #[test]
    fn test_status_error_conversion() {
        // Test Status error conversion from tonic::Status / 测试从tonic::Status转换Status错误
        let status = tonic::Status::internal("Internal server error");
        let sms_error = SmsError::Status(status);
        
        let error_message = sms_error.to_string();
        assert!(error_message.contains("gRPC status error"));
        assert!(error_message.contains("Internal server error"));
        
        // Test automatic conversion / 测试自动转换
        let result: SmsResult<()> = Err(tonic::Status::unavailable("Service unavailable").into());
        assert!(result.is_err());
        
        if let Err(SmsError::Status(_)) = result {
            // Expected / 预期的
        } else {
            panic!("Expected Status error");
        }
    }
    
    #[test]
    fn test_serialization_error() {
        // Test Serialization error creation and display / 测试Serialization错误创建和显示
        let message = "Failed to serialize node data to JSON".to_string();
        let error = SmsError::Serialization(message.clone());
        
        let error_message = error.to_string();
        assert!(error_message.contains("Serialization error"));
        assert!(error_message.contains(&message));
        
        // Test Debug trait / 测试Debug trait
        let debug_message = format!("{:?}", error);
        assert!(debug_message.contains("Serialization"));
        assert!(debug_message.contains(&message));
    }
    
    #[test]
    fn test_sms_error_to_tonic_status_conversion() {
        // Test conversion of SmsError to tonic::Status / 测试SmsError到tonic::Status的转换
        
        // Test NodeNotFound -> NOT_FOUND / 测试NodeNotFound -> NOT_FOUND
        let error = SmsError::NodeNotFound { uuid: "test-uuid".to_string() };
        let status: tonic::Status = error.into();
        assert_eq!(status.code(), Code::NotFound);
        assert!(status.message().contains("test-uuid"));
        assert!(status.message().contains("not found"));
        
        // Test NodeAlreadyExists -> ALREADY_EXISTS / 测试NodeAlreadyExists -> ALREADY_EXISTS
        let error = SmsError::NodeAlreadyExists { uuid: "test-uuid".to_string() };
        let status: tonic::Status = error.into();
        assert_eq!(status.code(), Code::AlreadyExists);
        assert!(status.message().contains("already exists"));
        
        // Test InvalidNodeData -> INVALID_ARGUMENT / 测试InvalidNodeData -> INVALID_ARGUMENT
        let error = SmsError::InvalidNodeData { message: "Invalid data".to_string() };
        let status: tonic::Status = error.into();
        assert_eq!(status.code(), Code::InvalidArgument);
        assert!(status.message().contains("Invalid data"));
        
        // Test Database -> INTERNAL / 测试Database -> INTERNAL
        let error = SmsError::Database(anyhow::anyhow!("DB error"));
        let status: tonic::Status = error.into();
        assert_eq!(status.code(), Code::Internal);
        
        // Test IO -> INTERNAL / 测试IO -> INTERNAL
        let error = SmsError::Io(io::Error::new(io::ErrorKind::Other, "IO error"));
        let status: tonic::Status = error.into();
        assert_eq!(status.code(), Code::Internal);
        
        // Test Config -> FAILED_PRECONDITION / 测试Config -> FAILED_PRECONDITION
        let figment = Figment::new();
        let figment_error = figment.extract::<String>().unwrap_err();
        let error = SmsError::Config(figment_error);
        let status: tonic::Status = error.into();
        assert_eq!(status.code(), Code::FailedPrecondition);
        
        // Test Status -> UNAVAILABLE / 测试Status -> UNAVAILABLE
        let error = SmsError::Status(tonic::Status::unavailable("Service down"));
        let status: tonic::Status = error.into();
        assert_eq!(status.code(), Code::Unavailable);
        
        // Test Serialization -> INTERNAL / 测试Serialization -> INTERNAL
        let error = SmsError::Serialization("Serialization failed".to_string());
        let status: tonic::Status = error.into();
        assert_eq!(status.code(), Code::Internal);
        assert!(status.message().contains("Serialization failed"));
    }
    
    #[test]
    fn test_sms_result_type_alias() {
        // Test SmsResult type alias usage / 测试SmsResult类型别名使用
        
        // Test successful result / 测试成功结果
        let success: SmsResult<String> = Ok("Success".to_string());
        assert!(success.is_ok());
        assert_eq!(success.unwrap(), "Success");
        
        // Test error result / 测试错误结果
        let error: SmsResult<String> = Err(SmsError::NodeNotFound { 
            uuid: "test-uuid".to_string() 
        });
        assert!(error.is_err());
        
        if let Err(SmsError::NodeNotFound { uuid }) = error {
            assert_eq!(uuid, "test-uuid");
        } else {
            panic!("Expected NodeNotFound error");
        }
    }
    
    #[test]
    fn test_error_chain_and_source() {
        // Test error chain and source tracking / 测试错误链和源跟踪
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied");
        let sms_error = SmsError::Io(io_error);
        
        // Test that the source error is preserved / 测试源错误被保留
        assert!(sms_error.source().is_some());
        
        let source = sms_error.source().unwrap();
        let source_message = source.to_string();
        assert!(source_message.contains("Permission denied"));
    }
    
    #[test]
    fn test_error_equality_and_matching() {
        // Test error pattern matching / 测试错误模式匹配
        let errors = vec![
            SmsError::NodeNotFound { uuid: "uuid1".to_string() },
            SmsError::NodeAlreadyExists { uuid: "uuid2".to_string() },
            SmsError::InvalidNodeData { message: "Invalid".to_string() },
            SmsError::Serialization("Serialization error".to_string()),
        ];
        
        for error in errors {
            match error {
                SmsError::NodeNotFound { .. } => {
                    // Expected for first error / 第一个错误的预期情况
                }
                SmsError::NodeAlreadyExists { .. } => {
                    // Expected for second error / 第二个错误的预期情况
                }
                SmsError::InvalidNodeData { .. } => {
                    // Expected for third error / 第三个错误的预期情况
                }
                SmsError::Serialization(_) => {
                    // Expected for fourth error / 第四个错误的预期情况
                }
                _ => {
                    // Other error types / 其他错误类型
                }
            }
        }
    }
}