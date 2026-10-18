use crate::sms::config::{ObjectStorageBackend, SmsConfig};
#[cfg(feature = "sms-rustfs")]
use crate::sms::blob_store_rustfs::rustfs_blob_store;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/// Blob store errors shared by compatibility and future backends.
/// 兼容后端与未来后端共享的 BlobStore 错误定义。
#[derive(Debug, Error)]
pub enum BlobStoreError {
    #[error("blob not found")]
    NotFound,
    #[error("blob lifecycle conflict: {0}")]
    Conflict(&'static str),
    #[error("blob store internal error")]
    Internal,
}

/// Request for storing a new blob.
/// 新 Blob 写入请求。
#[derive(Debug, Clone)]
pub struct PutBlobRequest {
    pub name: Option<String>,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

/// Content returned by a blob download.
/// Blob 下载返回的内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobContent {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

/// Canonical object metadata used by the admin and public object APIs.
/// 管理端与公共对象 API 使用的标准对象元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobMetadata {
    pub key: String,
    pub name: Option<String>,
    pub content_type: Option<String>,
    pub size: u64,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
    pub ref_count: u32,
    pub pinned: bool,
}

/// Summary row returned by object listing APIs.
/// 对象列表接口返回的摘要行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobSummary {
    pub key: String,
    pub name: Option<String>,
    pub size: u64,
    pub updated_at: u64,
    pub ref_count: u32,
    pub pinned: bool,
}

/// Paginated listing page for object summaries.
/// 对象摘要的分页结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobListPage {
    pub items: Vec<BlobSummary>,
    pub total_count: usize,
}

/// Health and implementation metadata for the current blob backend.
/// 当前 Blob 后端的健康状态与实现元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobStoreStatus {
    pub backend: String,
    pub healthy: bool,
    pub message: String,
}

/// Shared blob store handle used by gateway state and object APIs.
/// 网关状态与对象 API 共享的 BlobStore 句柄。
pub type SharedBlobStore = Arc<dyn BlobStore>;

/// Common operations required by SMS object handlers.
/// SMS 对象处理器依赖的通用存储操作。
#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn put_blob(&self, request: PutBlobRequest) -> Result<BlobMetadata, BlobStoreError>;
    async fn get_blob(&self, key: &str) -> Result<BlobContent, BlobStoreError>;
    async fn get_blob_metadata(&self, key: &str) -> Result<BlobMetadata, BlobStoreError>;
    async fn delete_blob(&self, key: &str) -> Result<(), BlobStoreError>;
    async fn list_blobs(
        &self,
        prefix: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<BlobListPage, BlobStoreError>;
    async fn retain_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError>;
    async fn release_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError>;
    async fn pin_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError>;
    async fn unpin_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError>;
    async fn status(&self) -> Result<BlobStoreStatus, BlobStoreError>;
}

/// Build the configured blob store implementation for SMS runtime wiring.
/// 为 SMS 运行时装配配置对应的 BlobStore 实现。
pub async fn build_blob_store(config: &SmsConfig) -> anyhow::Result<SharedBlobStore> {
    match config.object_storage.backend {
        ObjectStorageBackend::LocalFs => Ok(local_fs_blob_store(
            config.object_storage.local_fs.root_dir.clone(),
        )),
        ObjectStorageBackend::Rustfs => {
            #[cfg(feature = "sms-rustfs")]
            {
                return rustfs_blob_store(config.object_storage.rustfs.clone()).await;
            }
            #[cfg(not(feature = "sms-rustfs"))]
            {
                anyhow::bail!(
                    "RustFS support is disabled in this build; rebuild with the `sms-rustfs` feature enabled"
                );
            }
        }
    }
}

/// Build a shared local filesystem blob store handle.
/// 构建共享的本地文件系统 BlobStore 句柄。
pub fn local_fs_blob_store(root_dir: impl Into<String>) -> SharedBlobStore {
    Arc::new(LocalFsBlobStore::new(root_dir))
}

