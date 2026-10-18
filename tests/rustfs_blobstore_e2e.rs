#![cfg(feature = "sms-rustfs")]

// E2E test using a real RustFS container for BlobStore validation
// 端到端测试：使用真实 RustFS 容器验证 BlobStore 行为

use spear_next::sms::blob_store::{build_blob_store, BlobStoreError, PutBlobRequest};
use spear_next::sms::config::{ObjectStorageBackend, SmsConfig};
use std::process::Command;
use std::time::{Duration, Instant};
use testcontainers::{clients, GenericImage, RunnableImage};
use tokio::time::sleep;
use uuid::Uuid;

/// Check whether Docker is available before trying to start containers.
/// 在尝试启动容器前检查 Docker 是否可用。
fn docker_available() -> bool {
    Command::new("docker")
        .arg("info")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Build an SMS config that targets a RustFS endpoint exposed by testcontainers.
/// 构建指向 testcontainers 暴露 RustFS 端点的 SMS 配置。
fn rustfs_test_config(endpoint: String, bucket: String, root: String) -> SmsConfig {
    let mut config = SmsConfig::default();
    config.object_storage.backend = ObjectStorageBackend::Rustfs;
    config.object_storage.rustfs.endpoint = endpoint;
    config.object_storage.rustfs.bucket = bucket;
    config.object_storage.rustfs.root = root;
    config.object_storage.rustfs.region = "us-east-1".to_string();
    config.object_storage.rustfs.access_key = "rustfsadmin".to_string();
    config.object_storage.rustfs.secret_key = "rustfsadmin".to_string();
    config.object_storage.rustfs.path_style = true;
    config.object_storage.rustfs.create_bucket_if_missing = true;
    config
}

/// Wait until RustFS can be reached through the full `build_blob_store` path.
/// 等待 RustFS 通过完整 `build_blob_store` 路径可访问。
async fn wait_for_rustfs(
    config: &SmsConfig,
    deadline: Duration,
) -> anyhow::Result<spear_next::sms::blob_store::SharedBlobStore> {
    let start = Instant::now();
    loop {
        match build_blob_store(config).await {
            Ok(store) => match store.status().await {
                Ok(_) => return Ok(store),
                Err(_) if start.elapsed() < deadline => {}
                Err(error) => return Err(anyhow::anyhow!("RustFS status failed: {error:?}")),
            },
            Err(_) if start.elapsed() < deadline => {}
            Err(error) => return Err(error),
        }

        sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test]
#[ignore]
async fn e2e_rustfs_blob_store_round_trips_and_enforces_lifecycle() {
    if !docker_available() {
        eprintln!("Docker daemon not reachable, skipping RustFS E2E test");
        return;
    }

    let docker = clients::Cli::default();
    let image = GenericImage::new("rustfs/rustfs", "latest")
        .with_exposed_port(9000)
        .with_env_var("RUSTFS_ACCESS_KEY", "rustfsadmin")
        .with_env_var("RUSTFS_SECRET_KEY", "rustfsadmin");
    let container = docker.run(RunnableImage::from((image, vec!["/data".to_string()])));
    let rustfs_port = container.get_host_port_ipv4(9000);

    let config = rustfs_test_config(
        format!("http://127.0.0.1:{rustfs_port}"),
        format!("sms-rustfs-e2e-{}", Uuid::new_v4().simple()),
        format!("tenant-{}", Uuid::new_v4().simple()),
    );
    let store = wait_for_rustfs(&config, Duration::from_secs(40))
        .await
        .expect("RustFS should become ready");

    let created = store
        .put_blob(PutBlobRequest {
            name: Some("artifact.bin".to_string()),
            content_type: Some("application/octet-stream".to_string()),
            bytes: vec![1, 2, 3, 4, 5],
        })
        .await
        .expect("put_blob should succeed");
    assert_eq!(created.name.as_deref(), Some("artifact.bin"));
    assert_eq!(created.size, 5);

    let content = store
        .get_blob(&created.key)
        .await
        .expect("get_blob should succeed");
    assert_eq!(content.bytes, vec![1, 2, 3, 4, 5]);
    assert_eq!(
        content.content_type.as_deref(),
        Some("application/octet-stream")
    );

    let page = store
        .list_blobs(Some("artifact"), Some(10), Some(0))
        .await
        .expect("list_blobs should succeed");
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].key, created.key);

    let retained = store
        .retain_blob(&created.key)
        .await
        .expect("retain_blob should succeed");
    assert_eq!(retained.ref_count, 1);
    assert!(matches!(
        store.delete_blob(&created.key).await,
        Err(BlobStoreError::Conflict("Referenced objects cannot be deleted"))
    ));

    let released = store
        .release_blob(&created.key)
        .await
        .expect("release_blob should succeed");
    assert_eq!(released.ref_count, 0);

    let pinned = store
        .pin_blob(&created.key)
        .await
        .expect("pin_blob should succeed");
    assert!(pinned.pinned);
    assert!(matches!(
        store.delete_blob(&created.key).await,
        Err(BlobStoreError::Conflict("Pinned objects cannot be deleted"))
    ));

    store
        .unpin_blob(&created.key)
        .await
        .expect("unpin_blob should succeed");
    store
        .delete_blob(&created.key)
        .await
        .expect("delete_blob should succeed");
    assert!(matches!(
        store.get_blob_metadata(&created.key).await,
        Err(BlobStoreError::NotFound)
    ));
}
