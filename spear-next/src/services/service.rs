//! SPEAR Metadata Server gRPC service implementation
//! SPEAR元数据服务器gRPC服务实现

use crate::proto::sms::{
    node_service_server::NodeService as ProtoNodeService,
    *,
};
use crate::services::{
    node::{NodeService, NodeInfo, NodeStatus},
    resource::NodeResourceInfo,
};
use super::{SmsError, SmsResult};
use crate::storage::{KvStoreConfig, create_kv_store_from_config};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{info, warn};
use uuid::Uuid;
use chrono::Utc;

/// SPEAR Metadata Server service implementation / SPEAR元数据服务器服务实现
#[derive(Debug, Clone)]
pub struct SmsServiceImpl {
    /// Node service for managing cluster nodes / 用于管理集群节点的节点服务
    node_service: Arc<RwLock<NodeService>>,
    /// Heartbeat timeout in seconds / 心跳超时时间（秒）
    heartbeat_timeout: u64,
}

impl SmsServiceImpl {
    /// Create a new SPEAR Metadata Server service instance / 创建新的SPEAR元数据服务器服务实例
    pub async fn new(heartbeat_timeout: u64) -> Self {
        Self::with_kv_config(heartbeat_timeout, KvStoreConfig::memory()).await
    }
    
    /// Create a new SPEAR Metadata Server service instance with KV store configuration / 使用KV存储配置创建新的SPEAR元数据服务器服务实例
    pub async fn with_kv_config(heartbeat_timeout: u64, kv_config: KvStoreConfig) -> Self {
        // Create a KV store instance
        let kv_store = create_kv_store_from_config(&kv_config)
            .await
            .unwrap_or_else(|e| panic!("Failed to create KV store from config (type: {:?}): {}", kv_config, e));
        
        // Create NodeService with the KV store (it will create its own ResourceService internally)
        let node_service = NodeService::new_with_kv_store(kv_store);
        
        Self {
            node_service: Arc::new(RwLock::new(node_service)),
            heartbeat_timeout,
        }
    }
    
    /// Get the node service / 获取节点服务
    pub fn node_service(&self) -> Arc<RwLock<NodeService>> {
        self.node_service.clone()
    }
    
    /// Get the heartbeat timeout in seconds / 获取心跳超时时间（秒）
    pub fn heartbeat_timeout(&self) -> u64 {
        self.heartbeat_timeout
    }

    
    /// Convert protobuf Node to internal NodeInfo / 将protobuf Node转换为内部NodeInfo
    fn proto_to_node_info(&self, proto_node: Node) -> SmsResult<NodeInfo> {
        let uuid = Uuid::parse_str(&proto_node.uuid)
            .map_err(|e| SmsError::InvalidNodeData { 
                message: format!("Invalid UUID: {}", e) 
            })?;
        
        let status = proto_node.status.parse::<NodeStatus>()
            .map_err(|e| SmsError::InvalidNodeData { 
                message: format!("Invalid status: {}", e) 
            })?;
        
        let port = proto_node.port as u16;
        
        let mut node = NodeInfo {
            uuid,
            ip_address: proto_node.ip_address,
            port,
            status,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
            metadata: proto_node.metadata,
            health_info: HashMap::new(),
        };
        
        // Update last heartbeat from proto if provided / 如果提供则从proto更新最后心跳时间
        if proto_node.last_heartbeat > 0 {
            if let Some(dt) = chrono::DateTime::from_timestamp(proto_node.last_heartbeat, 0) {
                node.last_heartbeat = dt;
            }
        }
        
        Ok(node)
    }
    
    /// Convert internal NodeInfo to protobuf Node / 将内部NodeInfo转换为protobuf Node
    fn node_info_to_proto(&self, node: &NodeInfo) -> Node {
        Node {
            uuid: node.uuid.to_string(),
            ip_address: node.ip_address.clone(),
            port: node.port as i32,
            status: node.status.to_string(),
            last_heartbeat: node.last_heartbeat.timestamp(),
            registered_at: node.registered_at.timestamp(),
            metadata: node.metadata.clone(),
        }
    }

    /// Convert protobuf NodeResource to internal NodeResourceInfo / 将protobuf NodeResource转换为内部NodeResourceInfo
    fn proto_to_resource_info(&self, proto_resource: NodeResource) -> SmsResult<NodeResourceInfo> {
        let node_uuid = Uuid::parse_str(&proto_resource.node_uuid)
            .map_err(|e| SmsError::InvalidNodeData { message: format!("Invalid UUID: {}", e) })?;

        let mut resource = NodeResourceInfo::new(node_uuid);
        resource.cpu_usage_percent = proto_resource.cpu_usage_percent;
        resource.memory_usage_percent = proto_resource.memory_usage_percent;
        resource.total_memory_bytes = proto_resource.total_memory_bytes;
        resource.used_memory_bytes = proto_resource.used_memory_bytes;
        resource.available_memory_bytes = proto_resource.available_memory_bytes;
        resource.disk_usage_percent = proto_resource.disk_usage_percent;
        resource.total_disk_bytes = proto_resource.total_disk_bytes;
        resource.used_disk_bytes = proto_resource.used_disk_bytes;
        resource.network_rx_bytes_per_sec = proto_resource.network_rx_bytes_per_sec;
        resource.network_tx_bytes_per_sec = proto_resource.network_tx_bytes_per_sec;
        resource.load_average_1m = proto_resource.load_average_1m;
        resource.load_average_5m = proto_resource.load_average_5m;
        resource.load_average_15m = proto_resource.load_average_15m;
        resource.resource_metadata = proto_resource.resource_metadata;

        if proto_resource.updated_at > 0 {
            if let Some(dt) = chrono::DateTime::from_timestamp(proto_resource.updated_at, 0) {
                resource.updated_at = dt;
            }
        }

        Ok(resource)
    }