/// File-backed compatibility blob store used by the current SMS implementation.
/// 当前 SMS 实现使用的基于文件系统的兼容 BlobStore。
#[derive(Debug, Clone)]
pub struct LocalFsBlobStore {
    root_dir: String,
}

impl LocalFsBlobStore {
    /// Create a local filesystem blob store rooted at the configured directory.
    /// 基于配置目录创建本地文件系统 BlobStore。
    pub fn new(root_dir: impl Into<String>) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    /// Return the root directory for diagnostics and admin presentation.
    /// 返回用于诊断与管理端展示的根目录。
    pub fn root_dir(&self) -> &str {
        &self.root_dir
    }

    async fn ensure_root_dir(&self) -> Result<(), BlobStoreError> {
        fs::create_dir_all(&self.root_dir)
            .await
            .map_err(|_| BlobStoreError::Internal)
    }

    fn blob_path(&self, key: &str) -> std::path::PathBuf {
        std::path::Path::new(&self.root_dir).join(key)
    }

    fn blob_sidecar_path(&self, key: &str) -> std::path::PathBuf {
        self.blob_path(key).with_extension("json")
    }

    async fn read_blob_sidecar(&self, key: &str) -> Option<serde_json::Value> {
        let bytes = fs::read(self.blob_sidecar_path(key)).await.ok()?;
        serde_json::from_slice::<serde_json::Value>(&bytes).ok()
    }

    async fn load_blob_metadata(
        &self,
        key: &str,
        file_len: Option<u64>,
    ) -> Result<CompatObjectMeta, BlobStoreError> {
        let sidecar = self.read_blob_sidecar(key).await;
        let mut meta = sidecar
            .and_then(|value| serde_json::from_value::<CompatObjectMeta>(value).ok())
            .unwrap_or_else(|| CompatObjectMeta {
                id: key.to_string(),
                name: None,
                content_type: None,
                len: file_len,
                created_at: None,
                updated_at: None,
                ref_count: 0,
                pinned: false,
            });
        meta.id = key.to_string();
        if meta.len.is_none() {
            meta.len = file_len;
        }
        if meta.updated_at.is_none() {
            meta.updated_at = meta.created_at;
        }
        Ok(meta)
    }

    async fn write_blob_metadata(&self, meta: &CompatObjectMeta) -> Result<(), BlobStoreError> {
        let bytes = serde_json::to_vec(meta).map_err(|_| BlobStoreError::Internal)?;
        fs::write(self.blob_sidecar_path(&meta.id), bytes)
            .await
            .map_err(|_| BlobStoreError::Internal)
    }

    async fn update_lifecycle(
        &self,
        key: &str,
        mutate: impl FnOnce(&mut CompatObjectMeta) -> Result<(), &'static str>,
    ) -> Result<BlobMetadata, BlobStoreError> {
        let file_path = self.blob_path(key);
        let file_meta = fs::metadata(&file_path)
            .await
            .map_err(|_| BlobStoreError::NotFound)?;
        let mut meta = self.load_blob_metadata(key, Some(file_meta.len())).await?;
        mutate(&mut meta).map_err(BlobStoreError::Conflict)?;
        meta.updated_at = Some(current_epoch_seconds()?);
        self.write_blob_metadata(&meta).await?;
        Ok(blob_metadata_from_compat(meta))
    }
}

#[async_trait]
impl BlobStore for LocalFsBlobStore {
    async fn put_blob(&self, request: PutBlobRequest) -> Result<BlobMetadata, BlobStoreError> {
        self.ensure_root_dir().await?;
        let key = Uuid::new_v4().to_string();
        let path = self.blob_path(&key);
        let mut file = fs::File::create(&path)
            .await
            .map_err(|_| BlobStoreError::Internal)?;
        file.write_all(&request.bytes)
            .await
            .map_err(|_| BlobStoreError::Internal)?;
        let created_at = current_epoch_seconds()?;
        let meta = CompatObjectMeta {
            id: key,
            name: request.name,
            content_type: request.content_type,
            len: Some(request.bytes.len() as u64),
            created_at: Some(created_at),
            updated_at: Some(created_at),
            ref_count: 0,
            pinned: false,
        };
        self.write_blob_metadata(&meta).await?;
        Ok(blob_metadata_from_compat(meta))
    }

