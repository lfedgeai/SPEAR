//! Object service implementation for spearlet
//! spearlet的对象服务实现

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn, error};
use serde::{Deserialize, Serialize};
use crate::storage::{KvStore, serialization};

use crate::proto::spearlet::{
    object_service_server::ObjectService,
    Object, ObjectMeta, 
    PutObjectRequest, PutObjectResponse,
    GetObjectRequest, GetObjectResponse,
    ListObjectsRequest, ListObjectsResponse,
    AddObjectRefRequest, AddObjectRefResponse,
    RemoveObjectRefRequest, RemoveObjectRefResponse,
    PinObjectRequest, PinObjectResponse,
    UnpinObjectRequest, UnpinObjectResponse,
    DeleteObjectRequest, DeleteObjectResponse,
};

/// Object key generation helpers / 对象键生成辅助函数
mod object_keys {
    /// Generate object key / 生成对象键
    pub fn object_key(key: &str) -> String {
        format!("object:{}", key)
    }
    
    /// Object prefix for scanning / 对象扫描前缀
    pub fn object_prefix() -> &'static str {
        "object:"
    }
}

/// Object service statistics / 对象服务统计信息
#[derive(Debug, Clone)]
pub struct ObjectServiceStats {
    pub object_count: usize,
    pub total_size: u64,
    pub pinned_count: usize,
}

/// Internal object representation / 内部对象表示
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredObject {
    key: String,
    value: Vec<u8>,
    created_at: i64,
    updated_at: i64,
    ref_count: i32,
    pinned: bool,
    metadata: HashMap<String, String>,
}

impl StoredObject {
    fn new(key: String, value: Vec<u8>, metadata: HashMap<String, String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        Self {
            key,
            value,
            created_at: now,
            updated_at: now,
            ref_count: 1,
            pinned: false,
            metadata,
        }
    }

    fn to_object(&self) -> Object {
        Object {
            key: self.key.clone(),
            value: self.value.clone(),
            size: self.value.len() as i64,
            created_at: self.created_at,
            updated_at: self.updated_at,
            ref_count: self.ref_count,
            pinned: self.pinned,
            metadata: self.metadata.clone(),
        }
    }

    fn to_object_meta(&self) -> ObjectMeta {
        ObjectMeta {
            key: self.key.clone(),
            size: self.value.len() as i64,
            created_at: self.created_at,
            updated_at: self.updated_at,
            ref_count: self.ref_count,
            pinned: self.pinned,
            metadata: self.metadata.clone(),
        }
    }

    fn update_value(&mut self, value: Vec<u8>, metadata: HashMap<String, String>) {
        self.value = value;
        self.metadata = metadata;
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
    }
}

// Implement ObjectService for Arc<ObjectServiceImpl> to support gRPC server
// 为Arc<ObjectServiceImpl>实现ObjectService trait以支持gRPC服务器
#[tonic::async_trait]
impl ObjectService for Arc<ObjectServiceImpl> {
    async fn put_object(
        &self,
        request: Request<PutObjectRequest>,
    ) -> Result<Response<PutObjectResponse>, Status> {
        self.as_ref().put_object(request).await
    }

    async fn get_object(
        &self,
        request: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>, Status> {
        self.as_ref().get_object(request).await
    }

    async fn list_objects(
        &self,
        request: Request<ListObjectsRequest>,
    ) -> Result<Response<ListObjectsResponse>, Status> {
        self.as_ref().list_objects(request).await
    }

    async fn add_object_ref(
        &self,
        request: Request<AddObjectRefRequest>,
    ) -> Result<Response<AddObjectRefResponse>, Status> {
        self.as_ref().add_object_ref(request).await
    }

    async fn remove_object_ref(
        &self,
        request: Request<RemoveObjectRefRequest>,
    ) -> Result<Response<RemoveObjectRefResponse>, Status> {
        self.as_ref().remove_object_ref(request).await
    }

    async fn pin_object(
        &self,
        request: Request<PinObjectRequest>,
    ) -> Result<Response<PinObjectResponse>, Status> {
        self.as_ref().pin_object(request).await
    }

    async fn unpin_object(
        &self,
        request: Request<UnpinObjectRequest>,
    ) -> Result<Response<UnpinObjectResponse>, Status> {
        self.as_ref().unpin_object(request).await
    }

    async fn delete_object(
        &self,
        request: Request<DeleteObjectRequest>,
    ) -> Result<Response<DeleteObjectResponse>, Status> {
        self.as_ref().delete_object(request).await
    }
}

/// Object service implementation / 对象服务实现
#[derive(Debug)]
pub struct ObjectServiceImpl {
    /// KV store for object storage / 对象存储的KV存储
    kv_store: Arc<dyn KvStore>,
    /// Maximum object size in bytes / 最大对象大小（字节）
    max_object_size: u64,
}

impl ObjectServiceImpl {
    /// Create a new object service with KV store / 使用KV存储创建新的对象服务
    pub fn new(kv_store: Arc<dyn KvStore>, max_object_size: u64) -> Self {
        Self {
            kv_store,
            max_object_size,
        }
    }
    
