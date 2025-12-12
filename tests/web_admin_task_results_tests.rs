use axum_test::TestServer;
use spear_next::sms::gateway::GatewayState;
use spear_next::sms::web_admin::create_admin_router;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_admin_tasks_include_result_fields() {
    // Start a test gRPC server
    use spear_next::proto::sms::{
        node_service_client::NodeServiceClient, task_service_client::TaskServiceClient,
    };
    use spear_next::proto::sms::{
        node_service_server::NodeServiceServer, task_service_server::TaskServiceServer,
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
        Server::builder()
            .add_service(NodeServiceServer::new(sms_service.clone()))
            .add_service(TaskServiceServer::new(sms_service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    // Connect clients
    let channel = tonic::transport::Channel::from_shared(format!("http://{}", addr))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let state = GatewayState {
        node_client: NodeServiceClient::new(channel.clone()),
        task_client: TaskServiceClient::new(channel.clone()),
        cancel_token: CancellationToken::new(),
        max_upload_bytes: 64 * 1024 * 1024,
    };
    let app = create_admin_router(state);
    let server = TestServer::new(app.into_make_service()).unwrap();

    // Register one task via admin API
    let create_body = serde_json::json!({
        "name": "admin-task",
        "description": "d",
        "priority": "normal",
        "node_uuid": "node-1",
        "endpoint": "http://localhost/task",
        "version": "v1",
        "capabilities": ["c"]
    });
    let resp = server.post("/admin/api/tasks").json(&create_body).await;
    resp.assert_status_ok();

    // List tasks and verify result fields present
    let resp = server.get("/admin/api/tasks").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    let arr = body.get("tasks").and_then(|v| v.as_array()).unwrap();
    assert!(!arr.is_empty());
    let t = &arr[0];
    assert!(t.get("result_uris").is_some());
    assert!(t.get("last_result_uri").is_some());
    assert!(t.get("last_result_status").is_some());
    assert!(t.get("last_completed_at").is_some());
    assert!(t.get("last_result_metadata").is_some());

    handle.abort();
}
