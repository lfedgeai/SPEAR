use crate::sms::blob_store::{
    BlobContent, BlobListPage, BlobMetadata, BlobStore, BlobStoreError, BlobStoreStatus,
    BlobSummary, PutBlobRequest, SharedBlobStore,
};
use crate::sms::config::RustfsObjectStorageConfig;
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_config::timeout::TimeoutConfig;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials, Region};
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::operation::create_bucket::CreateBucketError;
use aws_sdk_s3::operation::delete_object::DeleteObjectError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::head_bucket::HeadBucketError;
use aws_sdk_s3::operation::head_object::HeadObjectError;
use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Error;
use aws_sdk_s3::operation::put_object::PutObjectError;
use aws_sdk_s3::primitives::ByteStream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;
use uuid::Uuid;

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use tokio::sync::Mutex;

/// Build a shared RustFS blob store handle backed by an S3-compatible client.
/// 构建基于 S3-compatible client 的共享 RustFS BlobStore 句柄。
pub async fn rustfs_blob_store(config: RustfsObjectStorageConfig) -> anyhow::Result<SharedBlobStore> {
    let store = RustFsBlobStore::new(config).await?;
    Ok(Arc::new(store))
}

/// RustFS-backed blob store using a separate metadata sidecar object.
/// 基于 RustFS 的 BlobStore，使用独立 sidecar 对象保存元数据。
#[derive(Clone)]
pub struct RustFsBlobStore {
    bucket: String,
    endpoint: String,
    root: String,
    create_bucket_if_missing: bool,
    object_client: Arc<dyn RustfsObjectClient>,
    bucket_ready: Arc<OnceCell<()>>,
}

impl std::fmt::Debug for RustFsBlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RustFsBlobStore")
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .field("create_bucket_if_missing", &self.create_bucket_if_missing)
            .finish_non_exhaustive()
    }
}

impl RustFsBlobStore {
    /// Create a RustFS blob store from runtime configuration.
    /// 基于运行时配置创建 RustFS BlobStore。
    pub async fn new(config: RustfsObjectStorageConfig) -> anyhow::Result<Self> {
        let object_client = AwsRustfsObjectClient::from_config(&config).await?;
        Ok(Self::from_parts(config, Arc::new(object_client)))
    }

    /// Create a RustFS blob store from configuration and an injected client.
    /// 基于配置与注入 client 创建 RustFS BlobStore。
    fn from_parts(
        config: RustfsObjectStorageConfig,
        object_client: Arc<dyn RustfsObjectClient>,
    ) -> Self {
        Self {
            bucket: config.bucket.trim().to_string(),
            endpoint: config.endpoint.trim().to_string(),
            root: normalize_root_prefix(&config.root),
            create_bucket_if_missing: config.create_bucket_if_missing,
            object_client,
            bucket_ready: Arc::new(OnceCell::new()),
        }
    }

    fn blob_object_key(&self, key: &str) -> String {
        if self.root.is_empty() {
            format!("blobs/{}", key)
        } else {
            format!("{}/blobs/{}", self.root, key)
        }
    }

    fn metadata_object_key(&self, key: &str) -> String {
        if self.root.is_empty() {
            format!("meta/{}.json", key)
        } else {
            format!("{}/meta/{}.json", self.root, key)
        }
    }

    fn metadata_prefix(&self) -> String {
        if self.root.is_empty() {
            "meta/".to_string()
        } else {
            format!("{}/meta/", self.root)
        }
    }

    async fn ensure_bucket_ready(&self) -> Result<(), BlobStoreError> {
        self.bucket_ready
            .get_or_try_init(|| async {
                self.object_client
                    .ensure_bucket(self.create_bucket_if_missing)
                    .await?;
                Ok(())
            })
            .await
            .map(|_| ())
    }