    async fn get_blob(&self, key: &str) -> Result<BlobContent, BlobStoreError> {
        let bytes = fs::read(self.blob_path(key))
            .await
            .map_err(|_| BlobStoreError::NotFound)?;
        let meta = self
            .load_blob_metadata(key, Some(bytes.len() as u64))
            .await
            .unwrap_or_else(|_| CompatObjectMeta {
                id: key.to_string(),
                name: None,
                content_type: None,
                len: Some(bytes.len() as u64),
                created_at: None,
                updated_at: None,
                ref_count: 0,
                pinned: false,
            });
        Ok(BlobContent {
            bytes,
            content_type: meta.content_type,
        })
    }

    async fn get_blob_metadata(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        let file_meta = fs::metadata(self.blob_path(key))
            .await
            .map_err(|_| BlobStoreError::NotFound)?;
        let meta = self.load_blob_metadata(key, Some(file_meta.len())).await?;
        Ok(blob_metadata_from_compat(meta))
    }

    async fn delete_blob(&self, key: &str) -> Result<(), BlobStoreError> {
        let meta = self.get_blob_metadata(key).await?;
        if meta.pinned {
            return Err(BlobStoreError::Conflict("Pinned objects cannot be deleted"));
        }
        if meta.ref_count > 0 {
            return Err(BlobStoreError::Conflict("Referenced objects cannot be deleted"));
        }
        fs::remove_file(self.blob_path(key))
            .await
            .map_err(|_| BlobStoreError::NotFound)?;
        let _ = fs::remove_file(self.blob_sidecar_path(key)).await;
        Ok(())
    }

    async fn list_blobs(
        &self,
        prefix: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<BlobListPage, BlobStoreError> {
        self.ensure_root_dir().await?;
        let mut rd = fs::read_dir(&self.root_dir)
            .await
            .map_err(|_| BlobStoreError::Internal)?;
        let mut items = Vec::new();
        while let Some(ent) = rd
            .next_entry()
            .await
            .map_err(|_| BlobStoreError::Internal)?
        {
            if let Ok(meta) = ent.metadata().await {
                if let Some(file_name) = ent.file_name().to_str() {
                    if file_name.ends_with(".json") {
                        continue;
                    }
                    let modified_at = meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_secs())
                        .unwrap_or(0);
                    let compat = self.load_blob_metadata(file_name, Some(meta.len())).await?;
                    items.push(BlobSummary {
                        key: file_name.to_string(),
                        name: compat.name,
                        size: compat.len.unwrap_or(meta.len()),
                        updated_at: compat.updated_at.or(compat.created_at).unwrap_or(modified_at),
                        ref_count: compat.ref_count,
                        pinned: compat.pinned,
                    });
                }
            }
        }
        items.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

        if let Some(prefix) = prefix.map(str::trim).filter(|value| !value.is_empty()) {
            let needle = prefix.to_lowercase();
            items.retain(|item| {
                item.key.to_lowercase().contains(&needle)
                    || item
                        .name
                        .as_ref()
                        .map(|name| name.to_lowercase().contains(&needle))
                        .unwrap_or(false)
            });
        }

        let total_count = items.len();
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let items = if offset >= items.len() {
            Vec::new()
        } else {
            items.into_iter().skip(offset).take(limit).collect()
        };
        Ok(BlobListPage { items, total_count })
    }