    /// Convert internal NodeResourceInfo to protobuf NodeResource / 将内部NodeResourceInfo转换为protobuf NodeResource
    fn resource_info_to_proto(&self, resource: &NodeResourceInfo) -> NodeResource {
        NodeResource {
            node_uuid: resource.node_uuid.to_string(),
            cpu_usage_percent: resource.cpu_usage_percent,
            memory_usage_percent: resource.memory_usage_percent,
            total_memory_bytes: resource.total_memory_bytes,
            used_memory_bytes: resource.used_memory_bytes,
            available_memory_bytes: resource.available_memory_bytes,
            disk_usage_percent: resource.disk_usage_percent,
            total_disk_bytes: resource.total_disk_bytes,
            used_disk_bytes: resource.used_disk_bytes,
            network_rx_bytes_per_sec: resource.network_rx_bytes_per_sec,
            network_tx_bytes_per_sec: resource.network_tx_bytes_per_sec,
            load_average_1m: resource.load_average_1m,
            load_average_5m: resource.load_average_5m,
            load_average_15m: resource.load_average_15m,
            updated_at: resource.updated_at.timestamp(),
            resource_metadata: resource.resource_metadata.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Helper function to create a test SPEAR Metadata Server service / 创建测试SPEAR元数据服务器服务的辅助函数
    async fn create_test_service() -> SmsServiceImpl {
        SmsServiceImpl::new(60).await // 60 seconds heartbeat timeout
    }

    /// Helper function to create a test node / 创建测试节点的辅助函数
    fn create_test_node(ip: &str, port: i32) -> Node {
        Node {
            uuid: Uuid::new_v4().to_string(),
            ip_address: ip.to_string(),
            port,
            status: "active".to_string(),
            last_heartbeat: Utc::now().timestamp(),
            registered_at: Utc::now().timestamp(),
            metadata: HashMap::new(),
        }
    }

    /// Helper function to create a test resource / 创建测试资源的辅助函数
    fn create_test_resource(node_uuid: &str) -> NodeResource {
        NodeResource {
            node_uuid: node_uuid.to_string(),
            cpu_usage_percent: 50.0,
            memory_usage_percent: 60.0,
            total_memory_bytes: 16 * 1024 * 1024 * 1024, // 16GB
            used_memory_bytes: 9 * 1024 * 1024 * 1024,   // 9GB
            available_memory_bytes: 7 * 1024 * 1024 * 1024, // 7GB
            disk_usage_percent: 70.0,
            total_disk_bytes: 1024 * 1024 * 1024 * 1024, // 1TB
            used_disk_bytes: 700 * 1024 * 1024 * 1024,   // 700GB
            network_rx_bytes_per_sec: 1000000,
            network_tx_bytes_per_sec: 500000,
            load_average_1m: 1.5,
            load_average_5m: 1.2,
            load_average_15m: 1.0,
            updated_at: Utc::now().timestamp(),
            resource_metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_service_creation() {
        // Test SPEAR Metadata Server service creation / 测试SPEAR元数据服务器服务创建
        let service = create_test_service().await;
        
        // Verify initial state / 验证初始状态
        let registry_arc = service.node_service();
        let registry = registry_arc.read().await;
        assert!(registry.is_empty().await.unwrap());
        assert_eq!(registry.node_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_register_node_success() {
        // Test successful node registration / 测试成功的节点注册
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        
        let request = Request::new(RegisterNodeRequest {
            node: Some(test_node.clone()),
        });
        
        let response = service.register_node(request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.success);
        assert_eq!(response_inner.message, "Node registered successfully");
        assert!(!response_inner.node_uuid.is_empty());
        
        // Verify node was added to registry / 验证节点已添加到注册表
        let registry_arc = service.node_service();
        let registry = registry_arc.read().await;
        assert_eq!(registry.node_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_register_node_missing_node() {
        // Test node registration with missing node data / 测试缺少节点数据的节点注册
        let service = create_test_service().await;
        
        let request = Request::new(RegisterNodeRequest {
            node: None,
        });
        
        let result = service.register_node(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_register_node_invalid_uuid() {
        // Test node registration with invalid UUID / 测试使用无效UUID的节点注册
        let service = create_test_service().await;
        let mut test_node = create_test_node("127.0.0.1", 8080);
        test_node.uuid = "invalid-uuid".to_string();
        
        let request = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        
        let result = service.register_node(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_register_duplicate_node() {
        // Test registering duplicate node / 测试注册重复节点
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        
        // Register first time / 第一次注册
        let request1 = Request::new(RegisterNodeRequest {
            node: Some(test_node.clone()),
        });
        let response1 = service.register_node(request1).await.unwrap();
        assert!(response1.into_inner().success);
        
        // Try to register again with same UUID / 尝试使用相同UUID再次注册
        let request2 = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        let response2 = service.register_node(request2).await.unwrap();
        let response2_inner = response2.into_inner();
        
        assert!(!response2_inner.success);
        assert!(response2_inner.message.contains("already exists"));
    }

    #[tokio::test]
    async fn test_update_node_success() {
        // Test successful node update / 测试成功的节点更新
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node first / 先注册节点
        let register_request = Request::new(RegisterNodeRequest {
            node: Some(test_node.clone()),
        });
        service.register_node(register_request).await.unwrap();
        
        // Update node / 更新节点
        let mut updated_node = test_node;
        updated_node.status = "inactive".to_string();
        
        let update_request = Request::new(UpdateNodeRequest {
            uuid: node_uuid.clone(),
            node: Some(updated_node),
        });
        
        let response = service.update_node(update_request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.success);
        assert_eq!(response_inner.message, "Node updated successfully");
    }

    #[tokio::test]
    async fn test_update_nonexistent_node() {
        // Test updating non-existent node / 测试更新不存在的节点
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let non_existent_uuid = Uuid::new_v4().to_string();
        
        let request = Request::new(UpdateNodeRequest {
            uuid: non_existent_uuid,
            node: Some(test_node),
        });
        
        let result = service.update_node(request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code(), tonic::Code::NotFound);
        assert!(error.message().contains("not found"));
    }

    #[tokio::test]
    async fn test_delete_node_success() {
        // Test successful node deletion / 测试成功的节点删除
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node first / 先注册节点
        let register_request = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        service.register_node(register_request).await.unwrap();
        
        // Delete node / 删除节点
        let delete_request = Request::new(DeleteNodeRequest {
            uuid: node_uuid,
        });
        
        let response = service.delete_node(delete_request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.success);
        assert_eq!(response_inner.message, "Node deleted successfully");
        
        // Verify node was removed / 验证节点已被移除
        let registry_arc = service.node_service();
        let registry = registry_arc.read().await;
        assert_eq!(registry.node_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_node() {
        // Test deleting non-existent node / 测试删除不存在的节点
        let service = create_test_service().await;
        let non_existent_uuid = Uuid::new_v4().to_string();
        
        let request = Request::new(DeleteNodeRequest {
            uuid: non_existent_uuid,
        });
        
        let result = service.delete_node(request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code(), tonic::Code::NotFound);
        assert!(error.message().contains("not found"));
    }

    #[tokio::test]
    async fn test_heartbeat_success() {
        // Test successful heartbeat / 测试成功的心跳
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node first / 先注册节点
        let register_request = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        service.register_node(register_request).await.unwrap();
        
        // Send heartbeat / 发送心跳
        let mut health_info = HashMap::new();
        health_info.insert("status".to_string(), "healthy".to_string());
        
        let heartbeat_request = Request::new(HeartbeatRequest {
            uuid: node_uuid,
            timestamp: Utc::now().timestamp(),
            health_info,
        });
        
        let response = service.heartbeat(heartbeat_request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.success);
        assert_eq!(response_inner.message, "Heartbeat received");
        assert!(response_inner.server_timestamp > 0);
    }

    #[tokio::test]
    async fn test_heartbeat_nonexistent_node() {
        // Test heartbeat for non-existent node / 测试不存在节点的心跳
        let service = create_test_service().await;
        let non_existent_uuid = Uuid::new_v4().to_string();
        
        let request = Request::new(HeartbeatRequest {
            uuid: non_existent_uuid,
            timestamp: Utc::now().timestamp(),
            health_info: HashMap::new(),
        });
        
        let result = service.heartbeat(request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code(), tonic::Code::NotFound);
        assert!(error.message().contains("not found"));
    }

    #[tokio::test]
    async fn test_list_nodes_empty() {
        // Test listing nodes when registry is empty / 测试注册表为空时列出节点
        let service = create_test_service().await;
        
        let request = Request::new(ListNodesRequest {
            status_filter: String::new(),
        });
        
        let response = service.list_nodes(request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.nodes.is_empty());
    }

    #[tokio::test]
    async fn test_list_nodes_with_data() {
        // Test listing nodes with data / 测试有数据时列出节点
        let service = create_test_service().await;
        
        // Register multiple nodes / 注册多个节点
        let node1 = create_test_node("127.0.0.1", 8080);
        let node2 = create_test_node("127.0.0.2", 8081);
        
        let request1 = Request::new(RegisterNodeRequest {
            node: Some(node1),
        });
        let request2 = Request::new(RegisterNodeRequest {
            node: Some(node2),
        });
        
        service.register_node(request1).await.unwrap();
        service.register_node(request2).await.unwrap();
        
        // List all nodes / 列出所有节点
        let list_request = Request::new(ListNodesRequest {
            status_filter: String::new(),
        });
        
        let response = service.list_nodes(list_request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert_eq!(response_inner.nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_list_nodes_with_status_filter() {
        // Test listing nodes with status filter / 测试使用状态过滤器列出节点
        let service = create_test_service().await;
        
        // Register nodes with different statuses / 注册不同状态的节点
        let mut node1 = create_test_node("127.0.0.1", 8080);
        let mut node2 = create_test_node("127.0.0.2", 8081);
        
        node1.status = "active".to_string();
        node2.status = "inactive".to_string();
        
        let request1 = Request::new(RegisterNodeRequest {
            node: Some(node1),
        });
        let request2 = Request::new(RegisterNodeRequest {
            node: Some(node2),
        });
        
        service.register_node(request1).await.unwrap();
        service.register_node(request2).await.unwrap();
        
        // List only active nodes / 只列出活跃节点
        let list_request = Request::new(ListNodesRequest {
            status_filter: "active".to_string(),
        });
        
        let response = service.list_nodes(list_request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert_eq!(response_inner.nodes.len(), 1);
        assert_eq!(response_inner.nodes[0].status, "active");
    }

    #[tokio::test]
    async fn test_get_node_success() {
        // Test successful node retrieval / 测试成功的节点检索
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node first / 先注册节点
        let register_request = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        service.register_node(register_request).await.unwrap();
        
        // Get node / 获取节点
        let get_request = Request::new(GetNodeRequest {
            uuid: node_uuid.clone(),
        });
        
        let response = service.get_node(get_request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.found);
        assert!(response_inner.node.is_some());
        assert_eq!(response_inner.node.unwrap().uuid, node_uuid);
    }

    #[tokio::test]
    async fn test_get_nonexistent_node() {
        // Test getting non-existent node / 测试获取不存在的节点
        let service = create_test_service().await;
        let non_existent_uuid = Uuid::new_v4().to_string();
        
        let request = Request::new(GetNodeRequest {
            uuid: non_existent_uuid,
        });
        
        let result = service.get_node(request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code(), tonic::Code::NotFound);
        assert!(error.message().contains("not found"));
    }

    #[tokio::test]
    async fn test_update_node_resource_success() {
        // Test successful node resource update / 测试成功的节点资源更新
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node first / 先注册节点
        let register_request = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        service.register_node(register_request).await.unwrap();
        
        // Update resource / 更新资源
        let test_resource = create_test_resource(&node_uuid);
        let update_request = Request::new(UpdateNodeResourceRequest {
            resource: Some(test_resource),
        });
        
        let response = service.update_node_resource(update_request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.success);
        assert_eq!(response_inner.message, "Resource updated successfully");
    }

    #[tokio::test]
    async fn test_update_resource_nonexistent_node() {
        // Test updating resource for non-existent node / 测试为不存在的节点更新资源
        let service = create_test_service().await;
        let non_existent_uuid = Uuid::new_v4().to_string();
        
        let test_resource = create_test_resource(&non_existent_uuid);
        let request = Request::new(UpdateNodeResourceRequest {
            resource: Some(test_resource),
        });
        
        let response = service.update_node_resource(request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(!response_inner.success);
        assert!(response_inner.message.contains("Failed to update resource"));
    }

    #[tokio::test]
    async fn test_get_node_resource_success() {
        // Test successful node resource retrieval / 测试成功的节点资源检索
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node and update resource / 注册节点并更新资源
        let register_request = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        service.register_node(register_request).await.unwrap();
        
        let test_resource = create_test_resource(&node_uuid);
        let update_request = Request::new(UpdateNodeResourceRequest {
            resource: Some(test_resource),
        });
        service.update_node_resource(update_request).await.unwrap();
        
        // Get resource / 获取资源
        let get_request = Request::new(GetNodeResourceRequest {
            node_uuid: node_uuid.clone(),
        });
        
        let response = service.get_node_resource(get_request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.found);
        assert!(response_inner.resource.is_some());
        assert_eq!(response_inner.resource.unwrap().node_uuid, node_uuid);
    }

    #[tokio::test]
    async fn test_get_resource_nonexistent_node() {
        // Test getting resource for non-existent node / 测试获取不存在节点的资源
        let service = create_test_service().await;
        let non_existent_uuid = Uuid::new_v4().to_string();
        
        let request = Request::new(GetNodeResourceRequest {
            node_uuid: non_existent_uuid,
        });
        
        let response = service.get_node_resource(request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(!response_inner.found);
        assert!(response_inner.resource.is_none());
    }

    #[tokio::test]
    async fn test_list_node_resources() {
        // Test listing node resources / 测试列出节点资源
        let service = create_test_service().await;
        
        // Register nodes and add resources / 注册节点并添加资源
        let node1 = create_test_node("127.0.0.1", 8080);
        let node2 = create_test_node("127.0.0.2", 8081);
        let uuid1 = node1.uuid.clone();
        let uuid2 = node2.uuid.clone();
        
        // Register nodes / 注册节点
        service.register_node(Request::new(RegisterNodeRequest {
            node: Some(node1),
        })).await.unwrap();
        service.register_node(Request::new(RegisterNodeRequest {
            node: Some(node2),
        })).await.unwrap();
        
        // Add resources / 添加资源
        service.update_node_resource(Request::new(UpdateNodeResourceRequest {
            resource: Some(create_test_resource(&uuid1)),
        })).await.unwrap();
        service.update_node_resource(Request::new(UpdateNodeResourceRequest {
            resource: Some(create_test_resource(&uuid2)),
        })).await.unwrap();
        
        // List resources / 列出资源
        let request = Request::new(ListNodeResourcesRequest {
            node_uuids: vec![], // Empty list to get all resources / 空列表获取所有资源
        });
        let response = service.list_node_resources(request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert_eq!(response_inner.resources.len(), 2);
    }

    #[tokio::test]
    async fn test_get_node_with_resource_success() {
        // Test getting node with resource / 测试获取节点及其资源
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node and add resource / 注册节点并添加资源
        service.register_node(Request::new(RegisterNodeRequest {
            node: Some(test_node),
        })).await.unwrap();
        service.update_node_resource(Request::new(UpdateNodeResourceRequest {
            resource: Some(create_test_resource(&node_uuid)),
        })).await.unwrap();
        
        // Get node with resource / 获取节点及其资源
        let request = Request::new(GetNodeWithResourceRequest {
            uuid: node_uuid.clone(),
        });
        
        let response = service.get_node_with_resource(request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.found);
        assert!(response_inner.node.is_some());
        assert!(response_inner.resource.is_some());
        assert_eq!(response_inner.node.unwrap().uuid, node_uuid);
        assert_eq!(response_inner.resource.unwrap().node_uuid, node_uuid);
    }

    #[tokio::test]
    async fn test_get_node_with_resource_no_resource() {
        // Test getting node without resource / 测试获取没有资源的节点
        let service = create_test_service().await;
        let test_node = create_test_node("127.0.0.1", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node only (no resource) / 只注册节点（没有资源）
        service.register_node(Request::new(RegisterNodeRequest {
            node: Some(test_node),
        })).await.unwrap();
        
        // Get node with resource / 获取节点及其资源
        let request = Request::new(GetNodeWithResourceRequest {
            uuid: node_uuid.clone(),
        });
        
        let response = service.get_node_with_resource(request).await.unwrap();
        let response_inner = response.into_inner();
        
        assert!(response_inner.found);
        assert!(response_inner.node.is_some());
        assert!(response_inner.resource.is_none());
        assert_eq!(response_inner.node.unwrap().uuid, node_uuid);
    }

    #[tokio::test]
    async fn test_proto_conversion_node() {
        // Test protobuf to NodeInfo conversion / 测试protobuf到NodeInfo的转换
        let service = create_test_service().await;
        let proto_node = create_test_node("192.168.1.100", 9090);
        
        let node_info = service.proto_to_node_info(proto_node.clone()).unwrap();
        
        assert_eq!(node_info.ip_address, proto_node.ip_address);
        assert_eq!(node_info.port, proto_node.port as u16);
        assert_eq!(node_info.status.to_string(), proto_node.status);
        
        // Test conversion back to proto / 测试转换回proto
        let converted_proto = service.node_info_to_proto(&node_info);
        assert_eq!(converted_proto.ip_address, proto_node.ip_address);
        assert_eq!(converted_proto.port, proto_node.port);
        assert_eq!(converted_proto.status, proto_node.status);
    }

    #[tokio::test]
    async fn test_proto_conversion_resource() {
        // Test protobuf to NodeResourceInfo conversion / 测试protobuf到NodeResourceInfo的转换
        let service = create_test_service().await;
        let node_uuid = Uuid::new_v4().to_string();
        let proto_resource = create_test_resource(&node_uuid);
        
        let resource_info = service.proto_to_resource_info(proto_resource.clone()).unwrap();
        
        assert_eq!(resource_info.node_uuid.to_string(), proto_resource.node_uuid);
        assert_eq!(resource_info.cpu_usage_percent, proto_resource.cpu_usage_percent);
        assert_eq!(resource_info.memory_usage_percent, proto_resource.memory_usage_percent);
        
        // Test conversion back to proto / 测试转换回proto
        let converted_proto = service.resource_info_to_proto(&resource_info);
        assert_eq!(converted_proto.node_uuid, proto_resource.node_uuid);
        assert_eq!(converted_proto.cpu_usage_percent, proto_resource.cpu_usage_percent);
        assert_eq!(converted_proto.memory_usage_percent, proto_resource.memory_usage_percent);
    }

    #[tokio::test]
    async fn test_invalid_status_conversion() {
        // Test invalid status conversion / 测试无效状态转换
        let service = create_test_service().await;
        let mut proto_node = create_test_node("127.0.0.1", 8080);
        proto_node.status = "invalid_status".to_string();
        
        let result = service.proto_to_node_info(proto_node);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sms_service_with_memory_kv_store() {
        // Test SMS service with memory KV store / 测试使用内存KV存储的SMS服务
        let kv_config = KvStoreConfig::memory();
        let service = SmsServiceImpl::with_kv_config(60, kv_config).await;
        
        // Test basic operations / 测试基本操作
        let test_node = create_test_node("192.168.1.100", 8080);
        let node_uuid = test_node.uuid.clone();
        
        // Register node / 注册节点
        let register_request = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        
        let response = service.register_node(register_request).await;
        assert!(response.is_ok());
        
        // Verify node exists / 验证节点存在
        let get_request = Request::new(GetNodeRequest {
            uuid: node_uuid.clone(),
        });
        
        let get_response = service.get_node(get_request).await;
        assert!(get_response.is_ok());
        let get_result = get_response.unwrap().into_inner();
        assert!(get_result.found);
        assert_eq!(get_result.node.unwrap().uuid, node_uuid);
    }

    #[cfg(feature = "sled")]
    #[tokio::test]
    async fn test_sms_service_with_sled_kv_store() {
        use tempfile::TempDir;
        use uuid::Uuid;
        
        // Test SMS service with Sled KV store / 测试使用Sled KV存储的SMS服务
        let temp_dir = TempDir::new().unwrap();
        let unique_id = Uuid::new_v4();
        let db_path = temp_dir.path().join(format!("test_sms_sled_{}.db", unique_id));
        
        let kv_config = KvStoreConfig::sled(db_path.to_string_lossy().to_string());
        let service = SmsServiceImpl::with_kv_config(60, kv_config).await;
        
        // Test basic operations / 测试基本操作
        let test_node = create_test_node("192.168.1.101", 8081);
        let node_uuid = test_node.uuid.clone();
        
        // Register node / 注册节点
        let register_request = Request::new(RegisterNodeRequest {
            node: Some(test_node),
        });
        
        let response = service.register_node(register_request).await;
        assert!(response.is_ok());
        
        // Verify node exists / 验证节点存在
        let get_request = Request::new(GetNodeRequest {
            uuid: node_uuid.clone(),
        });
        
        let get_response = service.get_node(get_request).await;
        assert!(get_response.is_ok());
        let get_result = get_response.unwrap().into_inner();
        assert!(get_result.found);
        assert_eq!(get_result.node.unwrap().uuid, node_uuid);
        
        // Test persistence by creating a new service instance with same config / 通过使用相同配置创建新服务实例来测试持久性
        // Drop the first service to ensure database is properly closed / 释放第一个服务以确保数据库正确关闭
        drop(service);
        
        // Small delay to ensure database is fully released / 短暂延迟以确保数据库完全释放
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let kv_config2 = KvStoreConfig::sled(db_path.to_string_lossy().to_string());
        let service2 = SmsServiceImpl::with_kv_config(60, kv_config2).await;
        
        // Verify node still exists after restart / 验证重启后节点仍然存在
        let get_request2 = Request::new(GetNodeRequest {
            uuid: node_uuid.clone(),
        });
        
        let get_response2 = service2.get_node(get_request2).await;
        assert!(get_response2.is_ok());
        let get_result2 = get_response2.unwrap().into_inner();
        assert!(get_result2.found);
        assert_eq!(get_result2.node.unwrap().uuid, node_uuid);
    }

    #[tokio::test]
    async fn test_sms_service_kv_config_validation() {
        // Test KV store configuration validation / 测试KV存储配置验证
        
        // Test memory config / 测试内存配置
        let memory_config = KvStoreConfig::memory();
        let memory_service = SmsServiceImpl::with_kv_config(60, memory_config).await;
        assert!(memory_service.node_service().read().await.is_empty().await.unwrap());
        
        // Test sled config with valid path / 测试有效路径的Sled配置
        #[cfg(feature = "sled")]
        {
            use tempfile::TempDir;
            let temp_dir = TempDir::new().unwrap();
            let valid_path = temp_dir.path().join("valid_test.db");
            
            let sled_config = KvStoreConfig::sled(valid_path.to_string_lossy().to_string());
            let sled_service = SmsServiceImpl::with_kv_config(60, sled_config).await;
            assert!(sled_service.node_service().read().await.is_empty().await.unwrap());
        }
    }
}

#[tonic::async_trait]
impl ProtoNodeService for SmsServiceImpl {
    /// Register a new node / 注册新节点
    async fn register_node(
        &self,
        request: Request<RegisterNodeRequest>,
    ) -> Result<Response<RegisterNodeResponse>, Status> {
        let req = request.into_inner();
        
        info!("Registering new node");
        
        let proto_node = req.node.ok_or_else(|| {
            Status::invalid_argument("Node information is required")
        })?;
        
        let node_info = self.proto_to_node_info(proto_node)
            .map_err(|e| Status::from(e))?;
        
        let mut node_service = self.node_service.write().await;

        match node_service.register_node(node_info).await {
            Ok(uuid) => {
                info!("Successfully registered node with UUID: {}", uuid);
                Ok(Response::new(RegisterNodeResponse {
                    success: true,
                    message: "Node registered successfully".to_string(),
                    node_uuid: uuid.to_string(),
                }))
            }
            Err(e) => {
                warn!("Failed to register node: {}", e);
                Ok(Response::new(RegisterNodeResponse {
                    success: false,
                    message: e.to_string(),
                    node_uuid: String::new(),
                }))
            }
        }
    }
    
    /// Update node / 更新节点
    async fn update_node(
        &self,
        request: Request<UpdateNodeRequest>,
    ) -> Result<Response<UpdateNodeResponse>, Status> {
        let inner = request.into_inner();
        
        // Parse UUID from request
        let uuid = Uuid::parse_str(&inner.uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        let proto_node = inner.node.ok_or_else(|| {
            Status::invalid_argument("Node is required")
        })?;
        
        let node_info = self.proto_to_node_info(proto_node)
            .map_err(|e| Status::from(e))?;
        
        let mut node_service = self.node_service.write().await;

        match node_service.update_node(uuid, node_info).await {
            Ok(_) => {
                info!("Successfully updated node");
                Ok(Response::new(UpdateNodeResponse {
                    success: true,
                    message: "Node updated successfully".to_string(),
                }))
            }
            Err(e) => {
                warn!("Failed to update node: {}", e);
                // Convert SmsError to gRPC Status for error cases / 将SmsError转换为gRPC状态用于错误情况
                Err(Status::from(e))
            }
        }
    }

    /// Delete node / 删除节点
    async fn delete_node(
        &self,
        request: Request<DeleteNodeRequest>,
    ) -> Result<Response<DeleteNodeResponse>, Status> {
        let inner = request.into_inner();
        
        let uuid = Uuid::parse_str(&inner.uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        let mut node_service = self.node_service.write().await;

        match node_service.remove_node(&uuid).await {
            Ok(_) => {
                info!("Successfully deleted node with UUID: {}", uuid);
                Ok(Response::new(DeleteNodeResponse {
                    success: true,
                    message: "Node deleted successfully".to_string(),
                }))
            }
            Err(e) => {
                warn!("Failed to delete node: {}", e);
                // Convert SmsError to gRPC Status for error cases / 将SmsError转换为gRPC状态用于错误情况
                Err(Status::from(e))
            }
        }
    }

    /// Send heartbeat / 发送心跳
    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let inner = request.into_inner();
        
        let uuid = Uuid::parse_str(&inner.uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        let mut node_service = self.node_service.write().await;

        match node_service.update_heartbeat(&uuid, None).await {
            Ok(_) => {
                info!("Heartbeat received from node: {}", uuid);
                Ok(Response::new(HeartbeatResponse {
                    success: true,
                    message: "Heartbeat received".to_string(),
                    server_timestamp: Utc::now().timestamp(),
                }))
            }
            Err(e) => {
                warn!("Failed to process heartbeat: {}", e);
                // Convert SmsError to gRPC Status for error cases / 将SmsError转换为gRPC状态用于错误情况
                Err(Status::from(e))
            }
        }
    }

    /// List nodes / 列出节点
    async fn list_nodes(
        &self,
        request: Request<ListNodesRequest>,
    ) -> Result<Response<ListNodesResponse>, Status> {
        let inner = request.into_inner();
        
        let node_service = self.node_service.read().await;
        
        let status_filter = if !inner.status_filter.is_empty() {
            match inner.status_filter.as_str() {
                "active" => Some(NodeStatus::Active),
                "inactive" => Some(NodeStatus::Inactive),
                "unhealthy" => Some(NodeStatus::Unhealthy),
                "decommissioning" => Some(NodeStatus::Decommissioning),
                _ => return Err(Status::invalid_argument("Invalid status filter")),
            }
        } else {
            None
        };
        
        match node_service.list_nodes().await {
            Ok(nodes) => {
                let filtered_nodes: Vec<&NodeInfo> = if let Some(filter_status) = status_filter {
                    nodes.iter().filter(|node| node.status == filter_status).collect()
                } else {
                    nodes.iter().collect()
                };
                
                let proto_nodes: Vec<Node> = filtered_nodes.iter()
                    .map(|node| self.node_info_to_proto(node))
                    .collect();
                
                Ok(Response::new(ListNodesResponse {
                    nodes: proto_nodes,
                }))
            }
            Err(e) => {
                warn!("Failed to list nodes: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }

    /// Get node / 获取节点
    async fn get_node(
        &self,
        request: Request<GetNodeRequest>,
    ) -> Result<Response<GetNodeResponse>, Status> {
        let inner = request.into_inner();
        
        let uuid = Uuid::parse_str(&inner.uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        let node_service = self.node_service.read().await;

        match node_service.get_node(&uuid).await {
            Ok(Some(node)) => {
                let proto_node = self.node_info_to_proto(&node);
                Ok(Response::new(GetNodeResponse {
                    node: Some(proto_node),
                    found: true,
                }))
            }
            Ok(None) => {
                Err(Status::not_found("Node not found"))
            }
            Err(e) => {
                warn!("Failed to get node: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }

    /// Update node resource information / 更新节点资源信息
    async fn update_node_resource(
        &self,
        request: Request<UpdateNodeResourceRequest>,
    ) -> Result<Response<UpdateNodeResourceResponse>, Status> {
        let req = request.into_inner();

        let resource = req.resource.ok_or_else(|| {
            Status::invalid_argument("Resource information is required")
        })?;

        info!("Updating node resource for UUID: {}", resource.node_uuid);

        let resource_info = self.proto_to_resource_info(resource.clone())
            .map_err(|e| Status::invalid_argument(format!("Invalid resource data: {}", e)))?;

        match self.node_service.write().await.update_node_resource(resource_info).await {
            Ok(_) => {
                info!("Successfully updated resource for node: {}", resource.node_uuid);
                Ok(Response::new(UpdateNodeResourceResponse {
                    success: true,
                    message: "Resource updated successfully".to_string(),
                }))
            }
            Err(e) => {
                warn!("Failed to update resource for node {}: {}", resource.node_uuid, e);
                Ok(Response::new(UpdateNodeResourceResponse {
                    success: false,
                    message: format!("Failed to update resource: {}", e),
                }))
            }
        }
    }

    /// Get node resource information / 获取节点资源信息
    async fn get_node_resource(
        &self,
        request: Request<GetNodeResourceRequest>,
    ) -> Result<Response<GetNodeResourceResponse>, Status> {
        let req = request.into_inner();
        info!("Getting resource for node UUID: {}", req.node_uuid);
        
        let uuid = Uuid::parse_str(&req.node_uuid)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {}", e)))?;

        let node_service = self.node_service.read().await;

        match node_service.get_node_resource(&uuid).await {
            Ok(Some(resource)) => {
                info!("Found resource for node: {}", uuid);
                Ok(Response::new(GetNodeResourceResponse {
                    found: true,
                    resource: Some(self.resource_info_to_proto(&resource)),
                }))
            }
            Ok(None) => {
                warn!("Resource not found for node: {}", uuid);
                Ok(Response::new(GetNodeResourceResponse {
                    found: false,
                    resource: None,
                }))
            }
            Err(e) => {
                warn!("Failed to get resource for node {}: {}", uuid, e);
                Err(Status::internal("Failed to get node resource"))
            }
        }
    }

    /// List all node resources / 列出所有节点资源
    async fn list_node_resources(
        &self,
        request: Request<ListNodeResourcesRequest>,
    ) -> Result<Response<ListNodeResourcesResponse>, Status> {
        let inner = request.into_inner();
        info!("Listing node resources with {} filters", inner.node_uuids.len());

        let node_service = self.node_service.read().await;
        
        let resources = if inner.node_uuids.is_empty() {
            // List all resources if no filter is provided / 如果没有提供过滤器则列出所有资源
            node_service.list_node_resources().await.map_err(|e| {
                warn!("Failed to list node resources: {}", e);
                Status::internal("Failed to list node resources")
            })?
        } else {
            // Filter by specific node UUIDs / 按特定节点UUID过滤
            let node_uuids: Result<Vec<Uuid>, _> = inner.node_uuids
                .iter()
                .map(|uuid_str| Uuid::parse_str(uuid_str))
                .collect();
            
            let node_uuids = node_uuids.map_err(|e| {
                warn!("Invalid UUID in node_uuids filter: {}", e);
                Status::invalid_argument("Invalid UUID format")
            })?;
            
            node_service.list_resources_by_nodes(&node_uuids).await.map_err(|e| {
                warn!("Failed to list resources by nodes: {}", e);
                Status::internal("Failed to list resources by nodes")
            })?
        };
        
        let proto_resources: Vec<NodeResource> = resources
            .iter()
            .map(|resource| self.resource_info_to_proto(resource))
            .collect();

        info!("Found {} node resources", proto_resources.len());
        
        Ok(Response::new(ListNodeResourcesResponse {
            resources: proto_resources,
        }))
    }

    /// Get node with its resource information / 获取节点及其资源信息
    async fn get_node_with_resource(
        &self,
        request: Request<GetNodeWithResourceRequest>,
    ) -> Result<Response<GetNodeWithResourceResponse>, Status> {
        let req = request.into_inner();
        info!("Getting node with resource for UUID: {}", req.uuid);

        let uuid = Uuid::parse_str(&req.uuid)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {}", e)))?;

        let node_service = self.node_service.read().await;

        match node_service.get_node_with_resource(&uuid).await {
            Ok(Some((node_info, resource_info))) => {
                if let Some(resource) = resource_info {
                    info!("Found node and resource for UUID: {}", uuid);
                    Ok(Response::new(GetNodeWithResourceResponse {
                        found: true,
                        node: Some(self.node_info_to_proto(&node_info)),
                        resource: Some(self.resource_info_to_proto(&resource)),
                    }))
                } else {
                    info!("Found node but no resource for UUID: {}", uuid);
                    Ok(Response::new(GetNodeWithResourceResponse {
                        found: true,
                        node: Some(self.node_info_to_proto(&node_info)),
                        resource: None,
                    }))
                }
            }
            Ok(None) => {
                warn!("Node not found for UUID: {}", uuid);
                Ok(Response::new(GetNodeWithResourceResponse {
                    found: false,
                    node: None,
                    resource: None,
                }))
            }
            Err(e) => {
                warn!("Error retrieving node for UUID {}: {}", uuid, e);
                Err(Status::internal(format!("Failed to retrieve node: {}", e)))
            }
        }
    }
}