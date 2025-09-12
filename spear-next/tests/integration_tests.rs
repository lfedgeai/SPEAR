//! Integration tests for SMS service
//! SMS服务的集成测试
//!
//! These tests verify the end-to-end functionality of both gRPC and HTTP services
//! 这些测试验证gRPC和HTTP服务的端到端功能

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use spear_next::sms::config::SmsConfig;


use spear_next::proto::sms::{
    node_service_client::NodeServiceClient,
    node_service_server::NodeServiceServer,
    *,
};

// Test utilities / 测试工具
mod test_utils {
    use super::*;
    use std::net::SocketAddr;
    use tonic::transport::Server;
    use tokio::net::TcpListener;
    
    /// Create a test gRPC server / 创建测试gRPC服务器
    pub async fn create_test_grpc_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        let mut storage_config = spear_next::config::base::StorageConfig::default();
        storage_config.backend = "memory".to_string();
        let service = spear_next::sms::service::SmsServiceImpl::with_storage_config(&storage_config).await;
        
        let handle = tokio::spawn(async move {
            Server::builder()
                .add_service(NodeServiceServer::new(service))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        
        // Wait for server to start / 等待服务器启动
        sleep(Duration::from_millis(100)).await;
        
        (addr, handle)
    }
    
    /// Create a test gRPC client / 创建测试gRPC客户端
    pub async fn create_test_grpc_client(addr: SocketAddr) -> NodeServiceClient<tonic::transport::Channel> {
        let endpoint = format!("http://{}", addr);
        NodeServiceClient::connect(endpoint).await.unwrap()
    }
    
    /// Generate test node data / 生成测试节点数据
    pub fn generate_test_node() -> (String, String, i32, HashMap<String, String>) {
        let uuid = Uuid::new_v4().to_string();
        let ip = "192.168.1.100".to_string();
        let port = 8080;
        let mut metadata = HashMap::new();
        metadata.insert("region".to_string(), "us-west".to_string());
        metadata.insert("env".to_string(), "test".to_string());
        
        (uuid, ip, port, metadata)
    }
    
    /// Generate test resource data / 生成测试资源数据
    pub fn generate_test_resource(node_uuid: String) -> NodeResource {
        NodeResource {
            node_uuid,
            cpu_usage_percent: 75.5,
            memory_usage_percent: 82.3,
            total_memory_bytes: 16_000_000_000,
            used_memory_bytes: 13_168_000_000,
            available_memory_bytes: 2_832_000_000,
            disk_usage_percent: 45.2,
            total_disk_bytes: 1_000_000_000_000,
            used_disk_bytes: 452_000_000_000,
            network_rx_bytes_per_sec: 1_048_576,
            network_tx_bytes_per_sec: 524_288,
            load_average_1m: 1.25,
            load_average_5m: 1.15,
            load_average_15m: 1.05,
            resource_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("gpu_count".to_string(), "2".to_string());
                metadata
            },
            updated_at: chrono::Utc::now().timestamp(),
        }
    }
}