    async fn read_sidecar_metadata(&self, key: &str) -> Result<Option<RustfsBlobMeta>, BlobStoreError> {
        match self
            .object_client
            .get_object(&self.metadata_object_key(key))
            .await
        {
            Ok(content) => serde_json::from_slice::<RustfsBlobMeta>(&content.bytes)
                .map(Some)
                .map_err(|_| BlobStoreError::Internal),
            Err(BlobStoreError::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn write_sidecar_metadata(&self, meta: &RustfsBlobMeta) -> Result<(), BlobStoreError> {
        let key = self.metadata_object_key(&meta.id);
        let body = serde_json::to_vec(meta).map_err(|_| BlobStoreError::Internal)?;
        self.object_client
            .put_object(&key, body, Some("application/json"))
            .await
    }

    async fn load_blob_metadata(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        let head = self
            .object_client
            .head_object(&self.blob_object_key(key))
            .await?;
        let sidecar = self.read_sidecar_metadata(key).await?;
        Ok(blob_metadata_from_rustfs(key, head, sidecar))
    }

    async fn update_lifecycle(
        &self,
        key: &str,
        mutate: impl FnOnce(&mut RustfsBlobMeta) -> Result<(), &'static str>,
    ) -> Result<BlobMetadata, BlobStoreError> {
        let current = self.load_blob_metadata(key).await?;
        let mut meta = RustfsBlobMeta::from_blob_metadata(current);
        mutate(&mut meta).map_err(BlobStoreError::Conflict)?;
        meta.updated_at = Some(current_epoch_seconds()?);
        self.write_sidecar_metadata(&meta).await?;
        Ok(meta.into_blob_metadata())
    }
}

#[async_trait]
impl BlobStore for RustFsBlobStore {
    async fn put_blob(&self, request: PutBlobRequest) -> Result<BlobMetadata, BlobStoreError> {
        self.ensure_bucket_ready().await?;

        let key = Uuid::new_v4().to_string();
        let created_at = current_epoch_seconds()?;
        let size = request.bytes.len() as u64;
        self.object_client
            .put_object(
                &self.blob_object_key(&key),
                request.bytes,
                request.content_type.as_deref(),
            )
            .await?;

        let meta = RustfsBlobMeta {
            id: key,
            name: request.name,
            content_type: request.content_type,
            len: Some(size),
            created_at: Some(created_at),
            updated_at: Some(created_at),
            ref_count: 0,
            pinned: false,
        };
        self.write_sidecar_metadata(&meta).await?;
        Ok(meta.into_blob_metadata())
    }

    async fn get_blob(&self, key: &str) -> Result<BlobContent, BlobStoreError> {
        self.ensure_bucket_ready().await?;

        let content = self
            .object_client
            .get_object(&self.blob_object_key(key))
            .await?;
        let sidecar = self.read_sidecar_metadata(key).await?;
        Ok(BlobContent {
            bytes: content.bytes,
            content_type: sidecar
                .and_then(|meta| meta.content_type)
                .or(content.content_type),
        })
    }

    async fn get_blob_metadata(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        self.ensure_bucket_ready().await?;
        self.load_blob_metadata(key).await
    }

    async fn delete_blob(&self, key: &str) -> Result<(), BlobStoreError> {
        self.ensure_bucket_ready().await?;

        let meta = self.load_blob_metadata(key).await?;
        if meta.pinned {
            return Err(BlobStoreError::Conflict("Pinned objects cannot be deleted"));
        }
        if meta.ref_count > 0 {
            return Err(BlobStoreError::Conflict("Referenced objects cannot be deleted"));
        }

        self.object_client
            .delete_object(&self.blob_object_key(key))
            .await?;
        let _ = self
            .object_client
            .delete_object(&self.metadata_object_key(key))
            .await;
        Ok(())
    }

    async fn list_blobs(
        &self,
        prefix: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<BlobListPage, BlobStoreError> {
        self.ensure_bucket_ready().await?;

        let metadata_keys = self
            .object_client
            .list_objects(&self.metadata_prefix())
            .await?;
        let mut items = Vec::new();
        for object_key in metadata_keys {
            let Some(key) = decode_blob_key_from_metadata_object(&self.metadata_prefix(), &object_key) else {
                continue;
            };
            let meta = match self.read_sidecar_metadata(&key).await? {
                Some(meta) => meta,
                None => continue,
            };
            items.push(BlobSummary {
                key: meta.id.clone(),
                name: meta.name.clone(),
                size: meta.len.unwrap_or(0),
                updated_at: meta.updated_at.or(meta.created_at).unwrap_or(0),
                ref_count: meta.ref_count,
                pinned: meta.pinned,
            });
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
        self.ensure_bucket_ready().await?;
        self.update_lifecycle(key, |meta| {
            meta.ref_count = meta.ref_count.saturating_add(1);
            Ok(())
        })
        .await
    }

    async fn release_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        self.ensure_bucket_ready().await?;
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
        self.ensure_bucket_ready().await?;
        self.update_lifecycle(key, |meta| {
            meta.pinned = true;
            Ok(())
        })
        .await
    }

    async fn unpin_blob(&self, key: &str) -> Result<BlobMetadata, BlobStoreError> {
        self.ensure_bucket_ready().await?;
        self.update_lifecycle(key, |meta| {
            meta.pinned = false;
            Ok(())
        })
        .await
    }

    async fn status(&self) -> Result<BlobStoreStatus, BlobStoreError> {
        self.ensure_bucket_ready().await?;
        Ok(BlobStoreStatus {
            backend: "rustfs-blobstore".to_string(),
            healthy: true,
            message: format!(
                "Using RustFsBlobStore at {} bucket={} root={}",
                self.endpoint,
                self.bucket,
                if self.root.is_empty() { "/" } else { self.root.as_str() }
            ),
        })
    }
}

/// Minimal object payload returned by the RustFS client abstraction.
/// RustFS client 抽象返回的最小对象载荷。
#[derive(Debug, Clone, PartialEq, Eq)]
struct RustfsObjectContent {
    bytes: Vec<u8>,
    content_type: Option<String>,
}

/// Minimal object head metadata returned by the RustFS client abstraction.
/// RustFS client 抽象返回的最小对象 head 元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
struct RustfsObjectHead {
    size: u64,
    content_type: Option<String>,
}

/// Low-level RustFS object client abstraction used to keep BlobStore tests local.
/// 低层 RustFS 对象 client 抽象，用于让 BlobStore 测试保持本地化。
#[async_trait]
trait RustfsObjectClient: Send + Sync {
    async fn ensure_bucket(&self, create_if_missing: bool) -> Result<(), BlobStoreError>;
    async fn put_object(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<(), BlobStoreError>;
    async fn get_object(&self, key: &str) -> Result<RustfsObjectContent, BlobStoreError>;
    async fn head_object(&self, key: &str) -> Result<RustfsObjectHead, BlobStoreError>;
    async fn delete_object(&self, key: &str) -> Result<(), BlobStoreError>;
    async fn list_objects(&self, prefix: &str) -> Result<Vec<String>, BlobStoreError>;
}

/// AWS SDK based S3-compatible RustFS object client.
/// 基于 AWS SDK 的 S3-compatible RustFS 对象 client。
#[derive(Debug, Clone)]
struct AwsRustfsObjectClient {
    bucket: String,
    client: aws_sdk_s3::Client,
}

impl AwsRustfsObjectClient {
    /// Build the AWS SDK client used to talk to RustFS.
    /// 构建用于访问 RustFS 的 AWS SDK client。
    async fn from_config(config: &RustfsObjectStorageConfig) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !config.endpoint.trim().is_empty(),
            "object_storage.rustfs.endpoint must not be empty"
        );
        anyhow::ensure!(
            !config.bucket.trim().is_empty(),
            "object_storage.rustfs.bucket must not be empty"
        );

        let timeout_config = TimeoutConfig::builder()
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .operation_attempt_timeout(Duration::from_millis(
                config.read_timeout_ms.max(config.write_timeout_ms),
            ))
            .build();

        let mut loader = aws_config::defaults(BehaviorVersion::latest())
            .endpoint_url(config.endpoint.trim().to_string())
            .region(Region::new(config.region.trim().to_string()))
            .timeout_config(timeout_config);

        if !config.access_key.trim().is_empty() || !config.secret_key.trim().is_empty() {
            anyhow::ensure!(
                !config.access_key.trim().is_empty() && !config.secret_key.trim().is_empty(),
                "object_storage.rustfs.access_key and secret_key must be configured together"
            );
            loader = loader.credentials_provider(Credentials::new(
                config.access_key.trim(),
                config.secret_key.trim(),
                None,
                None,
                "sms-rustfs-blobstore",
            ));
        }

        let shared_config = loader.load().await;
        let s3_config = S3ConfigBuilder::from(&shared_config)
            .force_path_style(config.path_style)
            .build();

        Ok(Self {
            bucket: config.bucket.trim().to_string(),
            client: aws_sdk_s3::Client::from_conf(s3_config),
        })
    }
}

#[async_trait]
impl RustfsObjectClient for AwsRustfsObjectClient {
    async fn ensure_bucket(&self, create_if_missing: bool) -> Result<(), BlobStoreError> {
        match self.client.head_bucket().bucket(&self.bucket).send().await {
            Ok(_) => Ok(()),
            Err(error) if is_head_bucket_not_found(&error) && create_if_missing => {
                self.client
                    .create_bucket()
                    .bucket(&self.bucket)
                    .send()
                    .await
                    .map_err(map_create_bucket_error)?;
                Ok(())
            }
            Err(error) => Err(map_head_bucket_error(error)),
        }
    }

    async fn put_object(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<(), BlobStoreError> {
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(bytes));
        let request = if let Some(content_type) = content_type {
            request.content_type(content_type)
        } else {
            request
        };
        request.send().await.map_err(map_put_object_error)?;
        Ok(())
    }

    async fn get_object(&self, key: &str) -> Result<RustfsObjectContent, BlobStoreError> {
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(map_get_object_error)?;
        let content_type = output.content_type().map(str::to_string);
        let bytes = output
            .body
            .collect()
            .await
            .map_err(|_| BlobStoreError::Internal)?
            .into_bytes()
            .to_vec();
        Ok(RustfsObjectContent {
            bytes,
            content_type,
        })
    }

    async fn head_object(&self, key: &str) -> Result<RustfsObjectHead, BlobStoreError> {
        let output = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(map_head_object_error)?;
        Ok(RustfsObjectHead {
            size: output.content_length().unwrap_or_default().max(0) as u64,
            content_type: output.content_type().map(str::to_string),
        })
    }

    async fn delete_object(&self, key: &str) -> Result<(), BlobStoreError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(map_delete_object_error)?;
        Ok(())
    }

    async fn list_objects(&self, prefix: &str) -> Result<Vec<String>, BlobStoreError> {
        let mut continuation_token: Option<String> = None;
        let mut keys = Vec::new();

        loop {
            let request = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix);
            let request = if let Some(token) = continuation_token.as_deref() {
                request.continuation_token(token)
            } else {
                request
            };
            let page = request.send().await.map_err(map_list_objects_error)?;
            for object in page.contents() {
                if let Some(key) = object.key() {
                    keys.push(key.to_string());
                }
            }
            if !page.is_truncated().unwrap_or(false) {
                break;
            }
            continuation_token = page.next_continuation_token().map(str::to_string);
        }

        Ok(keys)
    }
}

/// Metadata sidecar stored next to each RustFS blob object.
/// 与每个 RustFS blob 对象配套存储的元数据 sidecar。
#[derive(Clone, Debug, Deserialize, Serialize)]
struct RustfsBlobMeta {
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

impl RustfsBlobMeta {
    fn from_blob_metadata(meta: BlobMetadata) -> Self {
        Self {
            id: meta.key,
            name: meta.name,
            content_type: meta.content_type,
            len: Some(meta.size),
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            ref_count: meta.ref_count,
            pinned: meta.pinned,
        }
    }

