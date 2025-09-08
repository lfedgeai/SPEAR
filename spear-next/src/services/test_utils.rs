//! Test utilities for SPEAR Metadata Server service / SPEAR元数据服务器服务测试工具
//!
//! This module provides common test utilities, data generators, and helper functions
//! for testing various components of the SPEAR Metadata Server service.
//!
//! 此模块为SPEAR元数据服务器服务的各种组件测试提供通用测试工具、数据生成器和辅助函数。

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::services::node::{NodeInfo, NodeStatus, NodeService};
use crate::services::resource::NodeResourceInfo;
use crate::services::config::SmsConfig;
use crate::storage::KvStoreConfig;

/// Test data generator for creating sample nodes / 创建示例节点的测试数据生成器
pub struct TestDataGenerator;

impl TestDataGenerator {
    /// Create a sample node with default values / 创建具有默认值的示例节点
    pub fn create_sample_node() -> NodeInfo {
        NodeInfo::new("127.0.0.1".to_string(), 8080)
    }

    /// Create a sample node with custom IP and port / 创建具有自定义IP和端口的示例节点
    pub fn create_node_with_address(ip: &str, port: u16) -> NodeInfo {
        NodeInfo::new(ip.to_string(), port)
    }

    /// Create multiple sample nodes / 创建多个示例节点
    pub fn create_sample_nodes(count: usize) -> Vec<NodeInfo> {
        (0..count)
            .map(|i| {
                NodeInfo::new(
                    format!("127.0.0.{}", i + 1),
                    8080 + i as u16,
                )
            })
            .collect()
    }

    /// Create a node with specific status / 创建具有特定状态的节点
    pub fn create_node_with_status(status: NodeStatus) -> NodeInfo {
        let mut node = Self::create_sample_node();
        node.status = status;
        node
    }

    /// Create an unhealthy node (old heartbeat) / 创建不健康的节点（旧心跳）
    pub fn create_unhealthy_node(age_seconds: i64) -> NodeInfo {
        let mut node = Self::create_sample_node();
        node.last_heartbeat = Utc::now() - chrono::Duration::seconds(age_seconds);
        node
    }

    /// Create a node with metadata / 创建带有元数据的节点
    pub fn create_node_with_metadata(metadata: HashMap<String, String>) -> NodeInfo {
        let mut node = Self::create_sample_node();
        node.metadata = metadata;
        node
    }

    /// Create sample resource info / 创建示例资源信息
    pub fn create_sample_resource(node_uuid: Uuid) -> NodeResourceInfo {
        NodeResourceInfo::new(node_uuid)
    }

    /// Create resource info with custom values / 创建具有自定义值的资源信息
    pub fn create_resource_with_usage(
        node_uuid: Uuid,
        cpu_percent: f64,
        memory_percent: f64,
        disk_percent: f64,
    ) -> NodeResourceInfo {
        let mut resource = NodeResourceInfo::new(node_uuid);
        resource.cpu_usage_percent = cpu_percent;
        resource.memory_usage_percent = memory_percent;
        resource.disk_usage_percent = disk_percent;
        resource
    }

    /// Create high-load resource info / 创建高负载资源信息
    pub fn create_high_load_resource(node_uuid: Uuid) -> NodeResourceInfo {
        Self::create_resource_with_usage(node_uuid, 85.0, 90.0, 95.0)
    }

    /// Create test configuration / 创建测试配置
    pub fn create_test_config() -> SmsConfig {
        SmsConfig {
            grpc_addr: "127.0.0.1:50051".parse().unwrap(),
            http_addr: "127.0.0.1:8080".parse().unwrap(),
            heartbeat_timeout: 60,
            cleanup_interval: 120,
            enable_swagger: true,
            kv_store: KvStoreConfig::memory(),
        }
    }

    /// Create test configuration with custom values / 创建具有自定义值的测试配置
    pub fn create_config_with_values(
        grpc_addr: &str,
        http_addr: &str,
        heartbeat_timeout: u64,
    ) -> SmsConfig {
        SmsConfig {
            grpc_addr: grpc_addr.parse().unwrap(),
            http_addr: http_addr.parse().unwrap(),
            heartbeat_timeout,
            cleanup_interval: 120,
            enable_swagger: true,
            kv_store: KvStoreConfig::memory(),
        }
    }
}

