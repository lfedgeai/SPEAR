/// Error handling macros for reducing boilerplate code
/// 错误处理宏，用于减少样板代码

/// Macro for handling UUID parsing errors
/// 处理 UUID 解析错误的宏
#[macro_export]
macro_rules! parse_uuid {
    ($uuid_str:expr, $context:expr) => {
        uuid::Uuid::parse_str($uuid_str)
            .map_err(|e| crate::services::error::SmsError::Serialization(
                format!("Invalid UUID in {}: {}", $context, e)
            ))
    };
}

/// Macro for handling task join errors
/// 处理任务连接错误的宏
#[macro_export]
macro_rules! handle_task_join {
    ($task:expr) => {
        $task.map_err(|e| crate::services::error::SmsError::Serialization(
            format!("Task join error: {}", e)
        ))
    };
}

/// Macro for handling spawn_blocking task execution with automatic error handling
/// 处理 spawn_blocking 任务执行的宏，自动处理错误
#[macro_export]
macro_rules! spawn_blocking_task {
    ($task:expr) => {
        tokio::task::spawn_blocking($task)
            .await
            .map_err(|e| crate::services::error::SmsError::Serialization(
                format!("Task join error: {}", e)
            ))?
    };
}

/// Macro for handling Sled database errors
/// 处理 Sled 数据库错误的宏
#[macro_export]
macro_rules! handle_sled_error {
    ($operation:expr, $op_name:expr) => {
        $operation.map_err(|e| crate::services::error::SmsError::Serialization(
            format!("Sled {} error: {}", $op_name, e)
        ))
    };
}

/// Macro for handling RocksDB errors
/// 处理 RocksDB 错误的宏
#[macro_export]
macro_rules! handle_rocksdb_error {
    ($operation:expr, $op_name:expr) => {
        $operation.map_err(|e| crate::services::error::SmsError::Serialization(
            format!("RocksDB {} error: {}", $op_name, e)
        ))
    };
}

/// Macro for handling UTF-8 conversion errors
/// 处理 UTF-8 转换错误的宏
#[macro_export]
macro_rules! handle_utf8_error {
    ($operation:expr) => {
        $operation.map_err(|e| crate::services::error::SmsError::Serialization(
            format!("Invalid UTF-8 key: {}", e)
        ))
    };
}

/// Macro for handling gRPC status errors with invalid argument
/// 处理 gRPC 状态错误（无效参数）的宏
#[macro_export]
macro_rules! grpc_invalid_arg {
    ($operation:expr, $context:expr) => {
        $operation.map_err(|e| tonic::Status::invalid_argument(
            format!("Invalid {}: {}", $context, e)
        ))
    };
}

/// Macro for handling gRPC internal errors
/// 处理 gRPC 内部错误的宏
#[macro_export]
macro_rules! grpc_internal_error {
    ($operation:expr, $context:expr) => {
        $operation.map_err(|e| tonic::Status::internal(
            format!("Failed to {}: {}", $context, e)
        ))
    };
}