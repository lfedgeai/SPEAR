use axum_test::TestServer;
use spear_next::proto::sms::{
    node_service_client::NodeServiceClient, node_service_server::NodeServiceServer,
    placement_service_client::PlacementServiceClient,
    placement_service_server::PlacementServiceServer, task_service_client::TaskServiceClient,
    task_service_server::TaskServiceServer, Node, RegisterNodeRequest,
};
use spear_next::proto::spearlet::{
    function_service_server::FunctionServiceServer, InvokeFunctionRequest, InvokeFunctionResponse,
};
use spear_next::sms::gateway::GatewayState;
use spear_next::sms::service::SmsServiceImpl;
use spear_next::sms::web_admin::create_admin_router;
use std::sync::{atomic::AtomicUsize, atomic::Ordering, Arc};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};
use uuid::Uuid;

#[derive(Clone)]
struct MockFunctionService {
    mode: MockMode,
}

#[derive(Clone, Copy)]
enum MockMode {
    Unavailable,
    InvalidArgument,
    Success,
}

#[tonic::async_trait]
impl spear_next::proto::spearlet::function_service_server::FunctionService for MockFunctionService {
    async fn invoke_function(
        &self,
        request: Request<InvokeFunctionRequest>,
    ) -> Result<Response<InvokeFunctionResponse>, Status> {
        let req = request.into_inner();
        match self.mode {
            MockMode::Unavailable => Err(Status::unavailable("mock unavailable")),
            MockMode::InvalidArgument => Err(Status::invalid_argument("mock invalid")),
            MockMode::Success => Ok(Response::new(InvokeFunctionResponse {
                success: true,
                message: "ok".to_string(),
                execution_id: req.execution_id.unwrap_or_default(),
                task_id: req.task_id,
                ..Default::default()
            })),
        }
    }

