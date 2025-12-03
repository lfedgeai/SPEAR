//! Object service tests / 对象服务测试

#[cfg(test)]
use std::collections::HashMap;
use std::sync::Arc;
use tonic::{Request, Status};

use crate::proto::spearlet::{
    object_service_server::ObjectService, AddObjectRefRequest, DeleteObjectRequest,
    GetObjectRequest, ListObjectsRequest, PinObjectRequest, PutObjectRequest,
    RemoveObjectRefRequest, UnpinObjectRequest,
};
use crate::spearlet::object_service::{ObjectServiceImpl, ObjectServiceStats};
use crate::storage::kv::{KvStore, MemoryKvStore};

fn create_test_service() -> ObjectServiceImpl {
    ObjectServiceImpl::new_with_memory(1024 * 1024) // 1MB max object size
}

fn create_test_metadata() -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    metadata.insert("test_key".to_string(), "test_value".to_string());
    metadata.insert("author".to_string(), "test_user".to_string());
    metadata
}

#[tokio::test]
async fn test_object_service_creation() {
    // Test service creation / 测试服务创建
    let service = create_test_service();

    // Verify initial stats / 验证初始统计信息
    let stats = service.get_stats().await;
    assert_eq!(stats.object_count, 0);
    assert_eq!(stats.total_size, 0);
}

#[tokio::test]
async fn test_put_object() {
    // Test putting an object / 测试存储对象
    let service = create_test_service();

    let put_request = Request::new(PutObjectRequest {
        key: "test-key".to_string(),
        value: b"test-value".to_vec(),
        metadata: create_test_metadata(),
        overwrite: false,
    });

    let response = service.put_object(put_request).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    assert!(response.success);
    assert!(response.object_meta.is_some());
    let meta = response.object_meta.unwrap();
    assert_eq!(meta.key, "test-key");
    assert_eq!(meta.size, 10); // "test-value" length

    // Verify stats updated / 验证统计信息已更新
    let stats = service.get_stats().await;
    assert_eq!(stats.object_count, 1);
    assert_eq!(stats.total_size, 10);
}

#[tokio::test]
async fn test_get_object() {
    // Test getting an object / 测试获取对象
    let service = create_test_service();

    // Put an object first / 先存储一个对象
    let put_request = Request::new(PutObjectRequest {
        key: "test-key".to_string(),
        value: b"test-value".to_vec(),
        metadata: create_test_metadata(),
        overwrite: false,
    });
    service.put_object(put_request).await.unwrap();

    // Get the object / 获取对象
    let get_request = Request::new(GetObjectRequest {
        key: "test-key".to_string(),
        include_value: true,
    });

    let response = service.get_object(get_request).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    assert!(response.found);
    assert!(response.object.is_some());

    let object = response.object.unwrap();
    assert_eq!(object.key, "test-key");
    assert_eq!(object.value, b"test-value");
    assert_eq!(object.size, 10);
}

#[tokio::test]
async fn test_get_nonexistent_object() {
    // Test getting a nonexistent object / 测试获取不存在的对象
    let service = create_test_service();

    let request = Request::new(GetObjectRequest {
        key: "nonexistent-key".to_string(),
        include_value: true,
    });

    let response = service.get_object(request).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    assert!(!response.found);
    assert!(response.object.is_none());
}

#[tokio::test]
async fn test_list_objects() {
    // Test listing objects / 测试列出对象
    let service = create_test_service();

    // Put multiple objects / 存储多个对象
    for i in 0..3 {
        let put_request = Request::new(PutObjectRequest {
            key: format!("test-key-{}", i),
            value: format!("test-value-{}", i).into_bytes(),
            metadata: create_test_metadata(),
            overwrite: false,
        });
        service.put_object(put_request).await.unwrap();
    }

    // List objects / 列出对象
    let list_request = Request::new(ListObjectsRequest {
        prefix: "test-key".to_string(),
        limit: 10,
        start_after: "".to_string(),
        include_values: true,
    });

    let response = service.list_objects(list_request).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    assert_eq!(response.objects.len(), 3);
    assert!(!response.has_more);
}

#[tokio::test]
async fn test_add_object_ref() {
    // Test adding object reference / 测试添加对象引用
    let service = create_test_service();

    // Put an object / 存储一个对象
    let put_request = Request::new(PutObjectRequest {
        key: "test-key".to_string(),
        value: b"test-value".to_vec(),
        metadata: create_test_metadata(),
        overwrite: false,
    });
    service.put_object(put_request).await.unwrap();

    // Add reference / 添加引用
    let ref_request = Request::new(AddObjectRefRequest {
        key: "test-key".to_string(),
        count: 1,
    });

    let response = service.add_object_ref(ref_request).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    assert!(response.success);
    assert_eq!(response.new_ref_count, 2); // Initial ref_count is 1
}

#[tokio::test]
async fn test_remove_object_ref() {
    // Test removing object reference / 测试移除对象引用
    let service = create_test_service();

    // Put an object / 存储一个对象
    let put_request = Request::new(PutObjectRequest {
        key: "test-key".to_string(),
        value: b"test-value".to_vec(),
        metadata: create_test_metadata(),
        overwrite: false,
    });
    service.put_object(put_request).await.unwrap();

    // Add reference first / 首先添加引用
    let add_ref_request = Request::new(AddObjectRefRequest {
        key: "test-key".to_string(),
        count: 1,
    });
    service.add_object_ref(add_ref_request).await.unwrap();

    // Remove reference / 移除引用
    let remove_ref_request = Request::new(RemoveObjectRefRequest {
        key: "test-key".to_string(),
        count: 1,
    });

    let response = service.remove_object_ref(remove_ref_request).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    assert!(response.success);
    assert_eq!(response.new_ref_count, 1); // Should be back to 1
}

#[tokio::test]
async fn test_pin_object() {
    // Test pinning an object / 测试固定对象
    let service = create_test_service();

    // Put an object / 存储一个对象
    let put_request = Request::new(PutObjectRequest {
        key: "test-key".to_string(),
        value: b"test-value".to_vec(),
        metadata: create_test_metadata(),
        overwrite: false,
    });
    service.put_object(put_request).await.unwrap();

    // Pin the object / 固定对象
    let pin_request = Request::new(PinObjectRequest {
        key: "test-key".to_string(),
    });

    let response = service.pin_object(pin_request).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    assert!(response.success);
}

#[tokio::test]
async fn test_delete_object() {
    // Test deleting an object / 测试删除对象
    let service = create_test_service();

    // Put an object / 存储一个对象
    let put_request = Request::new(PutObjectRequest {
        key: "test-key".to_string(),
        value: b"test-value".to_vec(),
        metadata: create_test_metadata(),
        overwrite: false,
    });
    service.put_object(put_request).await.unwrap();

    // Delete the object / 删除对象
    let delete_request = Request::new(DeleteObjectRequest {
        key: "test-key".to_string(),
        force: false,
    });

    let response = service.delete_object(delete_request).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    assert!(response.success);
}
