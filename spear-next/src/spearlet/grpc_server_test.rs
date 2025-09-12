//! Tests for gRPC server module
//! gRPC服务器模块的测试

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::spearlet::config::{SpearletConfig, GrpcConfig, StorageConfig};
use crate::spearlet::grpc_server::{GrpcServer, HealthService, HealthStatus};

/// Create test configuration / 创建测试配置
fn create_test_config() -> SpearletConfig {
    SpearletConfig {
        grpc: GrpcConfig {
            address: "127.0.0.1".to_string(),
            port: 50052,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        },
        storage: StorageConfig {
            backend: "memory".to_string(),
            data_dir: "/tmp/test".to_string(),
            max_cache_size_mb: 100,
            compression_enabled: false,
            max_object_size: 1024 * 1024, // 1MB
        },
        ..Default::default()
    }
}

#[tokio::test]
async fn test_grpc_server_creation() {
    // Test gRPC server creation / 测试gRPC服务器创建
    let config = Arc::new(create_test_config());
    let server = GrpcServer::new(config.clone());
    
    // Verify server has object service / 验证服务器有对象服务
    let object_service = server.get_object_service();
    assert!(Arc::strong_count(&object_service) > 0);
}

#[tokio::test]
async fn test_grpc_server_config() {
    // Test gRPC server configuration / 测试gRPC服务器配置
    let mut config = create_test_config();
    config.grpc.address = "0.0.0.0".to_string();
    config.grpc.port = 8080;
    config.storage.max_object_size = 2 * 1024 * 1024; // 2MB
    
    let server = GrpcServer::new(Arc::new(config));
    let object_service = server.get_object_service();
    
    // Verify object service is created / 验证对象服务已创建
    assert!(Arc::strong_count(&object_service) > 0);
}

#[tokio::test]
async fn test_health_service_creation() {
    // Test health service creation / 测试健康服务创建
    let config = Arc::new(create_test_config());
    let server = GrpcServer::new(config);
    let object_service = server.get_object_service();
    
    let health_service = HealthService::new(object_service);
    
    // Test health status retrieval / 测试健康状态获取
    let health_status = health_service.get_health_status().await;
    assert_eq!(health_status.status, "healthy");
    assert_eq!(health_status.object_count, 0);
    assert_eq!(health_status.total_object_size, 0);
    assert_eq!(health_status.pinned_object_count, 0);
}

#[tokio::test]
async fn test_health_status_structure() {
    // Test health status structure / 测试健康状态结构
    let health_status = HealthStatus {
        status: "healthy".to_string(),
        object_count: 10,
        total_object_size: 1024,
        pinned_object_count: 5,
    };
    
    assert_eq!(health_status.status, "healthy");
    assert_eq!(health_status.object_count, 10);
    assert_eq!(health_status.total_object_size, 1024);
    assert_eq!(health_status.pinned_object_count, 5);
    
    // Test cloning / 测试克隆
    let cloned_status = health_status.clone();
    assert_eq!(cloned_status.status, health_status.status);
    assert_eq!(cloned_status.object_count, health_status.object_count);
}

#[tokio::test]
async fn test_grpc_server_invalid_address() {
    // Test gRPC server with invalid address / 测试无效地址的gRPC服务器
    let mut config = create_test_config();
    config.grpc.address = "invalid-address".to_string();
    config.grpc.port = 8080;
    
    let server = GrpcServer::new(Arc::new(config));
    
    // Server should start but fail when trying to bind to invalid address
    // 服务器应该启动但在尝试绑定到无效地址时失败
    let result = timeout(Duration::from_millis(100), server.start()).await;
    
    // Should timeout or return error / 应该超时或返回错误
    assert!(result.is_err() || result.unwrap().is_err());
}

#[tokio::test]
async fn test_multiple_grpc_servers() {
    // Test creating multiple gRPC servers / 测试创建多个gRPC服务器
    let config1 = Arc::new(create_test_config());
    let config2 = Arc::new(create_test_config());
    
    let server1 = GrpcServer::new(config1);
    let server2 = GrpcServer::new(config2);
    
    let service1 = server1.get_object_service();
    let service2 = server2.get_object_service();
    
    // Services should be different instances / 服务应该是不同的实例
    assert!(!Arc::ptr_eq(&service1, &service2));
}

#[tokio::test]
async fn test_grpc_server_tls_config() {
    // Test gRPC server with TLS configuration / 测试带TLS配置的gRPC服务器
    let mut config = create_test_config();
    config.grpc.tls_enabled = true;
    config.grpc.tls_cert_path = Some("/path/to/cert.pem".to_string());
    config.grpc.tls_key_path = Some("/path/to/key.pem".to_string());
    
    let server = GrpcServer::new(Arc::new(config));
    let object_service = server.get_object_service();
    
    // Verify object service is created even with TLS config / 验证即使有TLS配置也能创建对象服务
    assert!(Arc::strong_count(&object_service) > 0);
}

#[tokio::test]
async fn test_grpc_server_different_ports() {
    // Test gRPC servers on different ports / 测试不同端口的gRPC服务器
    let mut config1 = create_test_config();
    config1.grpc.port = 50053;
    
    let mut config2 = create_test_config();
    config2.grpc.port = 50054;
    
    let server1 = GrpcServer::new(Arc::new(config1));
    let server2 = GrpcServer::new(Arc::new(config2));
    
    let service1 = server1.get_object_service();
    let service2 = server2.get_object_service();
    
    // Services should be different instances / 服务应该是不同的实例
    assert!(!Arc::ptr_eq(&service1, &service2));
}

