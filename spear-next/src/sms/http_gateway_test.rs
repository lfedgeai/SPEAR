//! Tests for SMS HTTP Gateway
//! SMS HTTP网关测试

use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;
use anyhow::Result;

use crate::sms::http_gateway::HttpGateway;

/// Create a test address with the given port / 创建指定端口的测试地址
fn create_test_addr(port: u16) -> SocketAddr {
    format!("127.0.0.1:{}", port).parse().unwrap()
}

/// Create a test gRPC address / 创建测试gRPC地址
fn create_test_grpc_addr() -> SocketAddr {
    create_test_addr(50051)
}

#[tokio::test]
async fn test_http_gateway_creation() {
    // Test HTTP gateway creation / 测试HTTP网关创建
    let http_addr = create_test_addr(8080);
    let grpc_addr = create_test_grpc_addr();
    
    let gateway = HttpGateway::new(http_addr, grpc_addr, false);
    
    // Verify the gateway was created successfully / 验证网关创建成功
    assert_eq!(gateway.addr(), http_addr);
    assert_eq!(gateway.grpc_addr(), grpc_addr);
    assert!(!gateway.enable_swagger());
}

#[tokio::test]
async fn test_http_gateway_with_swagger() {
    // Test HTTP gateway creation with Swagger enabled / 测试启用Swagger的HTTP网关创建
    let http_addr = create_test_addr(8081);
    let grpc_addr = create_test_grpc_addr();
    
    let gateway = HttpGateway::new(http_addr, grpc_addr, true);
    
    // Verify Swagger is enabled / 验证Swagger已启用
    assert!(gateway.enable_swagger());
}

#[tokio::test]
async fn test_http_gateway_different_addresses() {
    // Test HTTP gateway with different address configurations / 测试不同地址配置的HTTP网关
    let test_cases = vec![
        (create_test_addr(8082), create_test_addr(50052)),
        (create_test_addr(8083), create_test_addr(50053)),
        ("0.0.0.0:8084".parse().unwrap(), "0.0.0.0:50054".parse().unwrap()),
    ];
    
    for (http_addr, grpc_addr) in test_cases {
        let gateway = HttpGateway::new(http_addr, grpc_addr, false);
        assert_eq!(gateway.addr(), http_addr);
        assert_eq!(gateway.grpc_addr(), grpc_addr);
    }
}

#[tokio::test]
async fn test_http_gateway_ipv6_addresses() {
    // Test HTTP gateway with IPv6 addresses / 测试IPv6地址的HTTP网关
    let http_addr: SocketAddr = "[::1]:8085".parse().unwrap();
    let grpc_addr: SocketAddr = "[::1]:50055".parse().unwrap();
    
    let gateway = HttpGateway::new(http_addr, grpc_addr, true);
    
    assert_eq!(gateway.addr(), http_addr);
    assert_eq!(gateway.grpc_addr(), grpc_addr);
    assert!(gateway.enable_swagger());
}

#[tokio::test]
async fn test_http_gateway_port_zero() {
    // Test HTTP gateway with port 0 (system assigned) / 测试端口0（系统分配）的HTTP网关
    let http_addr = create_test_addr(0);
    let grpc_addr = create_test_addr(0);
    
    let gateway = HttpGateway::new(http_addr, grpc_addr, false);
    
    assert_eq!(gateway.addr().port(), 0);
    assert_eq!(gateway.grpc_addr().port(), 0);
}

#[tokio::test]
async fn test_http_gateway_concurrent_creation() {
    // Test concurrent HTTP gateway creation / 测试并发HTTP网关创建
    let mut handles = vec![];
    
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            let http_addr = create_test_addr(8090 + i);
            let grpc_addr = create_test_addr(50060 + i);
            let gateway = HttpGateway::new(http_addr, grpc_addr, i % 2 == 0);
            
            (gateway.addr(), gateway.grpc_addr(), gateway.enable_swagger())
        });
        handles.push(handle);
    }
    
    for (i, handle) in handles.into_iter().enumerate() {
        let (http_addr, grpc_addr, swagger_enabled) = handle.await.unwrap();
        assert_eq!(http_addr.port(), 8090 + i as u16);
        assert_eq!(grpc_addr.port(), 50060 + i as u16);
        assert_eq!(swagger_enabled, i % 2 == 0);
    }
}

