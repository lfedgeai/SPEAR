use crate::proto::sms::{
    node_service_server::NodeService as NodeServiceTrait,
    placement_service_server::PlacementService as PlacementServiceTrait, InvocationOutcomeClass,
    Node, NodeResource, PlaceInvocationRequest, ReportInvocationOutcomeRequest,
    UpdateNodeResourceRequest,
};
use crate::sms::service::SmsServiceImpl;
use tonic::Request;
use uuid::Uuid;

#[tokio::test]
async fn test_penalty_snapshot_updates_and_blocks_node_immediately() {
    let service = SmsServiceImpl::with_storage_config(&crate::config::base::StorageConfig {
        backend: "memory".to_string(),
        ..Default::default()
    })
    .await;

    let now = chrono::Utc::now().timestamp();
    let node_good = Uuid::new_v4().to_string();
    let node_bad = Uuid::new_v4().to_string();

    service
        .register_node(Request::new(crate::proto::sms::RegisterNodeRequest {
            node: Some(Node {
                uuid: node_good.clone(),
                ip_address: "127.0.0.1".to_string(),
                port: 10001,
                status: "online".to_string(),
                last_heartbeat: now,
                registered_at: now,
                metadata: Default::default(),
            }),
        }))
        .await
        .unwrap();
    service
        .register_node(Request::new(crate::proto::sms::RegisterNodeRequest {
            node: Some(Node {
                uuid: node_bad.clone(),
                ip_address: "127.0.0.1".to_string(),
                port: 10002,
                status: "online".to_string(),
                last_heartbeat: now,
                registered_at: now,
                metadata: Default::default(),
            }),
        }))
        .await
        .unwrap();

    service
        .update_node_resource(Request::new(UpdateNodeResourceRequest {
            resource: Some(NodeResource {
                node_uuid: node_good.clone(),
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
        }))
        .await
        .unwrap();
    service
        .update_node_resource(Request::new(UpdateNodeResourceRequest {
            resource: Some(NodeResource {
                node_uuid: node_bad.clone(),
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
        }))
        .await
        .unwrap();

    let placed1 = service
        .place_invocation(Request::new(PlaceInvocationRequest {
            request_id: Uuid::new_v4().to_string(),
            task_id: "t".to_string(),
            max_candidates: 10,
            labels: Default::default(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(placed1.candidates.iter().any(|c| c.node_uuid == node_good));
    assert!(placed1.candidates.iter().any(|c| c.node_uuid == node_bad));

    service
        .report_invocation_outcome(Request::new(ReportInvocationOutcomeRequest {
            decision_id: placed1.decision_id.clone(),
            request_id: Uuid::new_v4().to_string(),
            task_id: "t".to_string(),
            node_uuid: node_bad.clone(),
            outcome_class: InvocationOutcomeClass::Unavailable as i32,
            error_message: "x".to_string(),
        }))
        .await
        .unwrap();

    let snap = service.test_get_node_penalty_snapshot(&node_bad).unwrap();
    assert!(snap.0 >= 1);
    assert!(snap.1 > chrono::Utc::now().timestamp());

    let placed2 = service
        .place_invocation(Request::new(PlaceInvocationRequest {
            request_id: Uuid::new_v4().to_string(),
            task_id: "t".to_string(),
            max_candidates: 10,
            labels: Default::default(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(!placed2.candidates.iter().any(|c| c.node_uuid == node_bad));
    assert!(placed2.candidates.iter().any(|c| c.node_uuid == node_good));
}