    /// Create a new object service with memory KV store / 使用内存KV存储创建新的对象服务
    pub fn new_with_memory(max_object_size: u64) -> Self {
        use crate::storage::MemoryKvStore;
        Self {
            kv_store: Arc::new(MemoryKvStore::new()),
            max_object_size,
        }
    }

    /// Get object count / 获取对象数量
    pub async fn object_count(&self) -> usize {
        match self.kv_store.scan_prefix(object_keys::object_prefix()).await {
            Ok(pairs) => pairs.len(),
            Err(_) => 0,
        }
    }

    /// Get total object size / 获取对象总大小
    pub async fn total_object_size(&self) -> u64 {
        match self.kv_store.scan_prefix(object_keys::object_prefix()).await {
            Ok(pairs) => {
                let mut total_size = 0u64;
                for pair in pairs {
                    if let Ok(stored_obj) = serialization::deserialize::<StoredObject>(&pair.value) {
                        total_size += stored_obj.value.len() as u64;
                    }
                }
                total_size
            },
            Err(_) => 0,
        }
    }

    /// Get pinned object count / 获取固定对象数量
    pub async fn pinned_object_count(&self) -> usize {
        match self.kv_store.scan_prefix(object_keys::object_prefix()).await {
            Ok(pairs) => {
                let mut pinned_count = 0;
                for pair in pairs {
                    if let Ok(stored_obj) = serialization::deserialize::<StoredObject>(&pair.value) {
                        if stored_obj.pinned {
                            pinned_count += 1;
                        }
                    }
                }
                pinned_count
            },
            Err(_) => 0,
        }
    }

    /// Cleanup objects with zero references / 清理零引用对象
    pub async fn cleanup_objects(&self) -> usize {
        match self.kv_store.scan_prefix(object_keys::object_prefix()).await {
            Ok(pairs) => {
                let mut cleaned_count = 0;
                for pair in pairs {
                    if let Ok(stored_obj) = serialization::deserialize::<StoredObject>(&pair.value) {
                        if stored_obj.ref_count <= 0 && !stored_obj.pinned {
                            if let Ok(_) = self.kv_store.delete(&pair.key).await {
                                cleaned_count += 1;
                            }
                        }
                    }
                }
                if cleaned_count > 0 {
                    info!("Cleaned up {} objects with zero references", cleaned_count);
                }
                cleaned_count
            },
            Err(_) => 0,
        }
    }