#[tokio::test]
async fn test_grpc_node_lifecycle() {
    // Test complete node lifecycle via gRPC / 通过gRPC测试完整的节点生命周期
    let (addr, _handle) = test_utils::create_test_grpc_server().await;
    let mut client = test_utils::create_test_grpc_client(addr).await;
    
    let (uuid, ip, port, metadata) = test_utils::generate_test_node();
    
    // 1. Register node / 注册节点
    let register_req = RegisterNodeRequest {
        node: Some(Node {
            uuid: uuid.clone(),
            ip_address: ip.clone(),
            port,
            status: "online".to_string(),
            metadata: metadata.clone(),
            registered_at: chrono::Utc::now().timestamp(),
            last_heartbeat: chrono::Utc::now().timestamp(),
        }),
    };
    
    let register_resp = client.register_node(register_req).await.unwrap();
    assert!(register_resp.into_inner().success);
    
    // 2. Get node / 获取节点
    let get_req = GetNodeRequest { uuid: uuid.clone() };
    let get_resp = client.get_node(get_req).await.unwrap();
    let node = get_resp.into_inner().node.unwrap();
    assert_eq!(node.uuid, uuid);
    assert_eq!(node.ip_address, ip);
    assert_eq!(node.port, port);
    
    // 3. Update node / 更新节点
    let mut updated_metadata = metadata.clone();
    updated_metadata.insert("updated".to_string(), "true".to_string());
    
    let update_req = UpdateNodeRequest {
        uuid: uuid.clone(),
        node: Some(Node {
             uuid: uuid.clone(),
             ip_address: "192.168.1.101".to_string(),
             port: 8081,
             status: "active".to_string(),
             metadata: updated_metadata.clone(),
             registered_at: chrono::Utc::now().timestamp(),
             last_heartbeat: chrono::Utc::now().timestamp(),
         }),
    };
    
    let update_resp = client.update_node(update_req).await.unwrap();
    assert!(update_resp.into_inner().success);
    
    // 4. Verify update / 验证更新
    let get_req = GetNodeRequest { uuid: uuid.clone() };
    let get_resp = client.get_node(get_req).await.unwrap();
    let updated_node = get_resp.into_inner().node.unwrap();
    assert_eq!(updated_node.ip_address, "192.168.1.101");
    assert_eq!(updated_node.port, 8081);
    assert_eq!(updated_node.metadata.get("updated"), Some(&"true".to_string()));
    
    // 5. Heartbeat / 心跳
    let mut health_info = HashMap::new();
    health_info.insert("cpu_usage".to_string(), "45.2".to_string());
    
    let heartbeat_req = HeartbeatRequest {
        uuid: uuid.clone(),
        timestamp: chrono::Utc::now().timestamp(),
        health_info: health_info,
    };
    
    let heartbeat_resp = client.heartbeat(heartbeat_req).await.unwrap();
    assert!(heartbeat_resp.into_inner().success);
    
    // 6. List nodes / 列出节点
    let list_req = ListNodesRequest {
        status_filter: String::new(),
    };
    
    let list_resp = client.list_nodes(list_req).await.unwrap();
    let nodes = list_resp.into_inner().nodes;
    assert!(!nodes.is_empty());
    assert!(nodes.iter().any(|n| n.uuid == uuid));
    
    // 7. Delete node / 删除节点
    let delete_req = DeleteNodeRequest { uuid: uuid.clone() };
    let delete_resp = client.delete_node(delete_req).await.unwrap();
    assert!(delete_resp.into_inner().success);
    
    // 8. Verify deletion / 验证删除
    let get_req = GetNodeRequest { uuid: uuid.clone() };
    let get_result = client.get_node(get_req).await;
    assert!(get_result.is_err());
}

#[tokio::test]
async fn test_grpc_resource_management() {
    // Test resource management via gRPC / 通过gRPC测试资源管理
    let (addr, _handle) = test_utils::create_test_grpc_server().await;
    let mut client = test_utils::create_test_grpc_client(addr).await;
    
    let (uuid, ip, port, metadata) = test_utils::generate_test_node();
    
    // 1. Register node first / 首先注册节点
    let register_req = RegisterNodeRequest {
        node: Some(Node {
            uuid: uuid.clone(),
            ip_address: ip,
            port,
            status: "active".to_string(),
            metadata,
            registered_at: chrono::Utc::now().timestamp(),
            last_heartbeat: chrono::Utc::now().timestamp(),
        }),
    };
    
    let register_resp = client.register_node(register_req).await.unwrap();
    assert!(register_resp.into_inner().success);
    
    // 2. Update node resource / 更新节点资源
    let resource = test_utils::generate_test_resource(uuid.clone());
    let update_resource_req = UpdateNodeResourceRequest {
        resource: Some(resource.clone()),
    };
    
    let update_resource_resp = client.update_node_resource(update_resource_req).await.unwrap();
    assert!(update_resource_resp.into_inner().success);
    
    // 3. Get node resource / 获取节点资源
    let get_resource_req = GetNodeResourceRequest {
        node_uuid: uuid.clone(),
    };
    
    let get_resource_resp = client.get_node_resource(get_resource_req).await.unwrap();
    let retrieved_resource = get_resource_resp.into_inner().resource.unwrap();
    assert_eq!(retrieved_resource.node_uuid, uuid);
    assert_eq!(retrieved_resource.cpu_usage_percent, 75.5);
    assert_eq!(retrieved_resource.memory_usage_percent, 82.3);
    
    // 4. List node resources / 列出节点资源
    let list_resources_req = ListNodeResourcesRequest {
        node_uuids: vec![uuid.clone()],
    };
    
    let list_resources_resp = client.list_node_resources(list_resources_req).await.unwrap();
    let resources = list_resources_resp.into_inner().resources;
    assert!(!resources.is_empty());
    assert!(resources.iter().any(|r| r.node_uuid == uuid));
    
    // 5. Get node with resource / 获取节点及其资源
    let get_with_resource_req = GetNodeWithResourceRequest {
        uuid: uuid.clone(),
    };
    
    let get_with_resource_resp = client.get_node_with_resource(get_with_resource_req).await.unwrap();
    let resp = get_with_resource_resp.into_inner();
    assert!(resp.node.is_some());
    assert!(resp.resource.is_some());
    
    let node = resp.node.unwrap();
    let resource = resp.resource.unwrap();
    assert_eq!(node.uuid, uuid);
    assert_eq!(resource.node_uuid, uuid);
}