/// Test helper functions / 测试辅助函数
pub struct TestHelpers;

impl TestHelpers {
    /// Setup a test handler with sample nodes / 设置包含示例节点的测试处理器
    pub async fn setup_test_registry() -> NodeService {
    let mut service = NodeService::new();
        let nodes = TestDataGenerator::create_sample_nodes(3);
        
        for node in nodes {
            service.register_node(node).await.unwrap();
        }
        
        service
    }

    /// Setup a service with nodes and resources / 设置包含节点和资源的服务
    pub async fn setup_registry_with_resources() -> NodeService {
        let mut service = Self::setup_test_registry().await;
        let nodes = service.list_nodes().await.unwrap();
        let node_uuids: Vec<_> = nodes.iter().map(|n| n.uuid).collect();
        
        for &uuid in &node_uuids {
            let resource = TestDataGenerator::create_sample_resource(uuid);
            service.update_node_resource(resource).await.unwrap();
        }
        
        service
    }

    /// Create a service with mixed healthy and unhealthy nodes / 创建包含健康和不健康节点的服务
    pub async fn setup_mixed_health_registry() -> NodeService {
        let mut service = NodeService::new();
        
        // Add healthy nodes / 添加健康节点
        let healthy_nodes = TestDataGenerator::create_sample_nodes(2);
        for node in healthy_nodes {
            service.register_node(node).await.unwrap();
        }
        
        // Add unhealthy node / 添加不健康节点
        let unhealthy_node = TestDataGenerator::create_unhealthy_node(120);
        let unhealthy_uuid = unhealthy_node.uuid;
        service.register_node(unhealthy_node).await.unwrap();
        
        // Make the node unhealthy after registration / 注册后使节点不健康
        if let Some(mut node) = service.get_node(&unhealthy_uuid).await.unwrap() {
            node.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
            service.update_node(unhealthy_uuid, node).await.unwrap();
        }
        
        service
    }

    /// Assert that two timestamps are close (within tolerance) / 断言两个时间戳接近（在容差范围内）
    pub fn assert_timestamps_close(
        actual: DateTime<Utc>,
        expected: DateTime<Utc>,
        tolerance_seconds: i64,
    ) {
        let diff = (actual - expected).num_seconds().abs();
        assert!(
            diff <= tolerance_seconds,
            "Timestamps differ by {} seconds, expected within {} seconds",
            diff,
            tolerance_seconds
        );
    }

    /// Assert that a timestamp is recent (within last few seconds) / 断言时间戳是最近的（在最近几秒内）
    pub fn assert_timestamp_recent(timestamp: DateTime<Utc>, tolerance_seconds: i64) {
        Self::assert_timestamps_close(timestamp, Utc::now(), tolerance_seconds);
    }

    /// Create a temporary test file with content / 创建包含内容的临时测试文件
    pub fn create_temp_config_file(content: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        temp_file.write_all(content.as_bytes()).unwrap();
        temp_file.flush().unwrap();
        temp_file
    }

    /// Generate random UUID for testing / 生成用于测试的随机UUID
    pub fn generate_test_uuid() -> Uuid {
        Uuid::new_v4()
    }

    /// Create test metadata map / 创建测试元数据映射
    pub fn create_test_metadata() -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("environment".to_string(), "test".to_string());
        metadata.insert("version".to_string(), "1.0.0".to_string());
        metadata.insert("region".to_string(), "us-west-1".to_string());
        metadata
    }

    /// Create test health info map / 创建测试健康信息映射
    pub fn create_test_health_info() -> HashMap<String, String> {
        let mut health_info = HashMap::new();
        health_info.insert("status".to_string(), "healthy".to_string());
        health_info.insert("uptime".to_string(), "3600".to_string());
        health_info.insert("load".to_string(), "0.5".to_string());
        health_info
    }
}

/// Constants for testing / 测试常量
pub mod test_constants {
    /// Default test timeout in seconds / 默认测试超时时间（秒）
    pub const DEFAULT_TIMEOUT: u64 = 60;
    
    /// Test gRPC address / 测试gRPC地址
    pub const TEST_GRPC_ADDR: &str = "127.0.0.1:50051";
    