    /// Get service statistics / 获取服务统计信息
    pub async fn get_stats(&self) -> ObjectServiceStats {
        ObjectServiceStats {
            object_count: self.object_count().await,
            total_size: self.total_object_size().await,
            pinned_count: self.pinned_object_count().await,
        }
    }
}

#[tonic::async_trait]
impl ObjectService for ObjectServiceImpl {
    /// Put or overwrite object content / 写入或覆盖对象内容
    async fn put_object(
        &self,
        request: Request<PutObjectRequest>,
    ) -> Result<Response<PutObjectResponse>, Status> {
        let req = request.into_inner();
        
        debug!("PutObject request for key: {}", req.key);
        
        // Validate key / 验证键
        if req.key.is_empty() {
            return Ok(Response::new(PutObjectResponse {
                success: false,
                message: "Object key cannot be empty".to_string(),
                object_meta: None,
            }));
        }

        // Validate object size / 验证对象大小
        if req.value.len() as u64 > self.max_object_size {
            return Ok(Response::new(PutObjectResponse {
                success: false,
                message: format!("Object size {} exceeds maximum size {}", 
                    req.value.len(), self.max_object_size),
                object_meta: None,
            }));
        }

        let kv_key = object_keys::object_key(&req.key);
        
        // Check if object exists and overwrite flag / 检查对象是否存在和覆盖标志
        if let Ok(Some(existing_data)) = self.kv_store.get(&kv_key).await {
            if !req.overwrite {
                if let Ok(existing_obj) = serialization::deserialize::<StoredObject>(&existing_data) {
                    return Ok(Response::new(PutObjectResponse {
                        success: false,
                        message: "Object already exists and overwrite is false".to_string(),
                        object_meta: Some(existing_obj.to_object_meta()),
                    }));
                }
            }
        }

        // Create or update object / 创建或更新对象
        let stored_obj = if let Ok(Some(existing_data)) = self.kv_store.get(&kv_key).await {
            if let Ok(mut existing_obj) = serialization::deserialize::<StoredObject>(&existing_data) {
                existing_obj.update_value(req.value, req.metadata);
                existing_obj
            } else {
                StoredObject::new(req.key.clone(), req.value, req.metadata)
            }
        } else {
            StoredObject::new(req.key.clone(), req.value, req.metadata)
        };

        // Store object in KV store / 在KV存储中存储对象
        match serialization::serialize(&stored_obj) {
            Ok(serialized_data) => {
                if let Err(e) = self.kv_store.put(&kv_key, &serialized_data).await {
                    error!("Failed to store object {}: {:?}", req.key, e);
                    return Ok(Response::new(PutObjectResponse {
                        success: false,
                        message: format!("Failed to store object: {:?}", e),
                        object_meta: None,
                    }));
                }
            },
            Err(e) => {
                error!("Failed to serialize object {}: {:?}", req.key, e);
                return Ok(Response::new(PutObjectResponse {
                    success: false,
                    message: format!("Failed to serialize object: {:?}", e),
                    object_meta: None,
                }));
            }
        }

        info!("Successfully put object: {}", req.key);
        
        Ok(Response::new(PutObjectResponse {
            success: true,
            message: "Object stored successfully".to_string(),
            object_meta: Some(stored_obj.to_object_meta()),
        }))
    }

