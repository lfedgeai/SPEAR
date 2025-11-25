//! SMS Service Implementation / SMS服务实现
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

use uuid::Uuid;

use crate::sms::services::{
    node_service::NodeService, 
    task_service::TaskService as TaskServiceImpl,
    resource_service::ResourceService,
};
use crate::sms::config::SmsConfig;


// Import proto types / 导入proto类型
use crate::proto::sms::{
    node_service_server::NodeService as NodeServiceTrait,
    task_service_server::TaskService as TaskServiceTrait,
    // Node service messages / 节点服务消息
    RegisterNodeRequest, RegisterNodeResponse,
    UpdateNodeRequest, UpdateNodeResponse,
    DeleteNodeRequest, DeleteNodeResponse,
    HeartbeatRequest, HeartbeatResponse,
    ListNodesRequest, ListNodesResponse,
    GetNodeRequest, GetNodeResponse,
    UpdateNodeResourceRequest, UpdateNodeResourceResponse,
    GetNodeResourceRequest, GetNodeResourceResponse,
    ListNodeResourcesRequest, ListNodeResourcesResponse,
    GetNodeWithResourceRequest, GetNodeWithResourceResponse,
    // Task service messages / 任务服务消息
    RegisterTaskRequest, RegisterTaskResponse,
    ListTasksRequest, ListTasksResponse,
    GetTaskRequest, GetTaskResponse,
    UnregisterTaskRequest, UnregisterTaskResponse,
};

// Note: SpearletRegistrationService is not defined in current proto files
// use crate::proto::spearlet::{
//     spearlet_registration_service_server::SpearletRegistrationService,
//     RegisterSpearletRequest, RegisterSpearletResponse,
//     SpearletHeartbeatRequest, SpearletHeartbeatResponse,
//     UnregisterSpearletRequest, UnregisterSpearletResponse,
// };

#[derive(Debug, Clone)]
pub struct SmsServiceImpl {
    node_service: Arc<RwLock<NodeService>>,
    resource_service: Arc<ResourceService>,
    #[allow(dead_code)]
    config: Arc<SmsConfig>,
    task_service: Arc<RwLock<TaskServiceImpl>>,
}

impl SmsServiceImpl {
    /// Create new SMS service implementation / 创建新的SMS服务实现
    pub async fn new(
        node_service: Arc<RwLock<NodeService>>,
        resource_service: Arc<ResourceService>,
        config: Arc<SmsConfig>,
    ) -> Self {
        // Create task service / 创建任务服务
        let task_service = Arc::new(RwLock::new(TaskServiceImpl::new()));

        Self {
            node_service,
            resource_service,
            config,
            task_service,
        }
    }

    /// Convert proto Node to internal NodeInfo / 将proto Node转换为内部NodeInfo
    #[allow(dead_code)]
    fn proto_node_to_node_info(proto_node: crate::proto::sms::Node) -> crate::sms::services::node_service::NodeInfo {
        use crate::sms::services::node_service::NodeInfo;

        let uuid = proto_node.uuid.clone();
        NodeInfo {
            uuid: uuid.clone(),
            name: format!("node-{}", uuid),
            address: format!("{}:{}", proto_node.ip_address, proto_node.port),
            port: proto_node.port as u16,
            capabilities: vec!["storage".to_string(), "compute".to_string()],
        }
    }

