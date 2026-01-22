use axum_test::TestServer;
use spear_next::sms::gateway::GatewayState;
use spear_next::sms::web_admin::create_admin_router;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_admin_tasks_schema_contains_expected_fields() {
    use spear_next::proto::sms::{
        node_service_server::NodeServiceServer, placement_service_server::PlacementServiceServer,
        task_service_server::TaskServiceServer,
    };
    use spear_next::sms::service::SmsServiceImpl;
    use tokio::net::TcpListener;
    use tonic::transport::Server;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let sms_service =
        SmsServiceImpl::with_storage_config(&spear_next::config::base::StorageConfig {
            backend: "memory".to_string(),
            ..Default::default()
        })
        .await;
    let handle = tokio::spawn(async move {
        let sms_service_node = sms_service.clone();
        let sms_service_task = sms_service.clone();
        Server::builder()
            .add_service(NodeServiceServer::new(sms_service_node))
            .add_service(TaskServiceServer::new(sms_service_task))
            .add_service(PlacementServiceServer::new(sms_service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let channel = tonic::transport::Channel::from_shared(format!("http://{}", addr))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let state = GatewayState {
        node_client: spear_next::proto::sms::node_service_client::NodeServiceClient::new(
            channel.clone(),
        ),
        task_client: spear_next::proto::sms::task_service_client::TaskServiceClient::new(
            channel.clone(),
        ),
        placement_client:
            spear_next::proto::sms::placement_service_client::PlacementServiceClient::new(
                channel.clone(),
            ),
        mcp_registry_client:
            spear_next::proto::sms::mcp_registry_service_client::McpRegistryServiceClient::new(
                channel.clone(),
            ),
        backend_registry_client:
            spear_next::proto::sms::backend_registry_service_client::BackendRegistryServiceClient::new(
                channel.clone(),
            ),
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    let create_body = serde_json::json!({
        "name": "task-a",
        "description": "d",
        "priority": "normal",
        "node_uuid": "node-1",
        "endpoint": "http://localhost/task",
        "version": "v1",
        "capabilities": ["c"]
    });
    let resp = server.post("/admin/api/tasks").json(&create_body).await;
    resp.assert_status_ok();

    let resp = server.get("/admin/api/tasks").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    let t = body["tasks"].as_array().unwrap()[0].clone();
    let obj = t.as_object().unwrap();
    for k in [
        "task_id",
        "name",
        "status",
        "priority",
        "node_uuid",
        "endpoint",
        "version",
        "registered_at",
        "last_heartbeat",
    ] {
        assert!(obj.contains_key(k));
    }

    handle.abort();
}