    /// Get object content / 读取指定对象的内容
    async fn get_object(
        &self,
        request: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>, Status> {
        let req = request.into_inner();
        
        debug!("GetObject request for key: {}", req.key);
        
        let key = object_keys::object_key(&req.key);
        
        match self.kv_store.get(&key).await {
            Ok(Some(data)) => {
                match serialization::deserialize::<StoredObject>(&data) {
                    Ok(obj) => {
                        info!("Successfully retrieved object: {}", req.key);
                        Ok(Response::new(GetObjectResponse {
                            found: true,
                            message: "Object retrieved successfully".to_string(),
                            object: Some(obj.to_object()),
                        }))
                    }
                    Err(e) => {
                        error!("Failed to deserialize object {}: {}", req.key, e);
                        Err(Status::internal(format!("Failed to deserialize object: {}", e)))
                    }
                }
            }
            Ok(None) => {
                warn!("Object not found: {}", req.key);
                Ok(Response::new(GetObjectResponse {
                    found: false,
                    message: "Object not found".to_string(),
                    object: None,
                }))
            }
            Err(e) => {
                error!("Failed to get object {}: {}", req.key, e);
                Err(Status::internal(format!("Failed to get object: {}", e)))
            }
        }
    }

    /// List objects with specified prefix / 列出指定前缀下的所有对象
    async fn list_objects(
        &self,
        request: Request<ListObjectsRequest>,
    ) -> Result<Response<ListObjectsResponse>, Status> {
        let req = request.into_inner();
        
        debug!("ListObjects request with prefix: {}", req.prefix);
        
        let limit = if req.limit <= 0 { 1000 } else { req.limit as usize };
        
        // Scan all objects from KV store / 从 KV 存储中扫描所有对象
        let mut matching_objects: Vec<Object> = Vec::new();
        
        match self.kv_store.scan_prefix(object_keys::object_prefix()).await {
            Ok(entries) => {
                for pair in entries {
                    match serialization::deserialize::<StoredObject>(&pair.value) {
                        Ok(obj) => {
                            if obj.key.starts_with(&req.prefix) {
                                matching_objects.push(obj.to_object());
                            }
                        }
                        Err(e) => {
                            warn!("Failed to deserialize object during list: {}", e);
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to scan objects: {}", e);
                return Err(Status::internal(format!("Failed to scan objects: {}", e)));
            }
        }
        
        // Sort by key for consistent ordering / 按键排序以保持一致的顺序
        matching_objects.sort_by(|a, b| a.key.cmp(&b.key));
        
        // Handle pagination / 处理分页
        let start_index = if req.start_after.is_empty() {
            0
        } else {
            // Simple pagination using index (in production, use proper tokens)
            // 简单的索引分页（生产环境中应使用适当的令牌）
            req.start_after.parse::<usize>().unwrap_or(0)
        };
        
        let end_index = std::cmp::min(start_index + limit, matching_objects.len());
        let page_objects = matching_objects[start_index..end_index].to_vec();
        
        let has_more = end_index < matching_objects.len();
        let next_token = if has_more {
            end_index.to_string()
        } else {
            String::new()
        };
        
        info!("Listed {} objects with prefix: {}", page_objects.len(), req.prefix);
        
        Ok(Response::new(ListObjectsResponse {
            objects: page_objects,
            next_start_after: next_token,
            has_more,
        }))
    }

    /// Add reference count to prevent premature garbage collection / 增加引用计数，防止对象被提前回收
    async fn add_object_ref(
        &self,
        request: Request<AddObjectRefRequest>,
    ) -> Result<Response<AddObjectRefResponse>, Status> {
        let req = request.into_inner();
        let count = if req.count <= 0 { 1 } else { req.count };
        
        debug!("AddObjectRef request for key: {}, count: {}", req.key, count);
        
        let key = object_keys::object_key(&req.key);
        
        match self.kv_store.get(&key).await {
            Ok(Some(data)) => {
                match serialization::deserialize::<StoredObject>(&data) {
                    Ok(mut obj) => {
                        obj.ref_count += count;
                        let new_ref_count = obj.ref_count;
                        
                        // Save updated object back to KV store / 将更新后的对象保存回 KV 存储
                        match serialization::serialize(&obj) {
                            Ok(serialized_data) => {
                                match self.kv_store.put(&key, &serialized_data).await {
                                    Ok(_) => {
                                        info!("Added {} references to object: {}, new count: {}", count, req.key, new_ref_count);
                                        Ok(Response::new(AddObjectRefResponse {
                                            success: true,
                                            message: "Reference count added successfully".to_string(),
                                            new_ref_count,
                                        }))
                                    }
                                    Err(e) => {
                                        error!("Failed to save updated object {}: {}", req.key, e);
                                        Err(Status::internal(format!("Failed to save updated object: {}", e)))
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize updated object {}: {}", req.key, e);
                                Err(Status::internal(format!("Failed to serialize updated object: {}", e)))
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize object {}: {}", req.key, e);
                        Err(Status::internal(format!("Failed to deserialize object: {}", e)))
                    }
                }
            }
            Ok(None) => {
                warn!("Cannot add reference to non-existent object: {}", req.key);
                Ok(Response::new(AddObjectRefResponse {
                    success: false,
                    message: "Object not found".to_string(),
                    new_ref_count: 0,
                }))
            }
            Err(e) => {
                error!("Failed to get object {}: {}", req.key, e);
                Err(Status::internal(format!("Failed to get object: {}", e)))
            }
        }
    }

    /// Remove reference count for lifecycle management / 减少引用计数，用于生命周期管理
    async fn remove_object_ref(
        &self,
        request: Request<RemoveObjectRefRequest>,
    ) -> Result<Response<RemoveObjectRefResponse>, Status> {
        let req = request.into_inner();
        let count = if req.count <= 0 { 1 } else { req.count };
        
        debug!("RemoveObjectRef request for key: {}, count: {}", req.key, count);
        
        let key = object_keys::object_key(&req.key);
        
        match self.kv_store.get(&key).await {
            Ok(Some(data)) => {
                match serialization::deserialize::<StoredObject>(&data) {
                    Ok(mut obj) => {
                        obj.ref_count = std::cmp::max(0, obj.ref_count - count);
                        let new_ref_count = obj.ref_count;
                        let object_deleted = new_ref_count == 0 && !obj.pinned;
                        
                        if object_deleted {
                            // Delete object from KV store / 从 KV 存储中删除对象
                            match self.kv_store.delete(&key).await {
                                Ok(_) => {
                                    info!("Removed object {} due to zero references", req.key);
                                    Ok(Response::new(RemoveObjectRefResponse {
                                        success: true,
                                        message: "Object deleted due to zero references".to_string(),
                                        new_ref_count,
                                        deleted: object_deleted,
                                    }))
                                }
                                Err(e) => {
                                    error!("Failed to delete object {}: {}", req.key, e);
                                    Err(Status::internal(format!("Failed to delete object: {}", e)))
                                }
                            }
                        } else {
                            // Save updated object back to KV store / 将更新后的对象保存回 KV 存储
                            match serialization::serialize(&obj) {
                                Ok(serialized_data) => {
                                    match self.kv_store.put(&key, &serialized_data).await {
                                        Ok(_) => {
                                            info!("Removed {} references from object: {}, new count: {}", count, req.key, new_ref_count);
                                            Ok(Response::new(RemoveObjectRefResponse {
                                                success: true,
                                                message: "Reference count removed successfully".to_string(),
                                                new_ref_count,
                                                deleted: object_deleted,
                                            }))
                                        }
                                        Err(e) => {
                                            error!("Failed to save updated object {}: {}", req.key, e);
                                            Err(Status::internal(format!("Failed to save updated object: {}", e)))
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to serialize updated object {}: {}", req.key, e);
                                    Err(Status::internal(format!("Failed to serialize updated object: {}", e)))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize object {}: {}", req.key, e);
                        Err(Status::internal(format!("Failed to deserialize object: {}", e)))
                    }
                }
            }
            Ok(None) => {
                warn!("Cannot remove reference from non-existent object: {}", req.key);
                Ok(Response::new(RemoveObjectRefResponse {
                    success: false,
                    message: "Object not found".to_string(),
                    new_ref_count: 0,
                    deleted: false,
                }))
            }
            Err(e) => {
                error!("Failed to get object {}: {}", req.key, e);
                Err(Status::internal(format!("Failed to get object: {}", e)))
            }
        }
    }

    /// Pin object to disable automatic garbage collection / 将对象标记为常驻，禁用自动回收机制
    async fn pin_object(
        &self,
        request: Request<PinObjectRequest>,
    ) -> Result<Response<PinObjectResponse>, Status> {
        let req = request.into_inner();
        
        debug!("PinObject request for key: {}", req.key);
        
        let key = object_keys::object_key(&req.key);
        
        match self.kv_store.get(&key).await {
            Ok(Some(data)) => {
                match serialization::deserialize::<StoredObject>(&data) {
                    Ok(mut obj) => {
                        let was_already_pinned = obj.pinned;
                        obj.pinned = true;
                        
                        match serialization::serialize(&obj) {
                            Ok(serialized_data) => {
                                match self.kv_store.put(&key, &serialized_data).await {
                                    Ok(_) => {
                                        info!("Pinned object: {}, was already pinned: {}", req.key, was_already_pinned);
                                        
                                        Ok(Response::new(PinObjectResponse {
                                            success: true,
                                            message: "Object pinned successfully".to_string(),
                                        }))
                                    }
                                    Err(e) => {
                                        error!("Failed to store pinned object {}: {:?}", req.key, e);
                                        Ok(Response::new(PinObjectResponse {
                                            success: false,
                                            message: format!("Failed to store pinned object: {:?}", e),
                                        }))
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize object {}: {:?}", req.key, e);
                                Ok(Response::new(PinObjectResponse {
                                    success: false,
                                    message: format!("Failed to serialize object: {:?}", e),
                                }))
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize object {}: {:?}", req.key, e);
                        Ok(Response::new(PinObjectResponse {
                            success: false,
                            message: format!("Failed to deserialize object: {:?}", e),
                        }))
                    }
                }
            }
            Ok(None) => {
                warn!("Cannot pin non-existent object: {}", req.key);
                Ok(Response::new(PinObjectResponse {
                    success: false,
                    message: "Object not found".to_string(),
                }))
            }
            Err(e) => {
                error!("Failed to get object {} from KV store: {:?}", req.key, e);
                Ok(Response::new(PinObjectResponse {
                    success: false,
                    message: format!("Failed to get object from KV store: {:?}", e),
                }))
            }
        }
    }

    /// Unpin object to restore normal garbage collection / 取消常驻标记，恢复为正常回收状态
    async fn unpin_object(
        &self,
        request: Request<UnpinObjectRequest>,
    ) -> Result<Response<UnpinObjectResponse>, Status> {
        let req = request.into_inner();
        
        debug!("UnpinObject request for key: {}", req.key);
        
        let key = object_keys::object_key(&req.key);
        
        match self.kv_store.get(&key).await {
            Ok(Some(data)) => {
                match serialization::deserialize::<StoredObject>(&data) {
                    Ok(mut obj) => {
                        if !obj.pinned {
                            warn!("Object {} is not pinned", req.key);
                            Ok(Response::new(UnpinObjectResponse {
                                success: false,
                                message: "Object is not pinned".to_string(),
                            }))
                        } else {
                            obj.pinned = false;
                            obj.updated_at = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs() as i64;
                            
                            match serialization::serialize(&obj) {
                                Ok(serialized) => {
                                    match self.kv_store.put(&key, &serialized).await {
                                        Ok(_) => {
                                            info!("Unpinned object: {}", req.key);
                                            Ok(Response::new(UnpinObjectResponse {
                                                success: true,
                                                message: "Object unpinned successfully".to_string(),
                                            }))
                                        }
                                        Err(e) => {
                                            error!("Failed to update object {} in KV store: {:?}", req.key, e);
                                            Ok(Response::new(UnpinObjectResponse {
                                                success: false,
                                                message: format!("Failed to update object in KV store: {:?}", e),
                                            }))
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to serialize object {}: {:?}", req.key, e);
                                    Ok(Response::new(UnpinObjectResponse {
                                        success: false,
                                        message: format!("Failed to serialize object: {:?}", e),
                                    }))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize object {}: {:?}", req.key, e);
                        Ok(Response::new(UnpinObjectResponse {
                            success: false,
                            message: format!("Failed to deserialize object: {:?}", e),
                        }))
                    }
                }
            }
            Ok(None) => {
                warn!("Cannot unpin non-existent object: {}", req.key);
                Ok(Response::new(UnpinObjectResponse {
                    success: false,
                    message: "Object not found".to_string(),
                }))
            }
            Err(e) => {
                error!("Failed to get object {} from KV store: {:?}", req.key, e);
                Ok(Response::new(UnpinObjectResponse {
                    success: false,
                    message: format!("Failed to get object from KV store: {:?}", e),
                }))
            }
        }
    }

    /// Delete object (for debugging and manual cleanup) / 删除对象（用于调试和手动清理）
    async fn delete_object(
        &self,
        request: Request<DeleteObjectRequest>,
    ) -> Result<Response<DeleteObjectResponse>, Status> {
        let req = request.into_inner();
        
        debug!("DeleteObject request for key: {}, force: {}", req.key, req.force);
        
        let key = object_keys::object_key(&req.key);
        
        match self.kv_store.get(&key).await {
            Ok(Some(data)) => {
                match serialization::deserialize::<StoredObject>(&data) {
                    Ok(obj) => {
                        let was_pinned = obj.pinned;
                        
                        if was_pinned && !req.force {
                            warn!("Cannot delete pinned object without force flag: {}", req.key);
                            Ok(Response::new(DeleteObjectResponse {
                                success: false,
                                message: "Cannot delete pinned object without force flag".to_string(),
                                deleted: false,
                            }))
                        } else {
                            match self.kv_store.delete(&key).await {
                                Ok(_) => {
                                    info!("Deleted object: {}, was pinned: {}", req.key, was_pinned);
                                    
                                    Ok(Response::new(DeleteObjectResponse {
                                        success: true,
                                        message: "Object deleted successfully".to_string(),
                                        deleted: true,
                                    }))
                                }
                                Err(e) => {
                                    error!("Failed to delete object {} from KV store: {:?}", req.key, e);
                                    Ok(Response::new(DeleteObjectResponse {
                                        success: false,
                                        message: format!("Failed to delete object from KV store: {:?}", e),
                                        deleted: false,
                                    }))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize object {}: {:?}", req.key, e);
                        Ok(Response::new(DeleteObjectResponse {
                            success: false,
                            message: format!("Failed to deserialize object: {:?}", e),
                            deleted: false,
                        }))
                    }
                }
            }
            Ok(None) => {
                warn!("Cannot delete non-existent object: {}", req.key);
                Ok(Response::new(DeleteObjectResponse {
                    success: false,
                    message: "Object not found".to_string(),
                    deleted: false,
                }))
            }
            Err(e) => {
                error!("Failed to get object {} from KV store: {:?}", req.key, e);
                Ok(Response::new(DeleteObjectResponse {
                    success: false,
                    message: format!("Failed to get object from KV store: {:?}", e),
                    deleted: false,
                }))
            }
        }
    }
}