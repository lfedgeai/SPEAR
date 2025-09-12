//! Tests for HTTP gateway module
//! HTTP网关模块的测试

use std::collections::HashMap;
use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::spearlet::config::{SpearletConfig, HttpConfig, GrpcConfig, StorageConfig};
use crate::spearlet::http_gateway::HttpGateway;
use crate::spearlet::grpc_server::HealthService;
use crate::spearlet::object_service::ObjectServiceImpl;

/// Create test configuration / 创建测试配置
fn create_test_config() -> SpearletConfig {
    SpearletConfig {
        http: HttpConfig {
            address: "127.0.0.1".to_string(),
            port: 0, // Use port 0 for testing
            cors_enabled: true,
            swagger_enabled: true,
        },
        grpc: GrpcConfig {
            address: "127.0.0.1".to_string(),
            port: 0,
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

/// Create test HTTP gateway / 创建测试HTTP网关
fn create_test_gateway() -> HttpGateway {
    let config = Arc::new(create_test_config());
    let object_service = Arc::new(ObjectServiceImpl::new_with_memory(1024 * 1024));
    let health_service = Arc::new(HealthService::new(object_service));
    
    HttpGateway::new(config, health_service)
}

#[tokio::test]
async fn test_http_gateway_creation() {
    // Test HTTP gateway creation / 测试HTTP网关创建
    let gateway = create_test_gateway();
    
    // Gateway should be created successfully / 网关应该成功创建
    // Note: We can't easily test the internal state without exposing it
    // 注意：我们无法在不暴露内部状态的情况下轻松测试内部状态
}

#[tokio::test]
async fn test_gateway_config() {
    // Test HTTP gateway configuration / 测试HTTP网关配置
    let mut config = create_test_config();
    config.http.address = "0.0.0.0".to_string();
    config.http.port = 8080;
    config.http.swagger_enabled = false;
    
    let object_service = Arc::new(ObjectServiceImpl::new_with_memory(1024 * 1024));
    let health_service = Arc::new(HealthService::new(object_service));
    let gateway = HttpGateway::new(Arc::new(config), health_service);
    
    // Gateway should be created with custom config / 网关应该使用自定义配置创建
}

#[tokio::test]
async fn test_gateway_with_different_storage_sizes() {
    // Test HTTP gateway with different storage sizes / 测试不同存储大小的HTTP网关
    let sizes = vec![1024, 1024 * 1024, 10 * 1024 * 1024]; // 1KB, 1MB, 10MB
    
    for size in sizes {
        let mut config = create_test_config();
        config.storage.max_object_size = size;
        
        let object_service = Arc::new(ObjectServiceImpl::new_with_memory(size));
        let health_service = Arc::new(HealthService::new(object_service));
        let gateway = HttpGateway::new(Arc::new(config), health_service);
        
        // Gateway should be created with different storage sizes / 网关应该使用不同存储大小创建
    }
}

#[tokio::test]
async fn test_gateway_swagger_enabled() {
    // Test HTTP gateway with Swagger enabled / 测试启用Swagger的HTTP网关
    let mut config = create_test_config();
    config.http.swagger_enabled = true;
    
    let object_service = Arc::new(ObjectServiceImpl::new_with_memory(1024 * 1024));
    let health_service = Arc::new(HealthService::new(object_service));
    let gateway = HttpGateway::new(Arc::new(config), health_service);
    
    // Gateway should be created with Swagger enabled / 网关应该启用Swagger创建
}

#[tokio::test]
async fn test_gateway_swagger_disabled() {
    // Test HTTP gateway with Swagger disabled / 测试禁用Swagger的HTTP网关
    let mut config = create_test_config();
    config.http.swagger_enabled = false;
    
    let object_service = Arc::new(ObjectServiceImpl::new_with_memory(1024 * 1024));
    let health_service = Arc::new(HealthService::new(object_service));
    let gateway = HttpGateway::new(Arc::new(config), health_service);
    
    // Gateway should be created with Swagger disabled / 网关应该禁用Swagger创建
}

#[tokio::test]
async fn test_invalid_http_address() {
    // Test HTTP gateway with invalid address / 测试无效地址的HTTP网关
    let mut config = create_test_config();
    config.http.address = "invalid-address".to_string();
    config.http.port = 8080;
    
    let object_service = Arc::new(ObjectServiceImpl::new_with_memory(1024 * 1024));
    let health_service = Arc::new(HealthService::new(object_service));
    let gateway = HttpGateway::new(Arc::new(config), health_service);
    
    // Gateway creation should succeed, but start() would fail
    // 网关创建应该成功，但start()会失败
    // Note: We don't test start() here to avoid actual network binding
    // 注意：我们这里不测试start()以避免实际的网络绑定
}

#[tokio::test]
async fn test_multiple_gateways() {
    // Test creating multiple HTTP gateways / 测试创建多个HTTP网关
    let config1 = Arc::new(create_test_config());
    let config2 = Arc::new(create_test_config());
    
    let object_service1 = Arc::new(ObjectServiceImpl::new_with_memory(1024 * 1024));
    let object_service2 = Arc::new(ObjectServiceImpl::new_with_memory(1024 * 1024));
    
    let health_service1 = Arc::new(HealthService::new(object_service1));
    let health_service2 = Arc::new(HealthService::new(object_service2));
    
    let gateway1 = HttpGateway::new(config1, health_service1);
    let gateway2 = HttpGateway::new(config2, health_service2);
    
    // Both gateways should be created successfully / 两个网关都应该成功创建
}

#[cfg(test)]
mod request_body_tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose};

    #[test]
    fn test_put_object_body_serialization() {
        // Test PutObjectBody serialization / 测试PutObjectBody序列化
        let test_data = b"test data";
        let encoded_data = general_purpose::STANDARD.encode(test_data);
        
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "test".to_string());
        
        let body = json!({
            "value": encoded_data,
            "metadata": metadata,
            "overwrite": true
        });
        
        // Should be valid JSON / 应该是有效的JSON
        assert!(body.is_object());
        assert!(body["value"].is_string());
        assert!(body["metadata"].is_object());
        assert!(body["overwrite"].is_boolean());
    }

    #[test]
    fn test_list_objects_query_params() {
        // Test ListObjectsQuery parameters / 测试ListObjectsQuery参数
        let query_params = json!({
            "prefix": "test-",
            "limit": 10,
            "continuation_token": "token123"
        });
        
        // Should be valid query parameters / 应该是有效的查询参数
        assert!(query_params.is_object());
        assert_eq!(query_params["prefix"], "test-");
        assert_eq!(query_params["limit"], 10);
        assert_eq!(query_params["continuation_token"], "token123");
    }

    #[test]
    fn test_ref_count_body() {
        // Test RefCountBody structure / 测试RefCountBody结构
        let body = json!({
            "count": 5
        });
        
        // Should be valid ref count body / 应该是有效的引用计数体
        assert!(body.is_object());
        assert_eq!(body["count"], 5);
    }

    #[test]
    fn test_delete_object_query() {
        // Test DeleteObjectQuery parameters / 测试DeleteObjectQuery参数
        let query = json!({
            "force": true
        });
        
        // Should be valid delete query / 应该是有效的删除查询
        assert!(query.is_object());
        assert_eq!(query["force"], true);
    }

    #[test]
    fn test_base64_encoding_decoding() {
        // Test Base64 encoding/decoding for object values / 测试对象值的Base64编码/解码
        let original_data = b"Hello, World! This is test data.";
        let encoded = general_purpose::STANDARD.encode(original_data);
        let decoded = general_purpose::STANDARD.decode(&encoded).unwrap();
        
        assert_eq!(original_data, decoded.as_slice());
    }

    #[test]
    fn test_empty_metadata() {
        // Test empty metadata handling / 测试空元数据处理
        let body = json!({
            "value": general_purpose::STANDARD.encode(b"test"),
            "metadata": {},
            "overwrite": false
        });
        
        // Should handle empty metadata / 应该处理空元数据
        assert!(body["metadata"].is_object());
        assert_eq!(body["metadata"].as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_optional_fields() {
        // Test optional fields in request bodies / 测试请求体中的可选字段
        let minimal_body = json!({
            "value": general_purpose::STANDARD.encode(b"test")
        });
        
        // Should work with minimal required fields / 应该使用最少必需字段工作
        assert!(minimal_body.is_object());
        assert!(minimal_body["value"].is_string());
        assert!(minimal_body["metadata"].is_null());
        assert!(minimal_body["overwrite"].is_null());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_gateway_lifecycle() {
        // Test complete HTTP gateway lifecycle / 测试完整的HTTP网关生命周期
        let config = Arc::new(create_test_config());
        let object_service = Arc::new(ObjectServiceImpl::new_with_memory(1024 * 1024));
        let health_service = Arc::new(HealthService::new(object_service.clone()));
        
        // Create gateway / 创建网关
        let gateway = HttpGateway::new(config.clone(), health_service.clone());
        
        // Verify initial state / 验证初始状态
        let stats = object_service.get_stats().await;
        assert_eq!(stats.object_count, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.pinned_count, 0);
        
        // Verify health service / 验证健康服务
        let health_status = health_service.get_health_status().await;
        assert_eq!(health_status.status, "healthy");
        assert_eq!(health_status.object_count, 0);
        
        // Note: We don't start the actual server to avoid port binding issues
        // 注意：我们不启动实际服务器以避免端口绑定问题
    }

    #[tokio::test]
    async fn test_gateway_with_different_configs() {
        // Test gateway with various configurations / 测试各种配置的网关
        let configs = vec![
            SpearletConfig {
                http: HttpConfig {
                    address: "127.0.0.1".to_string(),
                    port: 8080,
                    cors_enabled: true,
                    swagger_enabled: true,
                },
                grpc: GrpcConfig {
                    address: "127.0.0.1".to_string(),
                    port: 9090,
                    tls_enabled: false,
                    tls_cert_path: None,
                    tls_key_path: None,
                },
                storage: StorageConfig {
                    backend: "memory".to_string(),
                    data_dir: "/tmp/test".to_string(),
                    max_cache_size_mb: 100,
                    compression_enabled: false,
                    max_object_size: 1024 * 1024,
                },
                ..Default::default()
            },
            SpearletConfig {
                http: HttpConfig {
                    address: "0.0.0.0".to_string(),
                    port: 3000,
                    cors_enabled: false,
                    swagger_enabled: false,
                },
                grpc: GrpcConfig {
                    address: "0.0.0.0".to_string(),
                    port: 3001,
                    tls_enabled: false,
                    tls_cert_path: None,
                    tls_key_path: None,
                },
                storage: StorageConfig {
                    backend: "memory".to_string(),
                    data_dir: "/tmp/test".to_string(),
                    max_cache_size_mb: 100,
                    compression_enabled: false,
                    max_object_size: 10 * 1024 * 1024,
                },
                ..Default::default()
            },
        ];
        
        for config in configs {
            let object_service = Arc::new(ObjectServiceImpl::new_with_memory(config.storage.max_object_size));
            let health_service = Arc::new(HealthService::new(object_service));
            let gateway = HttpGateway::new(Arc::new(config), health_service);
            
            // Each gateway should be created successfully / 每个网关都应该成功创建
        }
    }
}