//! SMS Service Implementation / SMS服务实现
use std::collections::{HashMap, VecDeque};
use std::sync::{atomic::AtomicU64, atomic::Ordering, Arc};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, Mutex, RwLock};
use tonic::{Request, Response, Status};

use uuid::Uuid;

use crate::sms::config::SmsConfig;
use crate::sms::events::TaskEventBus;
use crate::sms::services::{
    node_service::NodeService, resource_service::ResourceService,
    task_service::TaskService as TaskServiceImpl,
};
use crate::storage::kv::{create_kv_store_from_config, get_kv_store_factory, KvStoreConfig};
use anyhow::Context;
use dashmap::DashMap;
use futures::stream::unfold;
use tokio::time::Duration;
use tokio_stream::StreamExt;
use tracing::{debug, warn};

// Import proto types / 导入proto类型
use crate::proto::sms::{
    mcp_registry_service_server::McpRegistryService as McpRegistryServiceTrait,
    node_service_server::NodeService as NodeServiceTrait,
    placement_service_server::PlacementService as PlacementServiceTrait,
    task_service_server::TaskService as TaskServiceTrait,
    DeleteMcpServerRequest,
    DeleteMcpServerResponse,
    DeleteNodeRequest,
    DeleteNodeResponse,
    GetNodeRequest,
    GetNodeResourceRequest,
    GetNodeResourceResponse,
    GetNodeResponse,
    GetNodeWithResourceRequest,
    GetNodeWithResourceResponse,
    GetTaskRequest,
    GetTaskResponse,
    HeartbeatRequest,
    HeartbeatResponse,
    InvocationOutcomeClass,
    ListMcpServersRequest,
    ListMcpServersResponse,
    ListNodeResourcesRequest,
    ListNodeResourcesResponse,
    ListNodesRequest,
    ListNodesResponse,
    ListTasksRequest,
    ListTasksResponse,
    McpRegistryEvent,
    McpServerRecord,
    McpTransport,
    NodeCandidate,
    PlaceInvocationRequest,
    PlaceInvocationResponse,
    // Node service messages / 节点服务消息
    RegisterNodeRequest,
    RegisterNodeResponse,
    // Task service messages / 任务服务消息
    RegisterTaskRequest,
    RegisterTaskResponse,
    ReportInvocationOutcomeRequest,
    ReportInvocationOutcomeResponse,
    UnregisterTaskRequest,
    UnregisterTaskResponse,
    UpdateNodeRequest,
    UpdateNodeResourceRequest,
    UpdateNodeResourceResponse,
    UpdateNodeResponse,
    UpdateTaskResultRequest,
    UpdateTaskResultResponse,
    UpdateTaskStatusRequest,
    UpdateTaskStatusResponse,
    UpsertMcpServerRequest,
    UpsertMcpServerResponse,
    WatchMcpServersRequest,
    WatchMcpServersResponse,
};

#[derive(Debug)]
struct McpRegistryState {
    revision: AtomicU64,
    records: RwLock<HashMap<String, McpServerRecord>>,
    recent_events: Mutex<VecDeque<McpRegistryEvent>>,
    event_tx: broadcast::Sender<McpRegistryEvent>,
}

impl McpRegistryState {
    fn new(event_buffer_size: usize, broadcast_buffer_size: usize) -> Self {
        let (event_tx, _rx) = broadcast::channel(broadcast_buffer_size);
        Self {
            revision: AtomicU64::new(0),
            records: RwLock::new(HashMap::new()),
            recent_events: Mutex::new(VecDeque::with_capacity(event_buffer_size)),
            event_tx,
        }
    }

    fn current_revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    fn bump_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::Relaxed) + 1
    }
}

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
    events: Arc<TaskEventBus>,
    placement_state: Arc<PlacementState>,
    mcp_registry: Arc<McpRegistryState>,
}