    /// Convert internal NodeInfo to proto Node / 将内部NodeInfo转换为proto Node
    #[allow(dead_code)]
    fn node_info_to_proto_node(node_info: &crate::sms::services::node_service::NodeInfo) -> crate::proto::sms::Node {
        // Parse IP and port from address
        let (ip, port) = if let Some(colon_pos) = node_info.address.rfind(':') {
            let ip = node_info.address[..colon_pos].to_string();
            let port = node_info.address[colon_pos + 1..].parse::<i32>().unwrap_or(8080);
            (ip, port)
        } else {
            (node_info.address.clone(), 8080)
        };

        crate::proto::sms::Node {
            uuid: node_info.uuid.clone(),
            ip_address: ip,
            port,
            status: "online".to_string(),
            last_heartbeat: chrono::Utc::now().timestamp(),
            registered_at: chrono::Utc::now().timestamp(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Convert proto NodeResource to internal NodeResourceInfo / 将proto NodeResource转换为内部NodeResourceInfo
    fn proto_resource_to_resource_info(proto_resource: &crate::proto::sms::NodeResource) -> Result<crate::sms::services::resource_service::NodeResourceInfo, Status> {
        let node_uuid = uuid::Uuid::parse_str(&proto_resource.node_uuid)
            .map_err(|_| Status::invalid_argument("Invalid node UUID format"))?;

        let updated_at = if proto_resource.updated_at > 0 {
            chrono::DateTime::from_timestamp(proto_resource.updated_at, 0)
                .unwrap_or_else(|| chrono::Utc::now())
        } else {
            chrono::Utc::now()
        };

        Ok(crate::sms::services::resource_service::NodeResourceInfo {
            node_uuid,
            cpu_usage_percent: proto_resource.cpu_usage_percent,
            memory_usage_percent: proto_resource.memory_usage_percent,
            total_memory_bytes: proto_resource.total_memory_bytes,
            used_memory_bytes: proto_resource.used_memory_bytes,
            available_memory_bytes: proto_resource.available_memory_bytes,
            disk_usage_percent: proto_resource.disk_usage_percent,
            total_disk_bytes: proto_resource.total_disk_bytes,
            used_disk_bytes: proto_resource.used_disk_bytes,
            network_rx_bytes_per_sec: proto_resource.network_rx_bytes_per_sec,
            network_tx_bytes_per_sec: proto_resource.network_tx_bytes_per_sec,
            load_average_1m: proto_resource.load_average_1m,
            load_average_5m: proto_resource.load_average_5m,
            load_average_15m: proto_resource.load_average_15m,
            updated_at,
            resource_metadata: proto_resource.resource_metadata.clone(),
        })
    }

    /// Convert internal NodeResourceInfo to proto NodeResource / 将内部NodeResourceInfo转换为proto NodeResource
    fn resource_info_to_proto_resource(resource_info: &crate::sms::services::resource_service::NodeResourceInfo) -> crate::proto::sms::NodeResource {
        crate::proto::sms::NodeResource {
            node_uuid: resource_info.node_uuid.to_string(),
            cpu_usage_percent: resource_info.cpu_usage_percent,
            memory_usage_percent: resource_info.memory_usage_percent,
            total_memory_bytes: resource_info.total_memory_bytes,
            used_memory_bytes: resource_info.used_memory_bytes,
            available_memory_bytes: resource_info.available_memory_bytes,
            disk_usage_percent: resource_info.disk_usage_percent,
            total_disk_bytes: resource_info.total_disk_bytes,
            used_disk_bytes: resource_info.used_disk_bytes,
            network_rx_bytes_per_sec: resource_info.network_rx_bytes_per_sec,
            network_tx_bytes_per_sec: resource_info.network_tx_bytes_per_sec,
            load_average_1m: resource_info.load_average_1m,
            load_average_5m: resource_info.load_average_5m,
            load_average_15m: resource_info.load_average_15m,
            updated_at: resource_info.updated_at.timestamp(),
            resource_metadata: resource_info.resource_metadata.clone(),
        }
    }

    /// Create SMS service with storage configuration / 使用存储配置创建SMS服务
    pub async fn with_storage_config(
        storage_config: &crate::config::base::StorageConfig,
    ) -> Self {
        // Create KV store from storage config / 从存储配置创建KV存储
        use crate::storage::{KvStoreConfig, create_kv_store_from_config};
        let kv_config = KvStoreConfig::from_storage_config(storage_config);
        let _kv_store = create_kv_store_from_config(&kv_config).await
            .expect("Failed to create KV store from storage config");
        
        let node_service = Arc::new(RwLock::new(
            NodeService::new()
        ));
        let resource_service = Arc::new(ResourceService::new());
        let config = Arc::new(SmsConfig::default());

        Self::new(node_service, resource_service, config).await
    }

    /// Get node service reference / 获取节点服务引用
    pub fn node_service(&self) -> Arc<RwLock<NodeService>> {
        self.node_service.clone()
    }

    /// Get resource service reference / 获取资源服务引用
    pub fn resource_service(&self) -> Arc<ResourceService> {
        self.resource_service.clone()
    }

    /// Get task service reference / 获取任务服务引用
    pub fn task_service(&self) -> Arc<RwLock<TaskServiceImpl>> {
        self.task_service.clone()
    }
}

// Implement NodeService trait / 实现NodeService trait
#[tonic::async_trait]
impl NodeServiceTrait for SmsServiceImpl {
    /// Register a new node / 注册新节点
    async fn register_node(
        &self,
        request: Request<RegisterNodeRequest>,
    ) -> Result<Response<RegisterNodeResponse>, Status> {
        let req = request.into_inner();
        let node = req.node.ok_or_else(|| Status::invalid_argument("Node is required"))?;
        
        // Register node directly / 直接注册节点
        let mut node_service = self.node_service.write().await;
        
        match node_service.register_node(node.clone()).await {
            Ok(()) => {
                tracing::info!(uuid = %node.uuid, ip = %node.ip_address, port = %node.port, "SPEARlet registered");
                let response = RegisterNodeResponse {
                    node_uuid: node.uuid.clone(),
                    success: true,
                    message: "Node registered successfully".to_string(),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                let response = RegisterNodeResponse {
                    success: false,
                    message: format!("Failed to register node: {}", e),
                    node_uuid: String::new(),
                };
                Ok(Response::new(response))
            }
        }
    }

    /// Update an existing node / 更新现有节点
    async fn update_node(
        &self,
        request: Request<UpdateNodeRequest>,
    ) -> Result<Response<UpdateNodeResponse>, Status> {
        let req = request.into_inner();
        let node = req.node.ok_or_else(|| Status::invalid_argument("Node is required"))?;
        
        // Use update_node to update the existing node / 使用update_node来更新现有节点
        let mut node_service = self.node_service.write().await;
        
        match node_service.update_node(node).await {
            Ok(_) => {
                let response = UpdateNodeResponse {
                    success: true,
                    message: "Node updated successfully".to_string(),
                };
                Ok(Response::new(response))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a node / 删除节点
    async fn delete_node(
        &self,
        request: Request<DeleteNodeRequest>,
    ) -> Result<Response<DeleteNodeResponse>, Status> {
        let req = request.into_inner();
        let node_uuid = Uuid::parse_str(&req.uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        let mut node_service = self.node_service.write().await;
        
        match node_service.remove_node(&node_uuid.to_string()).await {
            Ok(_) => {
                tracing::info!(uuid = %node_uuid, "SPEARlet unregistered");
                let response = DeleteNodeResponse {
                    success: true,
                    message: "Node deleted successfully".to_string(),
                };
                Ok(Response::new(response))
            }
            Err(e) => Err(e.into()), // Use SmsError to tonic::Status conversion
        }
    }

    /// Send heartbeat / 发送心跳
    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();
        let node_uuid = Uuid::parse_str(&req.uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        let mut node_service = self.node_service.write().await;
        
        match node_service.update_heartbeat(&node_uuid.to_string(), chrono::Utc::now().timestamp()).await {
            Ok(_) => {
                tracing::debug!(uuid = %node_uuid, "Heartbeat received");
                let response = HeartbeatResponse {
                    success: true,
                    message: "Heartbeat received".to_string(),
                    server_timestamp: chrono::Utc::now().timestamp(),
                };
                Ok(Response::new(response))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// List all nodes / 列出所有节点
    async fn list_nodes(
        &self,
        request: Request<ListNodesRequest>,
    ) -> Result<Response<ListNodesResponse>, Status> {
        let _req = request.into_inner();
        let node_service = self.node_service.read().await;
        
        let nodes = node_service.list_nodes().await;
        
        match nodes {
            Ok(node_list) => {
                let response = ListNodesResponse {
                    nodes: node_list,
                };
                Ok(Response::new(response))
            }
            Err(e) => Err(Status::internal(format!("List nodes failed: {}", e))),
        }
    }

    /// Get specific node / 获取特定节点
    async fn get_node(
        &self,
        request: Request<GetNodeRequest>,
    ) -> Result<Response<GetNodeResponse>, Status> {
        let req = request.into_inner();
        let node_uuid = uuid::Uuid::parse_str(&req.uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        let node_service = self.node_service.read().await;
        
        match node_service.get_node(&node_uuid.to_string()).await {
            Ok(Some(node)) => {
                let response = GetNodeResponse {
                    found: true,
                    node: Some(node),
                };
                Ok(Response::new(response))
            }
            Ok(None) => {
                Err(Status::not_found("Node not found"))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Update node resource information / 更新节点资源信息
    async fn update_node_resource(
        &self,
        request: Request<UpdateNodeResourceRequest>,
    ) -> Result<Response<UpdateNodeResourceResponse>, Status> {
        let req = request.into_inner();
        
        let resource = req.resource
            .ok_or_else(|| Status::invalid_argument("Resource is required"))?;
        
        let resource_info = Self::proto_resource_to_resource_info(&resource)?;
        
        match self.resource_service.update_resource(resource_info).await {
            Ok(_) => {
                let response = UpdateNodeResourceResponse {
                    success: true,
                    message: "Resource updated successfully".to_string(),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                let response = UpdateNodeResourceResponse {
                    success: false,
                    message: format!("Update failed: {}", e),
                };
                Ok(Response::new(response))
            }
        }
    }

    /// Get node resource information / 获取节点资源信息
    async fn get_node_resource(
        &self,
        request: Request<GetNodeResourceRequest>,
    ) -> Result<Response<GetNodeResourceResponse>, Status> {
        let req = request.into_inner();
        let node_uuid = uuid::Uuid::parse_str(&req.node_uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        match self.resource_service.get_resource(&node_uuid).await {
            Ok(Some(resource_info)) => {
                let response = GetNodeResourceResponse {
                    found: true,
                    resource: Some(Self::resource_info_to_proto_resource(&resource_info)),
                };
                Ok(Response::new(response))
            }
            Ok(None) => {
                let response = GetNodeResourceResponse {
                    found: false,
                    resource: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => Err(Status::internal(format!("Get resource failed: {}", e))),
        }
    }

    /// List node resources / 列出节点资源信息
    async fn list_node_resources(
        &self,
        request: Request<ListNodeResourcesRequest>,
    ) -> Result<Response<ListNodeResourcesResponse>, Status> {
        let req = request.into_inner();
        
        let resources = if req.node_uuids.is_empty() {
            // List all resources / 列出所有资源
            self.resource_service.list_resources().await
        } else {
            // Filter by specific node UUIDs / 按特定节点UUID过滤
            let mut node_uuids = Vec::new();
            for uuid_str in &req.node_uuids {
                match uuid::Uuid::parse_str(uuid_str) {
                    Ok(uuid) => node_uuids.push(uuid),
                    Err(_) => return Err(Status::invalid_argument(format!("Invalid UUID format: {}", uuid_str))),
                }
            }
            self.resource_service.list_resources_by_nodes(&node_uuids).await
        };
        
        match resources {
            Ok(resource_list) => {
                let proto_resources: Vec<_> = resource_list.iter()
                    .map(|resource| Self::resource_info_to_proto_resource(resource))
                    .collect();
                
                let response = ListNodeResourcesResponse {
                    resources: proto_resources,
                };
                Ok(Response::new(response))
            }
            Err(e) => Err(Status::internal(format!("List resources failed: {}", e))),
        }
    }

    /// Get node with resource information / 获取节点及其资源信息
    async fn get_node_with_resource(
        &self,
        request: Request<GetNodeWithResourceRequest>,
    ) -> Result<Response<GetNodeWithResourceResponse>, Status> {
        let req = request.into_inner();
        let node_uuid = uuid::Uuid::parse_str(&req.uuid)
            .map_err(|_| Status::invalid_argument("Invalid UUID format"))?;
        
        // Get node info / 获取节点信息
        let node_service = self.node_service.read().await;
        let node_result = node_service.get_node(&node_uuid.to_string()).await;
        drop(node_service);
        
        // Get resource info / 获取资源信息
        let resource_result = self.resource_service.get_resource(&node_uuid).await;
        
        match (node_result, resource_result) {
            (Ok(Some(node)), Ok(resource_info)) => {
                let response = GetNodeWithResourceResponse {
                    found: true,
                    node: Some(node),
                    resource: resource_info.map(|r| Self::resource_info_to_proto_resource(&r)),
                };
                Ok(Response::new(response))
            }
            (Ok(None), _) => {
                let response = GetNodeWithResourceResponse {
                    found: false,
                    node: None,
                    resource: None,
                };
                Ok(Response::new(response))
            }
            (Err(e), _) => Err(Status::internal(format!("Get node failed: {}", e))),
            (_, Err(e)) => Err(Status::internal(format!("Get resource failed: {}", e))),
        }
    }
}

// Implement TaskService trait / 实现TaskService trait
#[tonic::async_trait]
impl TaskServiceTrait for SmsServiceImpl {
    /// Register a new task / 注册新任务
    async fn register_task(
        &self,
        request: Request<RegisterTaskRequest>,
    ) -> Result<Response<RegisterTaskResponse>, Status> {
        let req = request.into_inner();
        
        // Create task from request fields
        let task = crate::proto::sms::Task {
            task_id: uuid::Uuid::new_v4().to_string(),
            name: req.name,
            description: req.description,
            status: crate::proto::sms::TaskStatus::Registered as i32,
            priority: req.priority as i32,
            node_uuid: req.node_uuid,
            endpoint: req.endpoint,
            version: req.version,
            capabilities: req.capabilities,
            registered_at: chrono::Utc::now().timestamp(),
            last_heartbeat: chrono::Utc::now().timestamp(),
            metadata: req.metadata,
            config: req.config,
        };
        
        let mut task_service = self.task_service.write().await;
        match task_service.register_task(task.clone()).await {
            Ok(_) => {
                 let response = RegisterTaskResponse {
                     success: true,
                     message: "Task registered successfully".to_string(),
                     task_id: task.task_id.clone(),
                     task: Some(task),
                 };
                 Ok(Response::new(response))
             }
             Err(e) => {
                 let response = RegisterTaskResponse {
                     success: false,
                     message: format!("Failed to register task: {}", e),
                     task_id: String::new(),
                     task: None,
                 };
                 Ok(Response::new(response))
             }
        }
    }

    /// List tasks with optional filtering / 列出任务（可选过滤）
    async fn list_tasks(
        &self,
        request: Request<ListTasksRequest>,
    ) -> Result<Response<ListTasksResponse>, Status> {
        let req = request.into_inner();
        
        let task_service = self.task_service.read().await;
        
        // Convert filter parameters / 转换过滤参数
        let node_uuid = if req.node_uuid.is_empty() { None } else { Some(req.node_uuid.as_str()) };
        let status_filter = if req.status_filter < 0 { None } else { Some(req.status_filter) };
        let priority_filter = if req.priority_filter < 0 { None } else { Some(req.priority_filter) };
        let limit = if req.limit <= 0 { None } else { Some(req.limit) };
        let offset = if req.offset < 0 { None } else { Some(req.offset) };
        
        match task_service.list_tasks_with_filters(
            node_uuid,
            status_filter,
            priority_filter,
            limit,
            offset,
        ).await {
             Ok(tasks) => {
                 // Get total count before filtering for pagination / 获取过滤前的总数用于分页
                 let all_tasks = task_service.list_tasks().await.unwrap_or_default();
                 let response = ListTasksResponse {
                     tasks: tasks.clone(),
                     total_count: all_tasks.len() as i32,
                 };
                 Ok(Response::new(response))
             }
             Err(e) => Err(Status::internal(format!("Failed to list tasks: {}", e))),
         }
    }

    /// Get task details by ID / 根据ID获取任务详情
    async fn get_task(
        &self,
        request: Request<GetTaskRequest>,
    ) -> Result<Response<GetTaskResponse>, Status> {
        let req = request.into_inner();
        
        let task_service = self.task_service.read().await;
        match task_service.get_task(&req.task_id).await {
            Ok(Some(task)) => {
                let response = GetTaskResponse {
                    found: true,
                    task: Some(task),
                };
                Ok(Response::new(response))
            }
            Ok(None) => {
                let response = GetTaskResponse {
                    found: false,
                    task: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => Err(Status::internal(format!("Failed to get task: {}", e))),
        }
    }

    /// Unregister a task / 注销任务
    async fn unregister_task(
        &self,
        request: Request<UnregisterTaskRequest>,
    ) -> Result<Response<UnregisterTaskResponse>, Status> {
        let req = request.into_inner();
        
        let mut task_service = self.task_service.write().await;
        match task_service.remove_task(&req.task_id).await {
             Ok(_) => {
                 let response = UnregisterTaskResponse {
                     success: true,
                     message: "Task unregistered successfully".to_string(),
                     task_id: req.task_id.clone(),
                 };
                 Ok(Response::new(response))
             }
             Err(e) => {
                 let response = UnregisterTaskResponse {
                     success: false,
                     message: format!("Failed to unregister task: {}", e),
                     task_id: req.task_id.clone(),
                 };
                 Ok(Response::new(response))
             }
         }
    }
}
