//! Function service tests / 函数服务测试

use crate::spearlet::function_service::{FunctionServiceImpl, FunctionServiceStats};
use std::sync::Arc;

#[tokio::test]
async fn test_function_service_creation() {
    // Test creating function service with memory store / 测试使用内存存储创建函数服务
    let service = FunctionServiceImpl::new(Arc::new(crate::spearlet::SpearletConfig::default()))
        .await
        .unwrap();

    // Verify initial state / 验证初始状态
    let stats = service.get_stats().await;
    assert_eq!(stats.task_count, 0);
    assert_eq!(stats.execution_count, 0);
    assert_eq!(stats.running_executions, 0);
}

#[tokio::test]
async fn test_function_service_stats() {
    // Test function service statistics / 测试函数服务统计信息
    let service = FunctionServiceImpl::new(Arc::new(crate::spearlet::SpearletConfig::default()))
        .await
        .unwrap();

    let stats = service.get_stats().await;

    // Verify stats structure / 验证统计结构
    assert_eq!(stats.task_count, 0);
    assert_eq!(stats.execution_count, 0);
    assert_eq!(stats.running_executions, 0);
}

// TODO: Add integration tests when proto types are generated
// TODO: 当proto类型生成后添加集成测试
// Integration tests will be added after the proto files are regenerated
// 在proto文件重新生成后将添加集成测试