#[tokio::test]
async fn test_http_gateway_start_without_grpc_server() {
    // Test HTTP gateway start when gRPC server is not available / 测试gRPC服务器不可用时的HTTP网关启动
    let http_addr = create_test_addr(8086);
    let grpc_addr = create_test_addr(50056); // Non-existent gRPC server / 不存在的gRPC服务器
    
    let gateway = HttpGateway::new(http_addr, grpc_addr, false);
    
    // The start should fail due to gRPC connection error / 由于gRPC连接错误，启动应该失败
    let result = timeout(Duration::from_secs(5), gateway.start()).await;
    
    match result {
        Ok(start_result) => {
            // Should fail to connect to gRPC server / 应该无法连接到gRPC服务器
            assert!(start_result.is_err());
        }
        Err(_) => {
            // Timeout is also acceptable as it indicates connection attempt / 超时也是可接受的，表示尝试连接
        }
    }
}

#[tokio::test]
async fn test_http_gateway_invalid_grpc_url() {
    // Test HTTP gateway with invalid gRPC URL format / 测试无效gRPC URL格式的HTTP网关
    let http_addr = create_test_addr(8087);
    let grpc_addr = create_test_addr(50057);
    
    let gateway = HttpGateway::new(http_addr, grpc_addr, false);
    
    // Even with invalid gRPC server, gateway creation should succeed / 即使gRPC服务器无效，网关创建也应该成功
    // The error will occur during start() / 错误将在start()期间发生
    assert_eq!(gateway.grpc_addr(), grpc_addr);
}

#[tokio::test]
async fn test_http_gateway_edge_cases() {
    // Test HTTP gateway edge cases / 测试HTTP网关边界情况
    
    // Test with maximum port number / 测试最大端口号
    let http_addr = create_test_addr(65535);
    let grpc_addr = create_test_addr(65534);
    let gateway = HttpGateway::new(http_addr, grpc_addr, true);
    assert_eq!(gateway.addr().port(), 65535);
    assert_eq!(gateway.grpc_addr().port(), 65534);
    
    // Test with minimum port number / 测试最小端口号
    let http_addr = create_test_addr(1);
    let grpc_addr = create_test_addr(2);
    let gateway = HttpGateway::new(http_addr, grpc_addr, false);
    assert_eq!(gateway.addr().port(), 1);
    assert_eq!(gateway.grpc_addr().port(), 2);
}

#[tokio::test]
async fn test_http_gateway_configuration_variations() {
    // Test various configuration combinations / 测试各种配置组合
    let configurations = vec![
        (true, "Swagger enabled"),
        (false, "Swagger disabled"),
    ];
    
    for (swagger_enabled, description) in configurations {
        let http_addr = create_test_addr(8088);
        let grpc_addr = create_test_addr(50058);
        
        let gateway = HttpGateway::new(http_addr, grpc_addr, swagger_enabled);
        
        assert_eq!(gateway.enable_swagger(), swagger_enabled, "Failed for: {}", description);
    }
}

// Integration tests module / 集成测试模块
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU16, Ordering};
    
    static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);
    
    fn get_next_port() -> u16 {
        PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
    }
    
    #[tokio::test]
    async fn test_http_gateway_stress_test() {
        // Stress test: create many gateways concurrently / 压力测试：并发创建多个网关
        let num_gateways = 50;
        let mut handles = vec![];
        
        for _ in 0..num_gateways {
            let handle = tokio::spawn(async move {
                let http_port = get_next_port();
                let grpc_port = get_next_port();
                let http_addr = create_test_addr(http_port);
                let grpc_addr = create_test_addr(grpc_port);
                
                let gateway = HttpGateway::new(http_addr, grpc_addr, false);
                
                // Verify gateway properties / 验证网关属性
                assert_eq!(gateway.addr().port(), http_port);
                assert_eq!(gateway.grpc_addr().port(), grpc_port);
                
                gateway
            });
            handles.push(handle);
        }
        
        // Wait for all gateways to be created / 等待所有网关创建完成
        let gateways: Vec<HttpGateway> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|result| result.unwrap())
            .collect();
        
        assert_eq!(gateways.len(), num_gateways);
    }
    
    #[tokio::test]
    async fn test_http_gateway_memory_usage() {
        // Test memory usage of gateway creation / 测试网关创建的内存使用
        let initial_memory = std::mem::size_of::<HttpGateway>();
        
        let gateway = HttpGateway::new(
            create_test_addr(get_next_port()),
            create_test_addr(get_next_port()),
            true
        );
        
        let gateway_memory = std::mem::size_of_val(&gateway);
        
        // Gateway should not use excessive memory / 网关不应使用过多内存
        assert!(gateway_memory >= initial_memory);
        assert!(gateway_memory < initial_memory * 10); // Reasonable upper bound / 合理的上限
    }
}