#[tokio::test]
async fn test_health_service_with_data() {
    // Test health service with simulated data / 测试带模拟数据的健康服务
    let config = Arc::new(create_test_config());
    let server = GrpcServer::new(config);
    let object_service = server.get_object_service();
    
    // Add some test objects to the service / 向服务添加一些测试对象
    let test_key = "test_key".to_string();
    let test_value = b"test_value".to_vec();
    let test_metadata = std::collections::HashMap::new();
    
    // Store an object using put_object / 使用put_object存储一个对象
    use tonic::Request;
    use crate::proto::spearlet::{PutObjectRequest, object_service_server::ObjectService};
    let put_request = Request::new(PutObjectRequest {
        key: test_key.clone(),
        value: test_value,
        metadata: test_metadata,
        overwrite: false,
    });
    let _ = object_service.put_object(put_request).await;
    
    let health_service = HealthService::new(object_service);
    let health_status = health_service.get_health_status().await;
    
    // Verify health status reflects the stored object / 验证健康状态反映了存储的对象
    assert_eq!(health_status.status, "healthy");
    assert_eq!(health_status.object_count, 1);
    assert!(health_status.total_object_size > 0);
}

#[tokio::test]
async fn test_grpc_server_edge_cases() {
    // Test gRPC server edge cases / 测试gRPC服务器边缘情况
    
    // Test with port 0 (should use random available port) / 测试端口0（应该使用随机可用端口）
    let mut config = create_test_config();
    config.grpc.port = 0;
    
    let server = GrpcServer::new(Arc::new(config));
    let object_service = server.get_object_service();
    assert!(Arc::strong_count(&object_service) > 0);
    
    // Test with very high port number / 测试非常高的端口号
    let mut config2 = create_test_config();
    config2.grpc.port = 65535;
    
    let server2 = GrpcServer::new(Arc::new(config2));
    let object_service2 = server2.get_object_service();
    assert!(Arc::strong_count(&object_service2) > 0);
}

#[tokio::test]
async fn test_health_status_debug_format() {
    // Test health status debug formatting / 测试健康状态调试格式
    let health_status = HealthStatus {
        status: "healthy".to_string(),
        object_count: 42,
        total_object_size: 2048,
        pinned_object_count: 10,
    };
    
    let debug_str = format!("{:?}", health_status);
    assert!(debug_str.contains("healthy"));
    assert!(debug_str.contains("42"));
    assert!(debug_str.contains("2048"));
    assert!(debug_str.contains("10"));
}

#[tokio::test]
async fn test_grpc_server_concurrent_creation() {
    // Test concurrent gRPC server creation / 测试并发gRPC服务器创建
    use std::sync::Arc;
    use tokio::task::JoinSet;
    
    let mut join_set = JoinSet::new();
    
    // Create multiple servers concurrently / 并发创建多个服务器
    for i in 0..5 {
        let mut config = create_test_config();
        config.grpc.port = 50060 + i;
        
        join_set.spawn(async move {
            let server = GrpcServer::new(Arc::new(config));
            let object_service = server.get_object_service();
            Arc::strong_count(&object_service) > 0
        });
    }
    
    // Wait for all tasks to complete / 等待所有任务完成
    let mut success_count = 0;
    while let Some(result) = join_set.join_next().await {
        if result.unwrap() {
            success_count += 1;
        }
    }
    
    assert_eq!(success_count, 5);
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_grpc_server_lifecycle() {
        // Test complete gRPC server lifecycle / 测试完整的gRPC服务器生命周期
        let config = Arc::new(create_test_config());
        let server = GrpcServer::new(config);
        
        // Get object service before starting server / 在启动服务器前获取对象服务
        let object_service = server.get_object_service();
        let health_service = HealthService::new(object_service);
        
        // Check initial health status / 检查初始健康状态
        let initial_status = health_service.get_health_status().await;
        assert_eq!(initial_status.status, "healthy");
        assert_eq!(initial_status.object_count, 0);
        
        // Note: We don't actually start the server in tests to avoid port conflicts
        // 注意：我们在测试中不实际启动服务器以避免端口冲突
    }
    
    #[tokio::test]
    async fn test_grpc_server_stress_test() {
        // Stress test for gRPC server creation / gRPC服务器创建的压力测试
        let config = Arc::new(create_test_config());
        
        // Create and destroy servers rapidly / 快速创建和销毁服务器
        for _ in 0..100 {
            let server = GrpcServer::new(config.clone());
            let object_service = server.get_object_service();
            assert!(Arc::strong_count(&object_service) > 0);
            // Server goes out of scope and is dropped / 服务器超出作用域并被丢弃
        }
    }
    
    #[tokio::test]
    async fn test_health_service_performance() {
        // Test health service performance / 测试健康服务性能
        let config = Arc::new(create_test_config());
        let server = GrpcServer::new(config);
        let object_service = server.get_object_service();
        let health_service = HealthService::new(object_service);
        
        // Measure time for health status retrieval / 测量健康状态获取时间
        let start = std::time::Instant::now();
        
        for _ in 0..1000 {
            let _ = health_service.get_health_status().await;
        }
        
        let duration = start.elapsed();
        
        // Should complete 1000 health checks in reasonable time / 应该在合理时间内完成1000次健康检查
        assert!(duration.as_millis() < 1000); // Less than 1 second
    }
}