impl SmsServiceImpl {
    async fn upsert_mcp_record_inner(&self, mut record: McpServerRecord) -> Result<u64, Status> {
        if record.server_id.is_empty() {
            return Err(Status::invalid_argument("server_id is required"));
        }
        let server_id = record.server_id.clone();
        if record.tool_namespace.is_empty() {
            record.tool_namespace = format!("mcp.{}", record.server_id);
        }

        match record.transport {
            x if x == McpTransport::Stdio as i32 => {
                let stdio = record
                    .stdio
                    .as_ref()
                    .ok_or_else(|| Status::invalid_argument("stdio config is required"))?;
                if stdio.command.is_empty() {
                    return Err(Status::invalid_argument("stdio.command is required"));
                }
            }
            x if x == McpTransport::StreamableHttp as i32 => {
                let http = record
                    .http
                    .as_ref()
                    .ok_or_else(|| Status::invalid_argument("http config is required"))?;
                if http.url.is_empty() {
                    return Err(Status::invalid_argument("http.url is required"));
                }
            }
            _ => {
                return Err(Status::invalid_argument("invalid transport"));
            }
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        record.updated_at_ms = now_ms;

        {
            let mut records = self.mcp_registry.records.write().await;
            records.insert(server_id.clone(), record);
        }

        let revision = self.mcp_registry.bump_revision();
        let event = McpRegistryEvent {
            revision,
            upserts: vec![server_id],
            deletes: vec![],
        };

        {
            let mut events = self.mcp_registry.recent_events.lock().await;
            if events.len() >= 1024 {
                events.pop_front();
            }
            events.push_back(event.clone());
        }
        let _ = self.mcp_registry.event_tx.send(event);

        Ok(revision)
    }

    pub async fn bootstrap_mcp_from_dir(&self, dir: &str) -> anyhow::Result<usize> {
        if dir.is_empty() {
            return Ok(0);
        }

        fn expand_tilde(s: &str) -> String {
            if let Some(rest) = s.strip_prefix("~/") {
                if let Ok(home) = std::env::var("HOME") {
                    return format!("{}/{}", home, rest);
                }
            }
            s.to_string()
        }

        #[derive(serde::Deserialize)]
        struct FileCfg {
            server_id: String,
            display_name: Option<String>,
            transport: String,
            stdio: Option<FileStdio>,
            http: Option<FileHttp>,
            tool_namespace: Option<String>,
            allowed_tools: Option<Vec<String>>,
            budgets: Option<FileBudgets>,
            approval_policy: Option<FileApprovalPolicy>,
        }
        #[derive(serde::Deserialize)]
        struct FileStdio {
            command: String,
            args: Option<Vec<String>>,
            env: Option<std::collections::HashMap<String, String>>,
            cwd: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct FileHttp {
            url: String,
            headers: Option<std::collections::HashMap<String, String>>,
            auth_ref: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct FileBudgets {
            tool_timeout_ms: Option<u64>,
            max_concurrency: Option<u64>,
            max_tool_output_bytes: Option<u64>,
        }
        #[derive(serde::Deserialize)]
        struct FileApprovalPolicy {
            default_policy: Option<String>,
            per_tool: Option<std::collections::HashMap<String, String>>,
        }

        let dir = expand_tilde(dir);
        let mut count = 0usize;
        let entries = std::fs::read_dir(&dir).with_context(|| format!("read_dir {}", dir))?;
        for ent in entries {
            let ent = match ent {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = ent.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext != "toml" && ext != "json" {
                continue;
            }
            let content = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let cfg: FileCfg = if ext == "toml" {
                match toml::from_str(&content) {
                    Ok(v) => v,
                    Err(_) => continue,
                }
            } else {
                match serde_json::from_str(&content) {
                    Ok(v) => v,
                    Err(_) => continue,
                }
            };

            let transport = match cfg.transport.as_str() {
                "stdio" => McpTransport::Stdio as i32,
                "streamable_http" => McpTransport::StreamableHttp as i32,
                _ => continue,
            };

            let record = McpServerRecord {
                server_id: cfg.server_id,
                display_name: cfg.display_name.unwrap_or_default(),
                transport,
                stdio: cfg.stdio.map(|s| crate::proto::sms::McpStdioConfig {
                    command: s.command,
                    args: s.args.unwrap_or_default(),
                    env: s.env.unwrap_or_default(),
                    cwd: s.cwd.unwrap_or_default(),
                }),
                http: cfg.http.map(|h| crate::proto::sms::McpHttpConfig {
                    url: h.url,
                    headers: h.headers.unwrap_or_default(),
                    auth_ref: h.auth_ref.unwrap_or_default(),
                }),
                tool_namespace: cfg.tool_namespace.unwrap_or_default(),
                allowed_tools: cfg.allowed_tools.unwrap_or_default(),
                approval_policy: cfg.approval_policy.map(|p| {
                    crate::proto::sms::McpApprovalPolicy {
                        default_policy: p.default_policy.unwrap_or_default(),
                        per_tool: p.per_tool.unwrap_or_default(),
                    }
                }),
                budgets: cfg.budgets.map(|b| crate::proto::sms::McpBudgets {
                    tool_timeout_ms: b.tool_timeout_ms.unwrap_or(0),
                    max_concurrency: b.max_concurrency.unwrap_or(0),
                    max_tool_output_bytes: b.max_tool_output_bytes.unwrap_or(0),
                }),
                updated_at_ms: 0,
            };

            if self.upsert_mcp_record_inner(record).await.is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }
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
        // Create KV store for events via factory, allow separate config / 事件KV支持独立配置
        let supported = get_kv_store_factory().supported_backends();
        let kv_cfg = if let Some(ev) = &config.event_kv {
            let backend = if supported.contains(&ev.backend) {
                ev.backend.clone()
            } else {
                "memory".to_string()
            };
            KvStoreConfig {
                backend,
                params: ev.params.clone(),
            }
        } else {
            KvStoreConfig {
                backend: "memory".to_string(),
                params: std::collections::HashMap::new(),
            }
        };
        let kv_box = create_kv_store_from_config(&kv_cfg)
            .await
            .expect("Failed to create KV store from config");
        let kv: Arc<dyn crate::storage::kv::KvStore> = Arc::from(kv_box);
        let events = Arc::new(TaskEventBus::new(kv));

        let cleanup_node_service = node_service.clone();
        let cleanup_resource_service = resource_service.clone();
        let cleanup_config = config.clone();
        tokio::spawn(async move {
            let mut t = tokio::time::interval(Duration::from_secs(cleanup_config.cleanup_interval));
            loop {
                t.tick().await;
                let updated_nodes = {
                    let mut svc = cleanup_node_service.write().await;
                    svc.mark_unhealthy_nodes_offline(cleanup_config.heartbeat_timeout)
                        .await
                        .unwrap_or_default()
                };
                if !updated_nodes.is_empty() {
                    tracing::info!(
                        count = updated_nodes.len(),
                        heartbeat_timeout_s = cleanup_config.heartbeat_timeout,
                        nodes = ?updated_nodes,
                        "Marked unhealthy nodes offline"
                    );
                }
                let _ = cleanup_resource_service
                    .cleanup_stale_resources(cleanup_config.heartbeat_timeout)
                    .await;
            }
        });

        Self {
            node_service,
            resource_service,
            config,
            task_service,
            events,
            placement_state: Arc::new(PlacementState::new()),
            mcp_registry: Arc::new(McpRegistryState::new(1024, 1024)),
        }
    }

    /// Convert proto Node to internal NodeInfo / 将proto Node转换为内部NodeInfo
    #[allow(dead_code)]
    fn proto_node_to_node_info(
        proto_node: crate::proto::sms::Node,
    ) -> crate::sms::services::node_service::NodeInfo {
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
    fn node_info_to_proto_node(
        node_info: &crate::sms::services::node_service::NodeInfo,
    ) -> crate::proto::sms::Node {
        // Parse IP and port from address
        let (ip, port) = if let Some(colon_pos) = node_info.address.rfind(':') {
            let ip = node_info.address[..colon_pos].to_string();
            let port = node_info.address[colon_pos + 1..]
                .parse::<i32>()
                .unwrap_or(8080);
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
    #[allow(clippy::result_large_err)]
    fn proto_resource_to_resource_info(
        proto_resource: &crate::proto::sms::NodeResource,
    ) -> Result<crate::sms::services::resource_service::NodeResourceInfo, Status> {
        let node_uuid = uuid::Uuid::parse_str(&proto_resource.node_uuid)
            .map_err(|_| Status::invalid_argument("Invalid node UUID format"))?;

        let updated_at = if proto_resource.updated_at > 0 {
            chrono::DateTime::from_timestamp(proto_resource.updated_at, 0)
                .unwrap_or_else(chrono::Utc::now)
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
    fn resource_info_to_proto_resource(
        resource_info: &crate::sms::services::resource_service::NodeResourceInfo,
    ) -> crate::proto::sms::NodeResource {
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
    pub async fn with_storage_config(storage_config: &crate::config::base::StorageConfig) -> Self {
        // Create KV store from storage config / 从存储配置创建KV存储
        use crate::storage::{create_kv_store_from_config, KvStoreConfig};
        let kv_config = KvStoreConfig::from_storage_config(storage_config);
        let _kv_store = create_kv_store_from_config(&kv_config)
            .await
            .expect("Failed to create KV store from storage config");

        let node_service = Arc::new(RwLock::new(NodeService::new()));
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

#[tonic::async_trait]
impl McpRegistryServiceTrait for SmsServiceImpl {
    type WatchMcpServersStream = std::pin::Pin<
        Box<
            dyn tokio_stream::Stream<Item = Result<WatchMcpServersResponse, Status>>
                + Send
                + 'static,
        >,
    >;

    async fn list_mcp_servers(
        &self,
        _request: Request<ListMcpServersRequest>,
    ) -> Result<Response<ListMcpServersResponse>, Status> {
        let revision = self.mcp_registry.current_revision();
        let records = self.mcp_registry.records.read().await;
        let servers = records.values().cloned().collect::<Vec<_>>();
        Ok(Response::new(ListMcpServersResponse { revision, servers }))
    }

    async fn watch_mcp_servers(
        &self,
        request: Request<WatchMcpServersRequest>,
    ) -> Result<Response<Self::WatchMcpServersStream>, Status> {
        let since_revision = request.into_inner().since_revision;

        let mut pending = VecDeque::new();
        {
            let events = self.mcp_registry.recent_events.lock().await;
            if let Some(oldest) = events.front().map(|e| e.revision) {
                if since_revision != 0 && since_revision < oldest {
                    return Err(Status::failed_precondition(
                        "since_revision too old; resync required",
                    ));
                }
            }
            for e in events.iter() {
                if e.revision > since_revision {
                    pending.push_back(e.clone());
                }
            }
        }

        let rx = self.mcp_registry.event_tx.subscribe();

        struct WatchState {
            pending: VecDeque<McpRegistryEvent>,
            rx: broadcast::Receiver<McpRegistryEvent>,
        }

        let stream = unfold(WatchState { pending, rx }, |mut st| async move {
            if let Some(event) = st.pending.pop_front() {
                return Some((Ok(WatchMcpServersResponse { event: Some(event) }), st));
            }

            match st.rx.recv().await {
                Ok(event) => Some((Ok(WatchMcpServersResponse { event: Some(event) }), st)),
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    Some((Err(Status::aborted("watch lagged; resync required")), st))
                }
                Err(broadcast::error::RecvError::Closed) => None,
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn upsert_mcp_server(
        &self,
        request: Request<UpsertMcpServerRequest>,
    ) -> Result<Response<UpsertMcpServerResponse>, Status> {
        let record = request
            .into_inner()
            .record
            .ok_or_else(|| Status::invalid_argument("record is required"))?;

        let revision = self.upsert_mcp_record_inner(record).await?;
        Ok(Response::new(UpsertMcpServerResponse { revision }))
    }

    async fn delete_mcp_server(
        &self,
        request: Request<DeleteMcpServerRequest>,
    ) -> Result<Response<DeleteMcpServerResponse>, Status> {
        let server_id = request.into_inner().server_id;
        if server_id.is_empty() {
            return Err(Status::invalid_argument("server_id is required"));
        }

        let existed = {
            let mut records = self.mcp_registry.records.write().await;
            records.remove(&server_id).is_some()
        };
        if !existed {
            return Err(Status::not_found("server not found"));
        }

        let revision = self.mcp_registry.bump_revision();
        let event = McpRegistryEvent {
            revision,
            upserts: vec![],
            deletes: vec![server_id],
        };

        {
            let mut events = self.mcp_registry.recent_events.lock().await;
            if events.len() >= 1024 {
                events.pop_front();
            }
            events.push_back(event.clone());
        }
        let _ = self.mcp_registry.event_tx.send(event);

        Ok(Response::new(DeleteMcpServerResponse { revision }))
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
        let node = req
            .node
            .ok_or_else(|| Status::invalid_argument("Node is required"))?;

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
        let node = req
            .node
            .ok_or_else(|| Status::invalid_argument("Node is required"))?;

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
                let _ = self.resource_service.remove_resource(&node_uuid).await;
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
        let mut node_service = self.node_service.write().await;

        match node_service
            .update_heartbeat(&req.uuid, chrono::Utc::now().timestamp())
            .await
        {
            Ok(_) => {
                tracing::debug!(uuid = %req.uuid, "Heartbeat received");
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
                let response = ListNodesResponse { nodes: node_list };
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
        let node_service = self.node_service.read().await;

        match node_service.get_node(&req.uuid).await {
            Ok(Some(node)) => {
                let response = GetNodeResponse {
                    found: true,
                    node: Some(node),
                };
                Ok(Response::new(response))
            }
            Ok(None) => Err(Status::not_found("Node not found")),
            Err(e) => Err(e.into()),
        }
    }

    /// Update node resource information / 更新节点资源信息
    async fn update_node_resource(
        &self,
        request: Request<UpdateNodeResourceRequest>,
    ) -> Result<Response<UpdateNodeResourceResponse>, Status> {
        let req = request.into_inner();

        let resource = req
            .resource
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
        // Allow non-UUID identifiers: if parsing fails, treat as no resource found
        let res = if let Ok(u) = uuid::Uuid::parse_str(&req.node_uuid) {
            self.resource_service.get_resource(&u).await
        } else {
            Ok(None)
        };
        match res {
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
                if let Ok(uuid) = uuid::Uuid::parse_str(uuid_str) {
                    node_uuids.push(uuid);
                }
            }
            self.resource_service
                .list_resources_by_nodes(&node_uuids)
                .await
        };

        match resources {
            Ok(resource_list) => {
                let proto_resources: Vec<_> = resource_list
                    .iter()
                    .map(Self::resource_info_to_proto_resource)
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
        let node_id = req.uuid;

        // Get node info / 获取节点信息
        let node_service = self.node_service.read().await;
        let node_result = node_service.get_node(&node_id).await;
        drop(node_service);

        // Get resource info / 获取资源信息（如果node_id不是UUID则返回None）
        let resource_result = if let Ok(uuid) = uuid::Uuid::parse_str(&node_id) {
            self.resource_service.get_resource(&uuid).await
        } else {
            Ok(None)
        };

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
    type SubscribeTaskEventsStream = std::pin::Pin<
        Box<
            dyn tokio_stream::Stream<Item = Result<crate::proto::sms::TaskEvent, Status>>
                + Send
                + 'static,
        >,
    >;
    /// Register a new task / 注册新任务
    async fn register_task(
        &self,
        request: Request<RegisterTaskRequest>,
    ) -> Result<Response<RegisterTaskResponse>, Status> {
        let req = request.into_inner();

        // Create task from request fields
        let mut task = crate::proto::sms::Task {
            task_id: uuid::Uuid::new_v4().to_string(),
            name: req.name,
            description: req.description,
            status: crate::proto::sms::TaskStatus::Registered as i32,
            priority: req.priority,
            node_uuid: req.node_uuid,
            endpoint: req.endpoint,
            version: req.version,
            capabilities: req.capabilities,
            registered_at: chrono::Utc::now().timestamp(),
            last_heartbeat: chrono::Utc::now().timestamp(),
            metadata: req.metadata,
            config: req.config,
            executable: req.executable,
            result_uris: Vec::new(),
            last_result_uri: String::new(),
            last_result_status: String::new(),
            last_completed_at: 0,
            last_result_metadata: std::collections::HashMap::new(),
            execution_kind: crate::proto::sms::TaskExecutionKind::Unknown as i32,
        };
        // Derive execution_kind / 解析执行类型
        task.execution_kind =
            if req.execution_kind != crate::proto::sms::TaskExecutionKind::Unknown as i32 {
                req.execution_kind
            } else {
                let ek = task
                    .metadata
                    .get("execution_kind")
                    .cloned()
                    .or_else(|| task.config.get("execution_kind").cloned())
                    .unwrap_or_else(|| "short_running".to_string());
                if ek.eq_ignore_ascii_case("long_running") {
                    crate::proto::sms::TaskExecutionKind::LongRunning as i32
                } else {
                    crate::proto::sms::TaskExecutionKind::ShortRunning as i32
                }
            };

        let mut task_service = self.task_service.write().await;
        match task_service.register_task(task.clone()).await {
            Ok(_) => {
                debug!(task_id = %task.task_id, node_uuid = %task.node_uuid, "RegisterTask: publishing create event");
                // Publish create event / 发布创建事件
                if let Err(e) = self.events.publish_create(&task).await {
                    warn!(error = %e, "Publish create event failed");
                }
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
        let node_uuid = if req.node_uuid.is_empty() {
            None
        } else {
            Some(req.node_uuid.as_str())
        };
        let status_filter = if req.status_filter < 0 {
            None
        } else {
            Some(req.status_filter)
        };
        let priority_filter = if req.priority_filter < 0 {
            None
        } else {
            Some(req.priority_filter)
        };
        let limit = if req.limit <= 0 {
            None
        } else {
            Some(req.limit)
        };
        let offset = if req.offset < 0 {
            None
        } else {
            Some(req.offset)
        };

        match task_service
            .list_tasks_with_filters(node_uuid, status_filter, priority_filter, limit, offset)
            .await
        {
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

    /// Subscribe task events for a node / 订阅节点任务事件
    async fn subscribe_task_events(
        &self,
        request: Request<crate::proto::sms::SubscribeTaskEventsRequest>,
    ) -> Result<tonic::Response<Self::SubscribeTaskEventsStream>, Status> {
        let req = request.into_inner();
        if req.node_uuid.is_empty() {
            return Err(Status::invalid_argument("node_uuid is required"));
        }
        let node_uuid = req.node_uuid;
        let last = req.last_event_id;
        // Durable replay first
        let replay = self
            .events
            .replay_since(&node_uuid, last, 1000)
            .await
            .map_err(|e| Status::internal(format!("Replay failed: {}", e)))?;
        debug!(node_uuid = %node_uuid, last_event_id = last, replay_count = replay.len(), "Subscribe: prepared replay events");
        let replay_stream = tokio_stream::iter(replay.into_iter().map(Ok));
        // Live broadcast
        let rx = self.events.subscribe(&node_uuid).await;
        debug!(node_uuid = %node_uuid, "Subscribe: live broadcast receiver created");
        let live_stream = unfold(rx, |mut r| async move {
            match r.recv().await {
                Ok(ev) => Some((Ok(ev), r)),
                Err(e) => {
                    warn!(error = %e, "Broadcast receive error, ending live stream");
                    None
                }
            }
        });
        let stream = replay_stream.chain(live_stream);
        debug!(node_uuid = %node_uuid, "Subscribe: returning combined stream");
        Ok(tonic::Response::new(Box::pin(stream)))
    }

    /// Update task status (observed state) / 更新任务状态（观测态）
    async fn update_task_status(
        &self,
        request: Request<UpdateTaskStatusRequest>,
    ) -> Result<Response<UpdateTaskStatusResponse>, Status> {
        let req = request.into_inner();
        debug!(task_id = %req.task_id, node_uuid = %req.node_uuid, status = req.status, status_version = req.status_version, updated_at = req.updated_at, reason = %req.reason, "UpdateTaskStatus: request received");
        if req.task_id.is_empty() {
            return Err(Status::invalid_argument("task_id is required"));
        }

        let mut task_service = self.task_service.write().await;
        match task_service.get_task(&req.task_id).await {
            Ok(Some(mut task)) => {
                // Apply status update / 应用状态更新
                let old_status = task.status;
                task.status = req.status;
                if req.updated_at > 0 {
                    task.last_heartbeat = req.updated_at;
                } else {
                    task.last_heartbeat = chrono::Utc::now().timestamp();
                }
                debug!(task_id = %task.task_id, old_status = old_status, new_status = task.status, last_heartbeat = task.last_heartbeat, "UpdateTaskStatus: applied state change");
                // Persist / 持久化
                match task_service.register_task(task.clone()).await {
                    Ok(_) => {
                        debug!(task_id = %task.task_id, "UpdateTaskStatus: persisted");
                    }
                    Err(e) => {
                        warn!(error = %e.to_string(), task_id = %task.task_id, "UpdateTaskStatus: persist failed");
                    }
                }

                // Optionally publish update event / 可选发布更新事件
                if let Err(e) = self.events.publish_update(&task).await {
                    warn!(error = %e, task_id = %task.task_id, "Publish update event failed");
                }

                let resp = UpdateTaskStatusResponse {
                    success: true,
                    message: "Task status updated".to_string(),
                    task: Some(task),
                };
                Ok(Response::new(resp))
            }
            Ok(None) => {
                debug!(task_id = %req.task_id, "UpdateTaskStatus: task not found");
                let resp = UpdateTaskStatusResponse {
                    success: false,
                    message: "Task not found".to_string(),
                    task: None,
                };
                Ok(Response::new(resp))
            }
            Err(e) => Err(Status::internal(format!("Failed to get task: {}", e))),
        }
    }

    /// Update task result fields / 更新任务结果字段
    async fn update_task_result(
        &self,
        request: Request<UpdateTaskResultRequest>,
    ) -> Result<Response<UpdateTaskResultResponse>, Status> {
        let req = request.into_inner();
        if req.task_id.is_empty() {
            return Err(Status::invalid_argument("task_id is required"));
        }
        let mut task_service = self.task_service.write().await;
        match task_service.get_task(&req.task_id).await {
            Ok(Some(mut task)) => {
                if !req.result_uri.is_empty() {
                    task.last_result_uri = req.result_uri.clone();
                    if !task.result_uris.contains(&req.result_uri) {
                        task.result_uris.push(req.result_uri.clone());
                    }
                }
                task.last_result_status = req.result_status.clone();
                task.last_completed_at = if req.completed_at > 0 {
                    req.completed_at
                } else {
                    chrono::Utc::now().timestamp()
                };
                task.last_result_metadata = req.result_metadata.clone();

                match task_service.register_task(task.clone()).await {
                    Ok(_) => {}
                    Err(e) => return Err(Status::internal(format!("Persist failed: {}", e))),
                }

                if let Err(e) = self.events.publish_update(&task).await {
                    warn!(error = %e, task_id = %task.task_id, "Publish update event failed");
                }
                let resp = UpdateTaskResultResponse {
                    success: true,
                    message: "Task result updated".to_string(),
                    task: Some(task),
                };
                Ok(Response::new(resp))
            }
            Ok(None) => Ok(Response::new(UpdateTaskResultResponse {
                success: false,
                message: "Task not found".to_string(),
                task: None,
            })),
            Err(e) => Err(Status::internal(format!("Failed to get task: {}", e))),
        }
    }
}

#[derive(Debug)]
struct PlacementDecisionRecord {
    // Decision tracking for debugging/observability.
    // 用于调试与可观测性的决策记录。
    request_id: String,
    task_id: String,
    candidates: Vec<String>,
    created_at: i64,
}

#[derive(Debug, Clone)]
struct NodePenalty {
    // Consecutive retryable failures to drive exponential backoff.
    // 连续可重试失败次数，用于指数退避。
    consecutive_failures: u32,
    // If now < blocked_until, the node is temporarily removed from candidate set.
    // 若 now < blocked_until，则节点被临时熔断，不参与候选。
    blocked_until: i64,
    // Timestamp of last failure.
    // 最近一次失败的时间戳。
    last_failure_at: i64,
}

#[derive(Debug)]
struct PlacementState {
    decisions: DashMap<String, PlacementDecisionRecord>,
    node_penalties: DashMap<String, NodePenalty>,
    decision_ops: AtomicU64,
    penalty_ops: AtomicU64,
}

impl PlacementState {
    fn new() -> Self {
        Self {
            decisions: DashMap::new(),
            node_penalties: DashMap::new(),
            decision_ops: AtomicU64::new(0),
            penalty_ops: AtomicU64::new(0),
        }
    }

    fn maybe_prune_decisions(&self, now: i64) {
        const MAX_DECISIONS: usize = 10_000;
        const DECISION_TTL_SECS: i64 = 600;

        let op = self.decision_ops.fetch_add(1, Ordering::Relaxed);
        if op % 256 != 0 && self.decisions.len() <= MAX_DECISIONS {
            return;
        }

        let mut to_remove: Vec<String> = Vec::new();
        for item in self.decisions.iter() {
            if now - item.value().created_at > DECISION_TTL_SECS {
                to_remove.push(item.key().clone());
            }
        }
        for k in to_remove {
            self.decisions.remove(&k);
        }

        let extra = self.decisions.len().saturating_sub(MAX_DECISIONS);
        if extra == 0 {
            return;
        }
        let mut victims: Vec<String> = Vec::with_capacity(extra);
        for item in self.decisions.iter().take(extra) {
            victims.push(item.key().clone());
        }
        for k in victims {
            self.decisions.remove(&k);
        }
    }

    fn maybe_prune_node_penalties(&self, now: i64) {
        const PENALTY_TTL_SECS: i64 = 3600;

        let op = self.penalty_ops.fetch_add(1, Ordering::Relaxed);
        if op % 256 != 0 {
            return;
        }

        let mut to_remove: Vec<String> = Vec::new();
        for item in self.node_penalties.iter() {
            let p = item.value();
            if p.blocked_until > now {
                continue;
            }
            if p.last_failure_at == 0 {
                to_remove.push(item.key().clone());
                continue;
            }
            if now - p.last_failure_at > PENALTY_TTL_SECS {
                to_remove.push(item.key().clone());
            }
        }
        for k in to_remove {
            self.node_penalties.remove(&k);
        }
    }

    fn is_blocked(&self, node_uuid: &str, now: i64) -> bool {
        self.node_penalties
            .get(node_uuid)
            .map(|p| p.blocked_until > now)
            .unwrap_or(false)
    }

    fn penalty_score(&self, node_uuid: &str, now: i64) -> f64 {
        self.node_penalties
            .get(node_uuid)
            .map(|p| {
                if p.blocked_until > now {
                    10.0
                } else {
                    (p.consecutive_failures as f64).min(10.0)
                }
            })
            .unwrap_or(0.0)
    }

    fn record_decision(&self, decision_id: String, record: PlacementDecisionRecord) {
        self.decisions.insert(decision_id, record);
        let now = chrono::Utc::now().timestamp();
        self.maybe_prune_decisions(now);
    }

    fn apply_outcome(&self, node_uuid: String, outcome_class: InvocationOutcomeClass) {
        let now = chrono::Utc::now().timestamp();
        match outcome_class {
            InvocationOutcomeClass::Success => {
                // Success clears penalty state.
                // 成功会清空惩罚状态。
                self.node_penalties.remove(&node_uuid);
            }
            InvocationOutcomeClass::Overloaded
            | InvocationOutcomeClass::Unavailable
            | InvocationOutcomeClass::Timeout => {
                // Retryable failures trigger exponential backoff with a hard cap.
                // 可重试失败触发指数退避，并设置硬上限。
                self.node_penalties
                    .entry(node_uuid)
                    .and_modify(|p| {
                        p.consecutive_failures = p.consecutive_failures.saturating_add(1);
                        p.last_failure_at = now;
                        let base = 10i64;
                        let backoff = base * (1i64 << (p.consecutive_failures.min(5)));
                        p.blocked_until = (now + backoff).min(now + 300);
                    })
                    .or_insert(NodePenalty {
                        consecutive_failures: 1,
                        blocked_until: (now + 20).min(now + 300),
                        last_failure_at: now,
                    });
            }
            _ => {}
        }

        self.maybe_prune_node_penalties(now);
    }

    #[cfg(test)]
    fn get_node_penalty_snapshot(&self, node_uuid: &str) -> Option<(u32, i64, i64)> {
        self.node_penalties
            .get(node_uuid)
            .map(|p| (p.consecutive_failures, p.blocked_until, p.last_failure_at))
    }
}

#[tonic::async_trait]
impl PlacementServiceTrait for SmsServiceImpl {
    async fn place_invocation(
        &self,
        request: Request<PlaceInvocationRequest>,
    ) -> Result<Response<PlaceInvocationResponse>, Status> {
        let req = request.into_inner();
        if req.request_id.is_empty() {
            return Err(Status::invalid_argument("request_id is required"));
        }
        if req.task_id.is_empty() {
            return Err(Status::invalid_argument("task_id is required"));
        }
        let max_candidates = if req.max_candidates == 0 {
            3
        } else {
            req.max_candidates
        };
        let now = chrono::Utc::now().timestamp();
        self.placement_state.maybe_prune_node_penalties(now);
        let nodes = {
            let svc = self.node_service.read().await;
            svc.list_nodes()
                .await
                .map_err(|e| Status::internal(e.to_string()))?
        };
        let heartbeat_timeout = self.config.heartbeat_timeout as i64;
        let mut candidates: Vec<(NodeCandidate, f64)> = Vec::new();
        for node in nodes {
            // Filter nodes by liveness and heartbeat freshness.
            // 先按存活状态与心跳新鲜度过滤。
            if node.status.to_ascii_lowercase() != "online" {
                continue;
            }
            if now - node.last_heartbeat > heartbeat_timeout {
                continue;
            }
            // Skip nodes in temporary circuit-break.
            // 熔断中的节点不参与候选。
            if self.placement_state.is_blocked(&node.uuid, now) {
                continue;
            }

            let uuid = uuid::Uuid::parse_str(&node.uuid).ok();
            let resource = if let Some(u) = uuid {
                self.resource_service.get_resource(&u).await.ok().flatten()
            } else {
                None
            };
            let (cpu, mem, disk, load) = if let Some(r) = resource {
                (
                    r.cpu_usage_percent,
                    r.memory_usage_percent,
                    r.disk_usage_percent,
                    r.load_average_1m,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };
            let mut score = 100.0;
            // Simple weighted scoring: lower usage/load => higher score.
            // 简单加权评分：资源占用/负载越低，分数越高。
            score -= cpu.min(100.0) * 0.5;
            score -= mem.min(100.0) * 0.3;
            score -= disk.min(100.0) * 0.1;
            score -= (load.min(16.0) / 16.0) * 10.0;
            // Apply penalty score derived from historical retryable failures.
            // 基于历史可重试失败的惩罚项。
            score -= self.placement_state.penalty_score(&node.uuid, now) * 5.0;
            let candidate = NodeCandidate {
                node_uuid: node.uuid.clone(),
                ip_address: node.ip_address.clone(),
                port: node.port,
                score,
            };
            candidates.push((candidate, score));
        }

        candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
        candidates.truncate(max_candidates as usize);

        let decision_id = Uuid::new_v4().to_string();
        self.placement_state.record_decision(
            decision_id.clone(),
            PlacementDecisionRecord {
                request_id: req.request_id.clone(),
                task_id: req.task_id.clone(),
                candidates: candidates
                    .iter()
                    .map(|(c, _)| c.node_uuid.clone())
                    .collect(),
                created_at: now,
            },
        );

        let resp = PlaceInvocationResponse {
            decision_id,
            candidates: candidates.into_iter().map(|(c, _)| c).collect(),
        };
        Ok(Response::new(resp))
    }

    async fn report_invocation_outcome(
        &self,
        request: Request<ReportInvocationOutcomeRequest>,
    ) -> Result<Response<ReportInvocationOutcomeResponse>, Status> {
        let req = request.into_inner();
        if req.decision_id.is_empty() || req.request_id.is_empty() || req.task_id.is_empty() {
            return Err(Status::invalid_argument(
                "decision_id, request_id, task_id are required",
            ));
        }
        if req.node_uuid.is_empty() {
            return Err(Status::invalid_argument("node_uuid is required"));
        }
        let outcome_class = InvocationOutcomeClass::try_from(req.outcome_class)
            .unwrap_or(InvocationOutcomeClass::Unknown);
        // Feedback loop: update penalty state to influence subsequent placements.
        // 反馈闭环：更新惩罚状态，影响后续 placement。
        self.placement_state
            .apply_outcome(req.node_uuid, outcome_class);
        Ok(Response::new(ReportInvocationOutcomeResponse {
            accepted: true,
        }))
    }
}

#[cfg(test)]
impl SmsServiceImpl {
    pub fn test_get_node_penalty_snapshot(&self, node_uuid: &str) -> Option<(u32, i64, i64)> {
        self.placement_state.get_node_penalty_snapshot(node_uuid)
    }
}