    fn into_blob_metadata(self) -> BlobMetadata {
        BlobMetadata {
            key: self.id,
            name: self.name,
            content_type: self.content_type,
            size: self.len.unwrap_or(0),
            created_at: self.created_at,
            updated_at: self.updated_at,
            ref_count: self.ref_count,
            pinned: self.pinned,
        }
    }
}

fn blob_metadata_from_rustfs(
    key: &str,
    head: RustfsObjectHead,
    sidecar: Option<RustfsBlobMeta>,
) -> BlobMetadata {
    match sidecar {
        Some(meta) => BlobMetadata {
            size: meta.len.unwrap_or(head.size),
            key: key.to_string(),
            name: meta.name,
            content_type: meta.content_type.or(head.content_type),
            created_at: meta.created_at,
            updated_at: meta.updated_at.or(meta.created_at),
            ref_count: meta.ref_count,
            pinned: meta.pinned,
        },
        None => BlobMetadata {
            key: key.to_string(),
            name: None,
            content_type: head.content_type,
            size: head.size,
            created_at: None,
            updated_at: None,
            ref_count: 0,
            pinned: false,
        },
    }
}

fn normalize_root_prefix(root: &str) -> String {
    root.trim().trim_matches('/').to_string()
}

fn decode_blob_key_from_metadata_object(metadata_prefix: &str, object_key: &str) -> Option<String> {
    let suffix = object_key.strip_prefix(metadata_prefix)?;
    Some(suffix.strip_suffix(".json")?.to_string())
}

fn current_epoch_seconds() -> Result<u64, BlobStoreError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| BlobStoreError::Internal)
        .map(|duration| duration.as_secs())
}

