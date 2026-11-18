//! Error handling macros for reducing boilerplate code
//! 错误处理宏，用于减少样板代码

/// Macro for handling spawn_blocking task execution with automatic error handling
/// 处理 spawn_blocking 任务执行的宏，自动处理错误
#[macro_export]
macro_rules! spawn_blocking_task {
    ($task:expr) => {
        tokio::task::spawn_blocking($task).await.map_err(|e| {
            $crate::sms::services::error::SmsError::Serialization(format!("Task join error: {}", e))
        })?
    };
}

/// Macro for handling Sled database errors
/// 处理 Sled 数据库错误的宏
#[macro_export]
macro_rules! handle_sled_error {
    ($operation:expr, $op_name:expr) => {
        $operation.map_err(|e| {
            $crate::sms::services::error::SmsError::Serialization(format!(
                "Sled {} error: {}",
                $op_name, e
            ))
        })
    };
}