    async fn retain_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        self.update_lifecycle(key, |meta| {
            meta.ref_count = meta.ref_count.saturating_add(1);
            Ok(())
        })
        .await
    }

    async fn release_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        self.update_lifecycle(key, |meta| {
            if meta.ref_count == 0 {
                return Err("Object reference count is already zero");
            }
            meta.ref_count -= 1;
            Ok(())
        })
        .await
    }

    async fn pin_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        self.update_lifecycle(key, |meta| {
            meta.pinned = true;
            Ok(())
        })
        .await
    }

    async fn unpin_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        self.update_lifecycle(key, |meta| {
            meta.pinned = false;
            Ok(())
        })
        .await
    }

    async fn status(&self) -> Result<BlobStoreStatus, BlobStoreError> {
        self.ensure_root_dir().await?;
        Ok(BlobStoreStatus {
            backend: "local-fs-blobstore".to_string(),
            healthy: true,
            message: format!(
                "Using LocalFsBlobStore compatibility backend rooted at {}",
                self.root_dir()
            ),
        })
    }
}

/// Compatibility object metadata persisted next to the blob body.
/// 与 Blob 内容并存的兼容对象元数据。
#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompatObjectMeta {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    len: Option<u64>,
    #[serde(default)]
    created_at: Option<u64>,
    #[serde(default)]
    updated_at: Option<u64>,
    #[serde(default)]
    ref_count: u32,
    #[serde(default)]
    pinned: bool,
}

fn blob_metadata_from_compat(meta: CompatObjectMeta) -> BlobMetadata {
    BlobMetadata {
        size: meta.len.unwrap_or(0),
        key: meta.id,
        name: meta.name,
        content_type: meta.content_type,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
        ref_count: meta.ref_count,
        pinned: meta.pinned,
    }
}

fn current_epoch_seconds() -> Result<u64, BlobStoreError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| BlobStoreError::Internal)
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> LocalFsBlobStore {
        LocalFsBlobStore::new(
            std::env::temp_dir()
                .join(format!("spear-local-fs-blobstore-{}", Uuid::new_v4()))
                .to_string_lossy()
                .to_string(),
        )
    }

    #[tokio::test]
    async fn local_store_round_trips_blob_content_and_metadata() {
        let store = temp_store();
        let created = store
            .put_blob(PutBlobRequest {
                name: Some("demo.wasm".to_string()),
                content_type: Some("application/wasm".to_string()),
                bytes: vec![1, 2, 3],
            })
            .await
            .expect("put should succeed");

        let content = store
            .get_blob(&created.key)
            .await
            .expect("get should succeed");
        let meta = store
            .get_blob_metadata(&created.key)
            .await
            .expect("meta should succeed");

        assert_eq!(content.bytes, vec![1, 2, 3]);
        assert_eq!(content.content_type.as_deref(), Some("application/wasm"));
        assert_eq!(meta.name.as_deref(), Some("demo.wasm"));
        assert_eq!(meta.size, 3);
    }

    #[tokio::test]
    async fn local_store_lifecycle_changes_gate_delete() {
        let store = temp_store();
        let created = store
            .put_blob(PutBlobRequest {
                name: Some("demo.bin".to_string()),
                content_type: Some("application/octet-stream".to_string()),
                bytes: vec![9; 8],
            })
            .await
            .expect("put should succeed");

        let retained = store
            .retain_blob(&created.key)
            .await
            .expect("retain should succeed");
        assert_eq!(retained.ref_count, 1);
        assert!(matches!(
            store.delete_blob(&created.key).await,
            Err(BlobStoreError::Conflict("Referenced objects cannot be deleted"))
        ));

        let released = store
            .release_blob(&created.key)
            .await
            .expect("release should succeed");
        assert_eq!(released.ref_count, 0);

        let pinned = store
            .pin_blob(&created.key)
            .await
            .expect("pin should succeed");
        assert!(pinned.pinned);
        assert!(matches!(
            store.delete_blob(&created.key).await,
            Err(BlobStoreError::Conflict("Pinned objects cannot be deleted"))
        ));

        store
            .unpin_blob(&created.key)
            .await
            .expect("unpin should succeed");
        store
            .delete_blob(&created.key)
            .await
            .expect("delete should succeed");
        assert!(matches!(
            store.get_blob_metadata(&created.key).await,
            Err(BlobStoreError::NotFound)
        ));
    }
}