#[tokio::test]
async fn test_grpc_error_handling() {
    // Test gRPC error handling / 测试gRPC错误处理
    let (addr, _handle) = test_utils::create_test_grpc_server().await;
    let mut client = test_utils::create_test_grpc_client(addr).await;
    
    // 1. Get non-existent node / 获取不存在的节点
    let non_existent_uuid = Uuid::new_v4().to_string();
    let get_req = GetNodeRequest { uuid: non_existent_uuid.clone() };
    let get_result = client.get_node(get_req).await;
    assert!(get_result.is_err());
    
    let status = get_result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound);
    
    // 2. Update non-existent node / 更新不存在的节点
    let update_req = UpdateNodeRequest {
        uuid: non_existent_uuid.clone(),
        node: Some(Node {
             uuid: non_existent_uuid.clone(),
             ip_address: "192.168.1.100".to_string(),
             port: 8080,
             status: "active".to_string(),
             metadata: HashMap::new(),
             registered_at: chrono::Utc::now().timestamp(),
             last_heartbeat: chrono::Utc::now().timestamp(),
         }),
    };
    
    let update_result = client.update_node(update_req).await;
    assert!(update_result.is_err());
    
    // 3. Delete non-existent node / 删除不存在的节点
    let delete_req = DeleteNodeRequest { uuid: non_existent_uuid.clone() };
    let delete_result = client.delete_node(delete_req).await;
    assert!(delete_result.is_err());
    
    // 4. Heartbeat for non-existent node / 对不存在的节点进行心跳
    let heartbeat_req = HeartbeatRequest {
        uuid: non_existent_uuid,
        timestamp: chrono::Utc::now().timestamp(),
        health_info: HashMap::new(),
    };
    
    let heartbeat_result = client.heartbeat(heartbeat_req).await;
    assert!(heartbeat_result.is_err());
}

#[tokio::test]
async fn test_grpc_concurrent_operations() {
    // Test concurrent gRPC operations / 测试并发gRPC操作
    let (addr, _handle) = test_utils::create_test_grpc_server().await;
    
    // Create multiple clients for concurrent operations / 为并发操作创建多个客户端
    let mut clients = Vec::new();
    for _ in 0..5 {
        clients.push(test_utils::create_test_grpc_client(addr).await);
    }
    
    // Register multiple nodes concurrently / 并发注册多个节点
    let mut handles = Vec::new();
    for (i, mut client) in clients.into_iter().enumerate() {
        let handle = tokio::spawn(async move {
            let uuid = Uuid::new_v4().to_string();
            let register_req = RegisterNodeRequest {
                node: Some(Node {
                    uuid: uuid.clone(),
                    ip_address: format!("192.168.1.{}", 100 + i),
                    port: 8080 + i as i32,
                    status: "online".to_string(),
                    metadata: HashMap::new(),
                    registered_at: chrono::Utc::now().timestamp(),
                    last_heartbeat: chrono::Utc::now().timestamp(),
                }),
            };
            
            let result = client.register_node(register_req).await;
            (uuid, result)
        });
        handles.push(handle);
    }
    
    // Wait for all registrations to complete / 等待所有注册完成
    let mut registered_uuids = Vec::new();
    for handle in handles {
        let (uuid, result) = handle.await.unwrap();
        assert!(result.is_ok());
        assert!(result.unwrap().into_inner().success);
        registered_uuids.push(uuid);
    }
    
    // Verify all nodes were registered / 验证所有节点都已注册
    let mut client = test_utils::create_test_grpc_client(addr).await;
    let list_req = ListNodesRequest {
        status_filter: String::new(),
    };
    
    let list_resp = client.list_nodes(list_req).await.unwrap();
    let nodes = list_resp.into_inner().nodes;
    assert_eq!(nodes.len(), 5);
    
    for uuid in &registered_uuids {
        assert!(nodes.iter().any(|n| n.uuid == *uuid));
    }
}