fn is_service_not_found<E>(error: &E) -> bool
where
    E: ProvideErrorMetadata,
{
    matches!(error.code(), Some(code) if matches!(code, "NotFound" | "NoSuchBucket" | "NoSuchKey" | "404"))
}

fn map_head_bucket_error(error: aws_sdk_s3::error::SdkError<HeadBucketError>) -> BlobStoreError {
    if error
        .as_service_error()
        .map(is_service_not_found)
        .unwrap_or(false)
    {
        BlobStoreError::NotFound
    } else {
        BlobStoreError::Internal
    }
}

fn is_head_bucket_not_found(error: &aws_sdk_s3::error::SdkError<HeadBucketError>) -> bool {
    error
        .as_service_error()
        .map(is_service_not_found)
        .unwrap_or(false)
}

fn map_create_bucket_error(
    error: aws_sdk_s3::error::SdkError<CreateBucketError>,
) -> BlobStoreError {
    if error
        .as_service_error()
        .and_then(|service_error| service_error.code())
        == Some("BucketAlreadyOwnedByYou")
    {
        return BlobStoreError::Internal;
    }
    BlobStoreError::Internal
}

fn map_put_object_error(error: aws_sdk_s3::error::SdkError<PutObjectError>) -> BlobStoreError {
    if error
        .as_service_error()
        .map(is_service_not_found)
        .unwrap_or(false)
    {
        BlobStoreError::NotFound
    } else {
        BlobStoreError::Internal
    }
}

