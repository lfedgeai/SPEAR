use axum_test::TestServer;
use spear_next::proto::sms::{
    node_service_server::NodeServiceServer, placement_service_server::PlacementServiceServer,
    task_service_server::TaskServiceServer, Node, NodeResource, RegisterNodeRequest,
    UpdateNodeResourceRequest,
};
use spear_next::sms::gateway::GatewayState;
use spear_next::sms::service::SmsServiceImpl;
use spear_next::sms::web_admin::create_admin_router;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

async fn start_test_grpc() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = SmsServiceImpl::new(
        std::sync::Arc::new(tokio::sync::RwLock::new(
            spear_next::sms::services::NodeService::new(),
        )),
        std::sync::Arc::new(spear_next::sms::services::ResourceService::new()),
        std::sync::Arc::new(spear_next::sms::config::SmsConfig::default()),
    )
    .await;
    let handle = tokio::spawn(async move {
        Server::builder()
            .add_service(NodeServiceServer::new(service.clone()))
            .add_service(TaskServiceServer::new(service.clone()))
            .add_service(PlacementServiceServer::new(service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    (handle, format!("http://{}", addr))
}

#[tokio::test]
async fn test_admin_list_nodes_empty() {
    let (_h, grpc_url) = start_test_grpc().await;
    let node_client =
        spear_next::proto::sms::node_service_client::NodeServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let task_client =
        spear_next::proto::sms::task_service_client::TaskServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let placement_client =
        spear_next::proto::sms::placement_service_client::PlacementServiceClient::connect(
            grpc_url.clone(),
        )
        .await
        .unwrap();
    let state = GatewayState {
        node_client,
        task_client,
        placement_client,
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    let resp = server.get("/admin/api/nodes").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["total_count"].as_i64().unwrap(), 0);
}

#[tokio::test]
async fn test_admin_list_nodes_filter_and_sort() {
    let (_h, grpc_url) = start_test_grpc().await;
    let mut node_client =
        spear_next::proto::sms::node_service_client::NodeServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    // Register two nodes with different heartbeats
    let now = chrono::Utc::now().timestamp();
    let n1 = Node {
        uuid: "n1".into(),
        ip_address: "10.0.0.1".into(),
        port: 8001,
        status: "online".into(),
        last_heartbeat: now - 10,
        registered_at: now - 100,
        metadata: Default::default(),
    };
    let n2 = Node {
        uuid: "n2".into(),
        ip_address: "10.0.0.2".into(),
        port: 8002,
        status: "online".into(),
        last_heartbeat: now - 1,
        registered_at: now - 90,
        metadata: Default::default(),
    };
    node_client
        .register_node(RegisterNodeRequest { node: Some(n1) })
        .await
        .unwrap();
    node_client
        .register_node(RegisterNodeRequest { node: Some(n2) })
        .await
        .unwrap();

    let task_client =
        spear_next::proto::sms::task_service_client::TaskServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let placement_client =
        spear_next::proto::sms::placement_service_client::PlacementServiceClient::connect(
            grpc_url.clone(),
        )
        .await
        .unwrap();
    let state = GatewayState {
        node_client,
        task_client,
        placement_client,
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    // Sort by last_heartbeat desc (use sort_by & order to avoid colon issues)
    let resp = server
        .get("/admin/api/nodes")
        .add_query_params(serde_json::json!({"sort_by":"last_heartbeat","order":"desc"}))
        .await;
    resp.assert_status_ok();
    let list: serde_json::Value = resp.json();
    assert_eq!(list["nodes"][0]["uuid"], "n2");

    // Filter by q
    let resp2 = server
        .get("/admin/api/nodes")
        .add_query_params(serde_json::json!({"q":"10.0.0.1"}))
        .await;
    resp2.assert_status_ok();
    let list2: serde_json::Value = resp2.json();
    assert_eq!(list2["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(list2["nodes"][0]["uuid"], "n1");
}

#[tokio::test]
async fn test_admin_stats() {
    let (_h, grpc_url) = start_test_grpc().await;
    let mut node_client =
        spear_next::proto::sms::node_service_client::NodeServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let now = chrono::Utc::now().timestamp();
    let n1 = Node {
        uuid: "s1".into(),
        ip_address: "10.0.0.1".into(),
        port: 8001,
        status: "online".into(),
        last_heartbeat: now,
        registered_at: now - 100,
        metadata: Default::default(),
    };
    let n2 = Node {
        uuid: "s2".into(),
        ip_address: "10.0.0.2".into(),
        port: 8002,
        status: "offline".into(),
        last_heartbeat: now - 600,
        registered_at: now - 1000,
        metadata: Default::default(),
    };
    node_client
        .register_node(RegisterNodeRequest { node: Some(n1) })
        .await
        .unwrap();
    node_client
        .register_node(RegisterNodeRequest { node: Some(n2) })
        .await
        .unwrap();

    let task_client =
        spear_next::proto::sms::task_service_client::TaskServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let placement_client =
        spear_next::proto::sms::placement_service_client::PlacementServiceClient::connect(
            grpc_url.clone(),
        )
        .await
        .unwrap();
    let state = GatewayState {
        node_client,
        task_client,
        placement_client,
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    let resp = server.get("/admin/api/stats").await;
    resp.assert_status_ok();
    let stats: serde_json::Value = resp.json();
    assert_eq!(stats["total_count"].as_i64().unwrap(), 2);
    assert_eq!(stats["online_count"].as_i64().unwrap(), 1);
    assert_eq!(stats["offline_count"].as_i64().unwrap(), 1);
    assert!(stats["recent_60s_count"].as_i64().unwrap() >= 1);
}

#[tokio::test]
async fn test_admin_nodes_stream() {
    let (_h, grpc_url) = start_test_grpc().await;
    let node_client =
        spear_next::proto::sms::node_service_client::NodeServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let task_client =
        spear_next::proto::sms::task_service_client::TaskServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let placement_client =
        spear_next::proto::sms::placement_service_client::PlacementServiceClient::connect(grpc_url)
            .await
            .unwrap();
    let state = GatewayState {
        node_client,
        task_client,
        placement_client,
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    let resp = server.get("/admin/api/nodes/stream?once=true").await;
    resp.assert_status_ok();
}

#[tokio::test]
async fn test_admin_node_detail_includes_resource() {
    let (_h, grpc_url) = start_test_grpc().await;
    let mut node_client =
        spear_next::proto::sms::node_service_client::NodeServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let uuid = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let n = Node {
        uuid: uuid.clone(),
        ip_address: "10.0.0.9".into(),
        port: 8009,
        status: "online".into(),
        last_heartbeat: now,
        registered_at: now - 10,
        metadata: Default::default(),
    };
    node_client
        .register_node(RegisterNodeRequest { node: Some(n) })
        .await
        .unwrap();

    node_client
        .update_node_resource(UpdateNodeResourceRequest {
            resource: Some(NodeResource {
                node_uuid: uuid.clone(),
                cpu_usage_percent: 12.0,
                memory_usage_percent: 34.0,
                total_memory_bytes: 100,
                used_memory_bytes: 50,
                available_memory_bytes: 50,
                disk_usage_percent: 56.0,
                total_disk_bytes: 1000,
                used_disk_bytes: 123,
                network_rx_bytes_per_sec: 0,
                network_tx_bytes_per_sec: 0,
                load_average_1m: 1.0,
                load_average_5m: 2.0,
                load_average_15m: 3.0,
                updated_at: now,
                resource_metadata: Default::default(),
            }),
        })
        .await
        .unwrap();

    let task_client =
        spear_next::proto::sms::task_service_client::TaskServiceClient::connect(grpc_url.clone())
            .await
            .unwrap();
    let placement_client =
        spear_next::proto::sms::placement_service_client::PlacementServiceClient::connect(
            grpc_url.clone(),
        )
        .await
        .unwrap();
    let state = GatewayState {
        node_client,
        task_client,
        placement_client,
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    let resp = server.get(&format!("/admin/api/nodes/{}", uuid)).await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert!(body["found"].as_bool().unwrap_or(false));
    assert_eq!(body["node"]["uuid"], uuid);
    assert_eq!(body["resource"]["cpu_usage_percent"], 12.0);
    assert_eq!(body["resource"]["memory_usage_percent"], 34.0);
    assert_eq!(body["resource"]["disk_usage_percent"], 56.0);
}