#[tokio::test]
async fn test_node_registry_integration() {
    // Test NodeService integration via gRPC / 通过gRPC测试NodeService集成
    let (addr, _handle) = test_utils::create_test_grpc_server().await;
    let mut client = test_utils::create_test_grpc_client(addr).await;
    
    // Test multiple node operations / 测试多个节点操作
    let mut node_uuids = Vec::new();
    
    // Register multiple nodes / 注册多个节点
    for i in 0..3 {
        let (uuid, ip, port, mut metadata) = test_utils::generate_test_node();
        metadata.insert("index".to_string(), i.to_string());
        
        let register_req = RegisterNodeRequest {
            node: Some(Node {
                uuid: uuid.clone(),
                ip_address: ip,
                port,
                status: "active".to_string(),
                metadata,
                registered_at: chrono::Utc::now().timestamp(),
                last_heartbeat: chrono::Utc::now().timestamp(),
            }),
        };
        
        let register_resp = client.register_node(register_req).await.unwrap();
        assert!(register_resp.into_inner().success);
        node_uuids.push(uuid);
    }
    
    // Verify all nodes are registered / 验证所有节点都已注册
    let list_req = ListNodesRequest {
        status_filter: String::new(),
    };
    let list_resp = client.list_nodes(list_req).await.unwrap();
    let all_nodes = list_resp.into_inner().nodes;
    assert_eq!(all_nodes.len(), 3);
    
    // Test heartbeat for all nodes / 对所有节点进行心跳测试
    for uuid in &node_uuids {
        let mut health_info = HashMap::new();
        health_info.insert("status".to_string(), "ok".to_string());
        
        let heartbeat_req = HeartbeatRequest {
            uuid: uuid.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            health_info: health_info,
        };
        
        let result = client.heartbeat(heartbeat_req).await;
        assert!(result.is_ok());
    }
    
    // Test resource management / 测试资源管理
    for (i, uuid) in node_uuids.iter().enumerate() {
        let resource = test_utils::generate_test_resource(uuid.clone());
        let update_resource_req = UpdateNodeResourceRequest {
            resource: Some(resource),
        };
        
        let update_resource_resp = client.update_node_resource(update_resource_req).await.unwrap();
        assert!(update_resource_resp.into_inner().success);
    }
    
    // Verify resource updates / 验证资源更新
    let list_resources_req = ListNodeResourcesRequest {
        node_uuids: vec![], // Empty means list all / 空表示列出所有
    };
    let list_resources_resp = client.list_node_resources(list_resources_req).await.unwrap();
    let all_resources = list_resources_resp.into_inner().resources;
    assert_eq!(all_resources.len(), 3);
    
    // Verify each resource exists / 验证每个资源存在
    for resource in all_resources.iter() {
        assert!(node_uuids.contains(&resource.node_uuid));
        assert!(resource.cpu_usage_percent > 0.0);
    }
}

#[tokio::test]
async fn test_configuration_integration() {
    // Test configuration integration / 测试配置集成
    
    // Test default configuration / 测试默认配置
    let default_config = SmsConfig::default();
    assert_eq!(default_config.grpc.addr.port(), 50051);
    assert_eq!(default_config.http.addr.port(), 8080);
    assert_eq!(default_config.enable_swagger, true);
    assert_eq!(default_config.database.db_type, "sled");
}