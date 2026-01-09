use spear_next::proto::sms::{
    node_service_client::NodeServiceClient, node_service_server::NodeServiceServer,
    placement_service_client::PlacementServiceClient,
    placement_service_server::PlacementServiceServer, task_service_server::TaskServiceServer, Node,
    NodeResource, RegisterNodeRequest, UpdateNodeResourceRequest,
};
use spear_next::sms::service::SmsServiceImpl;
use tokio::net::TcpListener;
use tonic::transport::Server;
use uuid::Uuid;

async fn start_sms_grpc_with_service() -> (tokio::task::JoinHandle<()>, String, SmsServiceImpl) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let sms_service =
        SmsServiceImpl::with_storage_config(&spear_next::config::base::StorageConfig {
            backend: "memory".to_string(),
            ..Default::default()
        })
        .await;

    let svc_for_server = sms_service.clone();
    let handle = tokio::spawn(async move {
        let svc_node = svc_for_server.clone();
        let svc_task = svc_for_server.clone();
        Server::builder()
            .add_service(NodeServiceServer::new(svc_node))
            .add_service(TaskServiceServer::new(svc_task))
            .add_service(PlacementServiceServer::new(svc_for_server))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    (handle, format!("http://{}", addr), sms_service)
}

#[tokio::test]
async fn test_placement_filters_offline_and_stale_and_orders_by_score() {
    let (handle, sms_url, _svc) = start_sms_grpc_with_service().await;

    let mut node_client = NodeServiceClient::connect(sms_url.clone()).await.unwrap();
    let mut placement_client = PlacementServiceClient::connect(sms_url.clone())
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    let good_uuid = Uuid::new_v4().to_string();
    let bad_uuid = Uuid::new_v4().to_string();
    let offline_uuid = Uuid::new_v4().to_string();
    let stale_uuid = Uuid::new_v4().to_string();

    node_client
        .register_node(RegisterNodeRequest {
            node: Some(Node {
                uuid: good_uuid.clone(),
                ip_address: "127.0.0.1".to_string(),
                port: 10001,
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
                uuid: bad_uuid.clone(),
                ip_address: "127.0.0.1".to_string(),
                port: 10002,
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
                uuid: offline_uuid.clone(),
                ip_address: "127.0.0.1".to_string(),
                port: 10003,
                status: "offline".to_string(),
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
                uuid: stale_uuid.clone(),
                ip_address: "127.0.0.1".to_string(),
                port: 10004,
                status: "online".to_string(),
                last_heartbeat: now - 10_000,
                registered_at: now,
                metadata: Default::default(),
            }),
        })
        .await
        .unwrap();

    node_client
        .update_node_resource(UpdateNodeResourceRequest {
            resource: Some(NodeResource {
                node_uuid: good_uuid.clone(),
                cpu_usage_percent: 5.0,
                memory_usage_percent: 5.0,
                total_memory_bytes: 1,
                used_memory_bytes: 1,
                available_memory_bytes: 1,
                disk_usage_percent: 5.0,
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
        .update_node_resource(UpdateNodeResourceRequest {
            resource: Some(NodeResource {
                node_uuid: bad_uuid.clone(),
                cpu_usage_percent: 95.0,
                memory_usage_percent: 95.0,
                total_memory_bytes: 1,
                used_memory_bytes: 1,
                available_memory_bytes: 1,
                disk_usage_percent: 95.0,
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

    let resp = placement_client
        .place_invocation(spear_next::proto::sms::PlaceInvocationRequest {
            request_id: Uuid::new_v4().to_string(),
            task_id: "t".to_string(),
            max_candidates: 10,
            labels: Default::default(),
        })
        .await
        .unwrap()
        .into_inner();

    let uuids: Vec<String> = resp
        .candidates
        .iter()
        .map(|c| c.node_uuid.clone())
        .collect();
    assert!(uuids.contains(&good_uuid));
    assert!(uuids.contains(&bad_uuid));
    assert!(!uuids.contains(&offline_uuid));
    assert!(!uuids.contains(&stale_uuid));

    let pos_good = uuids.iter().position(|u| u == &good_uuid).unwrap();
    let pos_bad = uuids.iter().position(|u| u == &bad_uuid).unwrap();
    assert!(pos_good < pos_bad);

    let score_good = resp
        .candidates
        .iter()
        .find(|c| c.node_uuid == good_uuid)
        .unwrap()
        .score;
    let score_bad = resp
        .candidates
        .iter()
        .find(|c| c.node_uuid == bad_uuid)
        .unwrap()
        .score;
    assert!(score_good > score_bad);

    handle.abort();
}