    /// Test HTTP address / 测试HTTP地址
    pub const TEST_HTTP_ADDR: &str = "127.0.0.1:8080";
    
    /// Test node IP addresses / 测试节点IP地址
    pub const TEST_NODE_IPS: &[&str] = &["127.0.0.1", "127.0.0.2", "127.0.0.3"];
    
    /// Test node ports / 测试节点端口
    pub const TEST_NODE_PORTS: &[u16] = &[8080, 8081, 8082];
    
    /// High load thresholds / 高负载阈值
    pub const HIGH_CPU_THRESHOLD: f64 = 80.0;
    pub const HIGH_MEMORY_THRESHOLD: f64 = 85.0;
    pub const HIGH_DISK_THRESHOLD: f64 = 90.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_generator_create_sample_node() {
        // Test creating a sample node / 测试创建示例节点
        let node = TestDataGenerator::create_sample_node();
        assert_eq!(node.ip_address, "127.0.0.1");
        assert_eq!(node.port, 8080);
        assert_eq!(node.status, NodeStatus::Active);
    }

    #[test]
    fn test_data_generator_create_multiple_nodes() {
        // Test creating multiple nodes / 测试创建多个节点
        let nodes = TestDataGenerator::create_sample_nodes(3);
        assert_eq!(nodes.len(), 3);
        
        for (i, node) in nodes.iter().enumerate() {
            assert_eq!(node.ip_address, format!("127.0.0.{}", i + 1));
            assert_eq!(node.port, 8080 + i as u16);
        }
    }

    #[test]
    fn test_data_generator_create_unhealthy_node() {
        // Test creating an unhealthy node / 测试创建不健康节点
        let node = TestDataGenerator::create_unhealthy_node(120);
        assert!(!node.is_healthy(60));
    }

    #[test]
    fn test_data_generator_create_high_load_resource() {
        // Test creating high load resource / 测试创建高负载资源
        let uuid = TestHelpers::generate_test_uuid();
        let resource = TestDataGenerator::create_high_load_resource(uuid);
        assert!(resource.is_high_load());
    }

    #[tokio::test]
    async fn test_helpers_setup_test_registry() {
        // Test setting up a test registry / 测试设置测试注册表
        let registry = TestHelpers::setup_test_registry().await;
        assert_eq!(registry.node_count().await.unwrap(), 3);
        assert!(!registry.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_helpers_setup_registry_with_resources() {
        // Test setting up registry with resources / 测试设置包含资源的注册表
        let registry = TestHelpers::setup_registry_with_resources().await;
        assert_eq!(registry.node_count().await.unwrap(), 3);
        assert_eq!(registry.resource_count().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_helpers_setup_mixed_health_registry() {
        // Test setting up mixed health registry / 测试设置混合健康状态注册表
        let registry = TestHelpers::setup_mixed_health_registry().await;
        assert_eq!(registry.node_count().await.unwrap(), 3);
        
        // Should have some healthy and some unhealthy nodes / 应该有一些健康和一些不健康的节点
        let all_nodes = registry.list_nodes().await.unwrap();
        let healthy_count = all_nodes.iter().filter(|n| n.is_healthy(60)).count();
        let unhealthy_count = all_nodes.len() - healthy_count;
        
        assert!(healthy_count > 0);
        assert!(unhealthy_count > 0);
    }

    #[test]
    fn test_helpers_timestamp_assertions() {
        // Test timestamp assertion helpers / 测试时间戳断言辅助函数
        let now = Utc::now();
        let recent = now - chrono::Duration::seconds(5);
        
        TestHelpers::assert_timestamps_close(now, recent, 10);
        TestHelpers::assert_timestamp_recent(now, 5);
    }

    #[test]
    fn test_create_test_metadata() {
        // Test creating test metadata / 测试创建测试元数据
        let metadata = TestHelpers::create_test_metadata();
        assert!(metadata.contains_key("environment"));
        assert!(metadata.contains_key("version"));
        assert!(metadata.contains_key("region"));
    }

    #[test]
    fn test_create_test_health_info() {
        // Test creating test health info / 测试创建测试健康信息
        let health_info = TestHelpers::create_test_health_info();
        assert!(health_info.contains_key("status"));
        assert!(health_info.contains_key("uptime"));
        assert!(health_info.contains_key("load"));
    }
}