    async fn get_execution_status(
        &self,
        _request: Request<spear_next::proto::spearlet::GetExecutionStatusRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::GetExecutionStatusResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn cancel_execution(
        &self,
        _request: Request<spear_next::proto::spearlet::CancelExecutionRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::CancelExecutionResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    type StreamFunctionStream = std::pin::Pin<
        Box<
            dyn tokio_stream::Stream<
                    Item = Result<spear_next::proto::spearlet::StreamExecutionResult, Status>,
                > + Send,
        >,
    >;

    async fn stream_function(
        &self,
        _request: Request<InvokeFunctionRequest>,
    ) -> Result<Response<Self::StreamFunctionStream>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn list_tasks(
        &self,
        _request: Request<spear_next::proto::spearlet::ListTasksRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::ListTasksResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn get_task(
        &self,
        _request: Request<spear_next::proto::spearlet::GetTaskRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::GetTaskResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn delete_task(
        &self,
        _request: Request<spear_next::proto::spearlet::DeleteTaskRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::DeleteTaskResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn list_executions(
        &self,
        _request: Request<spear_next::proto::spearlet::ListExecutionsRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::ListExecutionsResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn get_health(
        &self,
        _request: Request<spear_next::proto::spearlet::GetHealthRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::GetHealthResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn get_stats(
        &self,
        _request: Request<spear_next::proto::spearlet::GetStatsRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::GetStatsResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }
}

#[derive(Clone)]
struct CountingFunctionService {
    mode: MockMode,
    calls: Arc<AtomicUsize>,
}

#[tonic::async_trait]
impl spear_next::proto::spearlet::function_service_server::FunctionService
    for CountingFunctionService
{
    async fn invoke_function(
        &self,
        request: Request<InvokeFunctionRequest>,
    ) -> Result<Response<InvokeFunctionResponse>, Status> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let req = request.into_inner();
        match self.mode {
            MockMode::Unavailable => Err(Status::unavailable("mock unavailable")),
            MockMode::InvalidArgument => Err(Status::invalid_argument("mock invalid")),
            MockMode::Success => Ok(Response::new(InvokeFunctionResponse {
                success: true,
                message: "ok".to_string(),
                execution_id: req.execution_id.unwrap_or_default(),
                task_id: req.task_id,
                ..Default::default()
            })),
        }
    }

    async fn get_execution_status(
        &self,
        _request: Request<spear_next::proto::spearlet::GetExecutionStatusRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::GetExecutionStatusResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn cancel_execution(
        &self,
        _request: Request<spear_next::proto::spearlet::CancelExecutionRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::CancelExecutionResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    type StreamFunctionStream = <MockFunctionService as spear_next::proto::spearlet::function_service_server::FunctionService>::StreamFunctionStream;

    async fn stream_function(
        &self,
        _request: Request<InvokeFunctionRequest>,
    ) -> Result<Response<Self::StreamFunctionStream>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn list_tasks(
        &self,
        _request: Request<spear_next::proto::spearlet::ListTasksRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::ListTasksResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn get_task(
        &self,
        _request: Request<spear_next::proto::spearlet::GetTaskRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::GetTaskResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn delete_task(
        &self,
        _request: Request<spear_next::proto::spearlet::DeleteTaskRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::DeleteTaskResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn list_executions(
        &self,
        _request: Request<spear_next::proto::spearlet::ListExecutionsRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::ListExecutionsResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn get_health(
        &self,
        _request: Request<spear_next::proto::spearlet::GetHealthRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::GetHealthResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn get_stats(
        &self,
        _request: Request<spear_next::proto::spearlet::GetStatsRequest>,
    ) -> Result<Response<spear_next::proto::spearlet::GetStatsResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }
}

async fn start_counting_spearlet(
    mode: MockMode,
) -> (tokio::task::JoinHandle<()>, u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let calls = Arc::new(AtomicUsize::new(0));
    let svc = CountingFunctionService {
        mode,
        calls: calls.clone(),
    };
    let handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(FunctionServiceServer::new(svc))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    (handle, port, calls)
}

async fn start_mock_spearlet(mode: MockMode) -> (tokio::task::JoinHandle<()>, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let svc = MockFunctionService { mode };
    let handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(FunctionServiceServer::new(svc))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    (handle, port)
}

async fn start_sms_grpc() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let sms_service =
        SmsServiceImpl::with_storage_config(&spear_next::config::base::StorageConfig {
            backend: "memory".to_string(),
            ..Default::default()
        })
        .await;

    let handle = tokio::spawn(async move {
        let svc_node = sms_service.clone();
        let svc_task = sms_service.clone();
        tonic::transport::Server::builder()
            .add_service(NodeServiceServer::new(svc_node))
            .add_service(TaskServiceServer::new(svc_task))
            .add_service(PlacementServiceServer::new(sms_service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    (handle, format!("http://{}", addr))
}

#[tokio::test]
async fn test_admin_execution_spillback_and_feedback_affects_next_placement() {
    let (_h1, port1) = start_mock_spearlet(MockMode::Unavailable).await;
    let (_h2, port2) = start_mock_spearlet(MockMode::Success).await;

    let (sms_handle, sms_url) = start_sms_grpc().await;

    let mut node_client = NodeServiceClient::connect(sms_url.clone()).await.unwrap();
    let task_client = TaskServiceClient::connect(sms_url.clone()).await.unwrap();
    let mut placement_client = PlacementServiceClient::connect(sms_url.clone())
        .await
        .unwrap();
    let mcp_registry_client =
        spear_next::proto::sms::mcp_registry_service_client::McpRegistryServiceClient::connect(
            sms_url.clone(),
        )
        .await
        .unwrap();

    let node1_uuid = Uuid::new_v4().to_string();
    let node2_uuid = Uuid::new_v4().to_string();

    let now = chrono::Utc::now().timestamp();
    let node1 = Node {
        uuid: node1_uuid.clone(),
        ip_address: "127.0.0.1".to_string(),
        port: port1 as i32,
        status: "online".to_string(),
        last_heartbeat: now,
        registered_at: now,
        metadata: Default::default(),
    };
    let node2 = Node {
        uuid: node2_uuid.clone(),
        ip_address: "127.0.0.1".to_string(),
        port: port2 as i32,
        status: "online".to_string(),
        last_heartbeat: now,
        registered_at: now,
        metadata: Default::default(),
    };

    node_client
        .register_node(RegisterNodeRequest { node: Some(node1) })
        .await
        .unwrap();
    node_client
        .register_node(RegisterNodeRequest { node: Some(node2) })
        .await
        .unwrap();

    node_client
        .update_node_resource(spear_next::proto::sms::UpdateNodeResourceRequest {
            resource: Some(spear_next::proto::sms::NodeResource {
                node_uuid: node1_uuid.clone(),
                cpu_usage_percent: 1.0,
                memory_usage_percent: 1.0,
                total_memory_bytes: 1,
                used_memory_bytes: 1,
                available_memory_bytes: 1,
                disk_usage_percent: 1.0,
                total_disk_bytes: 1,
                used_disk_bytes: 1,
                network_rx_bytes_per_sec: 0,
                network_tx_bytes_per_sec: 0,
                load_average_1m: 0.1,
                load_average_5m: 0.1,
                load_average_15m: 0.1,
                updated_at: chrono::Utc::now().timestamp(),
                resource_metadata: Default::default(),
            }),
        })
        .await
        .unwrap();
    node_client
        .update_node_resource(spear_next::proto::sms::UpdateNodeResourceRequest {
            resource: Some(spear_next::proto::sms::NodeResource {
                node_uuid: node2_uuid.clone(),
                cpu_usage_percent: 99.0,
                memory_usage_percent: 99.0,
                total_memory_bytes: 1,
                used_memory_bytes: 1,
                available_memory_bytes: 1,
                disk_usage_percent: 99.0,
                total_disk_bytes: 1,
                used_disk_bytes: 1,
                network_rx_bytes_per_sec: 0,
                network_tx_bytes_per_sec: 0,
                load_average_1m: 10.0,
                load_average_5m: 10.0,
                load_average_15m: 10.0,
                updated_at: chrono::Utc::now().timestamp(),
                resource_metadata: Default::default(),
            }),
        })
        .await
        .unwrap();

    let state = GatewayState {
        node_client,
        task_client,
        placement_client: placement_client.clone(),
        mcp_registry_client,
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    let resp = server
        .post("/admin/api/executions")
        .json(&serde_json::json!({
            "task_id": "t-1",
            "request_id": Uuid::new_v4().to_string(),
            "execution_id": Uuid::new_v4().to_string(),
            "max_candidates": 2,
            "execution_mode": "sync"
        }))
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["success"], true);
    assert_eq!(body["node_uuid"], node2_uuid);

    let placed = placement_client
        .place_invocation(spear_next::proto::sms::PlaceInvocationRequest {
            request_id: Uuid::new_v4().to_string(),
            task_id: "t-1".to_string(),
            max_candidates: 2,
            labels: Default::default(),
        })
        .await
        .unwrap()
        .into_inner();
    assert!(!placed.candidates.iter().any(|c| c.node_uuid == node1_uuid));
    assert!(placed.candidates.iter().any(|c| c.node_uuid == node2_uuid));

    sms_handle.abort();
}

#[tokio::test]
async fn test_admin_does_not_spillback_on_invalid_argument() {
    let (_h1, port1, calls1) = start_counting_spearlet(MockMode::InvalidArgument).await;
    let (_h2, port2, calls2) = start_counting_spearlet(MockMode::Success).await;

    let (sms_handle, sms_url) = start_sms_grpc().await;

    let mut node_client = NodeServiceClient::connect(sms_url.clone()).await.unwrap();
    let task_client = TaskServiceClient::connect(sms_url.clone()).await.unwrap();
    let placement_client = PlacementServiceClient::connect(sms_url.clone())
        .await
        .unwrap();
    let mcp_registry_client =
        spear_next::proto::sms::mcp_registry_service_client::McpRegistryServiceClient::connect(
            sms_url.clone(),
        )
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    let node1_uuid = Uuid::new_v4().to_string();
    let node2_uuid = Uuid::new_v4().to_string();
    node_client
        .register_node(RegisterNodeRequest {
            node: Some(Node {
                uuid: node1_uuid.clone(),
                ip_address: "127.0.0.1".to_string(),
                port: port1 as i32,
                status: "online".to_string(),
                last_heartbeat: now,
                registered_at: now,
                metadata: Default::default(),
            }),
        })
        .await
        .unwrap();
    node_client
        .register_node(RegisterNodeRequest {
            node: Some(Node {
                uuid: node2_uuid.clone(),
                ip_address: "127.0.0.1".to_string(),
                port: port2 as i32,
                status: "online".to_string(),
                last_heartbeat: now,
                registered_at: now,
                metadata: Default::default(),
            }),
        })
        .await
        .unwrap();

    node_client
        .update_node_resource(spear_next::proto::sms::UpdateNodeResourceRequest {
            resource: Some(spear_next::proto::sms::NodeResource {
                node_uuid: node1_uuid.clone(),
                cpu_usage_percent: 1.0,
                memory_usage_percent: 1.0,
                total_memory_bytes: 1,
                used_memory_bytes: 1,
                available_memory_bytes: 1,
                disk_usage_percent: 1.0,
                total_disk_bytes: 1,
                used_disk_bytes: 1,
                network_rx_bytes_per_sec: 0,
                network_tx_bytes_per_sec: 0,
                load_average_1m: 0.1,
                load_average_5m: 0.1,
                load_average_15m: 0.1,
                updated_at: chrono::Utc::now().timestamp(),
                resource_metadata: Default::default(),
            }),
        })
        .await
        .unwrap();
    node_client
        .update_node_resource(spear_next::proto::sms::UpdateNodeResourceRequest {
            resource: Some(spear_next::proto::sms::NodeResource {
                node_uuid: node2_uuid.clone(),
                cpu_usage_percent: 99.0,
                memory_usage_percent: 99.0,
                total_memory_bytes: 1,
                used_memory_bytes: 1,
                available_memory_bytes: 1,
                disk_usage_percent: 99.0,
                total_disk_bytes: 1,
                used_disk_bytes: 1,
                network_rx_bytes_per_sec: 0,
                network_tx_bytes_per_sec: 0,
                load_average_1m: 10.0,
                load_average_5m: 10.0,
                load_average_15m: 10.0,
                updated_at: chrono::Utc::now().timestamp(),
                resource_metadata: Default::default(),
            }),
        })
        .await
        .unwrap();

    let state = GatewayState {
        node_client,
        task_client,
        placement_client,
        mcp_registry_client,
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    let resp = server
        .post("/admin/api/executions")
        .json(&serde_json::json!({
            "task_id": "t-1",
            "request_id": Uuid::new_v4().to_string(),
            "execution_id": Uuid::new_v4().to_string(),
            "max_candidates": 2,
            "execution_mode": "sync"
        }))
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["success"], false);
    assert!(calls1.load(Ordering::SeqCst) >= 1);
    assert_eq!(calls2.load(Ordering::SeqCst), 0);

    sms_handle.abort();
}
