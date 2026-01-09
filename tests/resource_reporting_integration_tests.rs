use std::sync::Arc;

use spear_next::proto::sms::{
    node_service_client::NodeServiceClient, node_service_server::NodeServiceServer,
    placement_service_server::PlacementServiceServer, task_service_server::TaskServiceServer,
    GetNodeWithResourceRequest,
};
use spear_next::sms::service::SmsServiceImpl;
use spear_next::spearlet::config::SpearletConfig;
use spear_next::spearlet::registration::RegistrationService;
use tokio::net::TcpListener;
use tonic::transport::Server;

async fn start_sms_grpc() -> (tokio::task::JoinHandle<()>, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = SmsServiceImpl::new(
        Arc::new(tokio::sync::RwLock::new(
            spear_next::sms::services::NodeService::new(),
        )),
        Arc::new(spear_next::sms::services::ResourceService::new()),
        Arc::new(spear_next::sms::config::SmsConfig::default()),
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
    (handle, addr)
}

#[tokio::test(start_paused = true)]
async fn test_spearlet_reports_node_resource_on_heartbeat() {
    let (sms_handle, addr) = start_sms_grpc().await;
    let node_name = uuid::Uuid::new_v4().to_string();

    let mut cfg = SpearletConfig::default();
    cfg.sms_grpc_addr = addr.to_string();
    cfg.auto_register = true;
    cfg.heartbeat_interval = 1;
    cfg.reconnect_total_timeout_ms = 300_000;
    cfg.node_name = node_name.clone();

    let reg = RegistrationService::new(Arc::new(cfg.clone()));
    reg.start().await.unwrap();

    let mut client = NodeServiceClient::connect(format!("http://{}", addr))
        .await
        .unwrap();
    let mut last = None;
    for _ in 0..5 {
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        let resp = client
            .get_node_with_resource(GetNodeWithResourceRequest {
                uuid: node_name.clone(),
            })
            .await
            .unwrap()
            .into_inner();
        if resp.node.is_some() && resp.resource.is_some() {
            last = Some(resp);
            break;
        }
        last = Some(resp);
    }
    let last = last.unwrap();
    assert!(last.node.is_some());
    assert!(last.resource.is_some());

    reg.shutdown();
    sms_handle.abort();
}