fn map_get_object_error(error: aws_sdk_s3::error::SdkError<GetObjectError>) -> BlobStoreError {
    if error
        .as_service_error()
        .map(is_service_not_found)
        .unwrap_or(false)
    {
        BlobStoreError::NotFound
    } else {
        BlobStoreError::Internal
    }
}

fn map_head_object_error(error: aws_sdk_s3::error::SdkError<HeadObjectError>) -> BlobStoreError {
    if error
        .as_service_error()
        .map(is_service_not_found)
        .unwrap_or(false)
    {
        BlobStoreError::NotFound
    } else {
        BlobStoreError::Internal
    }
}

fn map_delete_object_error(
    error: aws_sdk_s3::error::SdkError<DeleteObjectError>,
) -> BlobStoreError {
    if error
        .as_service_error()
        .map(is_service_not_found)
        .unwrap_or(false)
    {
        BlobStoreError::NotFound
    } else {
        BlobStoreError::Internal
    }
}

fn map_list_objects_error(
    _error: aws_sdk_s3::error::SdkError<ListObjectsV2Error>,
) -> BlobStoreError {
    BlobStoreError::Internal
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory fake RustFS client for unit testing BlobStore semantics.
    /// 用于测试 BlobStore 语义的内存版 fake RustFS client。
    #[derive(Debug, Default)]
    struct InMemoryRustfsClient {
        state: Mutex<InMemoryRustfsState>,
    }

    #[derive(Debug, Default)]
    struct InMemoryRustfsState {
        bucket_exists: bool,
        ensure_bucket_calls: usize,
        objects: HashMap<String, InMemoryObject>,
    }

    #[derive(Debug, Clone)]
    struct InMemoryObject {
        bytes: Vec<u8>,
        content_type: Option<String>,
    }

    #[async_trait]
    impl RustfsObjectClient for InMemoryRustfsClient {
        async fn ensure_bucket(&self, create_if_missing: bool) -> Result<(), BlobStoreError> {
            let mut guard = self.state.lock().await;
            guard.ensure_bucket_calls += 1;
            if !guard.bucket_exists && create_if_missing {
                guard.bucket_exists = true;
            }
            if guard.bucket_exists {
                Ok(())
            } else {
                Err(BlobStoreError::NotFound)
            }
        }

        async fn put_object(
            &self,
            key: &str,
            bytes: Vec<u8>,
            content_type: Option<&str>,
        ) -> Result<(), BlobStoreError> {
            let mut guard = self.state.lock().await;
            if !guard.bucket_exists {
                return Err(BlobStoreError::NotFound);
            }
            guard.objects.insert(
                key.to_string(),
                InMemoryObject {
                    bytes,
                    content_type: content_type.map(str::to_string),
                },
            );
            Ok(())
        }

        async fn get_object(&self, key: &str) -> Result<RustfsObjectContent, BlobStoreError> {
            let guard = self.state.lock().await;
            let Some(object) = guard.objects.get(key) else {
                return Err(BlobStoreError::NotFound);
            };
            Ok(RustfsObjectContent {
                bytes: object.bytes.clone(),
                content_type: object.content_type.clone(),
            })
        }

        async fn head_object(&self, key: &str) -> Result<RustfsObjectHead, BlobStoreError> {
            let guard = self.state.lock().await;
            let Some(object) = guard.objects.get(key) else {
                return Err(BlobStoreError::NotFound);
            };
            Ok(RustfsObjectHead {
                size: object.bytes.len() as u64,
                content_type: object.content_type.clone(),
            })
        }

        async fn delete_object(&self, key: &str) -> Result<(), BlobStoreError> {
            let mut guard = self.state.lock().await;
            if guard.objects.remove(key).is_some() {
                Ok(())
            } else {
                Err(BlobStoreError::NotFound)
            }
        }

        async fn list_objects(&self, prefix: &str) -> Result<Vec<String>, BlobStoreError> {
            let guard = self.state.lock().await;
            let mut keys = guard
                .objects
                .keys()
                .filter(|key| key.starts_with(prefix))
                .cloned()
                .collect::<Vec<_>>();
            keys.sort();
            Ok(keys)
        }
    }

    fn test_rustfs_config() -> RustfsObjectStorageConfig {
        RustfsObjectStorageConfig {
            endpoint: "http://127.0.0.1:9000".to_string(),
            bucket: "sms-objects".to_string(),
            root: "tenant-a".to_string(),
            access_key: "access-key".to_string(),
            secret_key: "secret-key".to_string(),
            ..Default::default()
        }
    }

    fn test_store(client: Arc<dyn RustfsObjectClient>) -> RustFsBlobStore {
        RustFsBlobStore::from_parts(test_rustfs_config(), client)
    }

    #[tokio::test]
    async fn rustfs_store_round_trips_blob_content_and_metadata() {
        let client = Arc::new(InMemoryRustfsClient::default());
        let store = test_store(client);

        let created = store
            .put_blob(PutBlobRequest {
                name: Some("demo.wasm".to_string()),
                content_type: Some("application/wasm".to_string()),
                bytes: vec![1, 2, 3, 4],
            })
            .await
            .expect("put should succeed");

        let content = store
            .get_blob(&created.key)
            .await
            .expect("get should succeed");
        let metadata = store
            .get_blob_metadata(&created.key)
            .await
            .expect("metadata should succeed");

        assert_eq!(content.bytes, vec![1, 2, 3, 4]);
        assert_eq!(content.content_type.as_deref(), Some("application/wasm"));
        assert_eq!(metadata.name.as_deref(), Some("demo.wasm"));
        assert_eq!(metadata.size, 4);
        assert_eq!(metadata.ref_count, 0);
        assert!(!metadata.pinned);
    }

    #[tokio::test]
    async fn rustfs_store_lifecycle_changes_gate_delete() {
        let client = Arc::new(InMemoryRustfsClient::default());
        let store = test_store(client);
        let created = store
            .put_blob(PutBlobRequest {
                name: Some("artifact.bin".to_string()),
                content_type: Some("application/octet-stream".to_string()),
                bytes: vec![7; 8],
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

    #[tokio::test]
    async fn rustfs_store_initializes_bucket_once_and_lists_items() {
        let client = Arc::new(InMemoryRustfsClient::default());
        let store = test_store(client.clone());

        let first = store
            .put_blob(PutBlobRequest {
                name: Some("alpha.txt".to_string()),
                content_type: Some("text/plain".to_string()),
                bytes: b"alpha".to_vec(),
            })
            .await
            .expect("first put should succeed");
        let second = store
            .put_blob(PutBlobRequest {
                name: Some("beta.txt".to_string()),
                content_type: Some("text/plain".to_string()),
                bytes: b"beta".to_vec(),
            })
            .await
            .expect("second put should succeed");

        let page = store
            .list_blobs(Some("beta"), Some(10), Some(0))
            .await
            .expect("list should succeed");
        let status = store.status().await.expect("status should succeed");
        let guard = client.state.lock().await;

        assert_eq!(page.total_count, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].key, second.key);
        assert!(status.healthy);
        assert_eq!(guard.ensure_bucket_calls, 1);
        assert_ne!(first.key, second.key);
    }
}
