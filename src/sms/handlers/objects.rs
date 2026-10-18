use crate::sms::blob_store::{BlobMetadata, BlobStoreError, PutBlobRequest};
use crate::sms::gateway::GatewayState;
use axum::body::Body;
use axum::extract::Query;
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

/// Object query parameters for the admin object catalog.
/// 管理台对象目录的查询参数。
#[derive(Deserialize)]
pub struct ListObjectsQuery {
    pub prefix: Option<String>,
    pub limit: Option<usize>,
    pub page_token: Option<String>,
}

/// Upload an object through the admin object API.
/// 通过管理台对象接口上传对象。
pub async fn upload_object(
    State(state): State<GatewayState>,
    req: axum::http::Request<Body>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    upload_blob(state, req, "object").await
}

/// Shared upload path for object APIs.
/// 对象接口复用的上传路径。
async fn upload_blob(
    state: GatewayState,
    req: axum::http::Request<Body>,
    uri_scheme: &str,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let headers = file_headers_from_request(&req);
    let body = req.into_body();
    let bytes = axum::body::to_bytes(body, state.max_upload_bytes)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let store = state.blob_store.clone();
    let created = store
        .put_blob(PutBlobRequest {
            name: Some(headers.file_name.clone()),
            content_type: Some(headers.content_type),
            bytes: bytes.to_vec(),
        })
        .await
        .map_err(blob_store_error_to_status)?;
    Ok(Json(json!({
        "success": true,
        "id": created.key,
        "key": created.key,
        "name": headers.file_name,
        "uri": format!("{}://{}", uri_scheme, created.key),
    })))
}

/// Download object content through the object API.
/// 通过对象 API 下载对象内容。
pub async fn download_object(
    State(state): State<GatewayState>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let key = parse_blob_id(&key)?;
    let store = state.blob_store.clone();
    let content = store
        .get_blob(&key)
        .await
        .map_err(blob_store_error_to_status)?;
    let response = axum::response::Response::builder()
        .header(
            CONTENT_TYPE,
            content
                .content_type
                .as_deref()
                .unwrap_or("application/octet-stream"),
        )
        .body(Body::from(content.bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(response)
}

/// Delete an object through the object API.
/// 通过对象 API 删除对象。
pub async fn delete_object(
    State(state): State<GatewayState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key = parse_blob_id(&key)?;
    let store = state.blob_store.clone();
    match store.delete_blob(&key).await {
        Ok(()) => Ok(Json(json!({ "success": true }))),
        Err(BlobStoreError::Conflict(message)) => {
            Ok(Json(json!({ "success": false, "message": message })))
        }
        Err(error) => Err(blob_store_error_to_status(error)),
    }
}

/// Return object metadata from the admin object API.
/// 通过管理台对象 API 返回对象元数据。
pub async fn get_object_meta(
    State(state): State<GatewayState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key = parse_blob_id(&key)?;
    let store = state.blob_store.clone();
    match store.get_blob_metadata(&key).await {
        Ok(meta) => Ok(Json(blob_metadata_response(meta))),
        Err(BlobStoreError::NotFound) => Ok(Json(json!({ "found": false, "key": key }))),
        Err(error) => Err(blob_store_error_to_status(error)),
    }
}

/// Increase an object's reference count.
/// 增加对象的引用计数。
pub async fn retain_object(
    State(state): State<GatewayState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key = parse_blob_id(&key)?;
    let store = state.blob_store.clone();
    match store.retain_blob(&key).await {
        Ok(updated) => Ok(Json(lifecycle_response(updated))),
        Err(BlobStoreError::Conflict(message)) => {
            Ok(Json(json!({ "success": false, "message": message })))
        }
        Err(error) => Err(blob_store_error_to_status(error)),
    }
}

/// Decrease an object's reference count.
/// 减少对象的引用计数。
pub async fn release_object(
    State(state): State<GatewayState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key = parse_blob_id(&key)?;
    let store = state.blob_store.clone();
    match store.release_blob(&key).await {
        Ok(updated) => Ok(Json(lifecycle_response(updated))),
        Err(BlobStoreError::Conflict(message)) => {
            Ok(Json(json!({ "success": false, "message": message })))
        }
        Err(error) => Err(blob_store_error_to_status(error)),
    }
}

/// Pin an object to prevent deletion.
/// 固定对象以防止删除。
pub async fn pin_object(
    State(state): State<GatewayState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key = parse_blob_id(&key)?;
    let store = state.blob_store.clone();
    match store.pin_blob(&key).await {
        Ok(updated) => Ok(Json(lifecycle_response(updated))),
        Err(BlobStoreError::Conflict(message)) => {
            Ok(Json(json!({ "success": false, "message": message })))
        }
        Err(error) => Err(blob_store_error_to_status(error)),
    }
}

/// Unpin an object so it may be deleted when no longer referenced.
/// 取消固定对象，使其在无引用时可被删除。
pub async fn unpin_object(
    State(state): State<GatewayState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key = parse_blob_id(&key)?;
    let store = state.blob_store.clone();
    match store.unpin_blob(&key).await {
        Ok(updated) => Ok(Json(lifecycle_response(updated))),
        Err(BlobStoreError::Conflict(message)) => {
            Ok(Json(json!({ "success": false, "message": message })))
        }
        Err(error) => Err(blob_store_error_to_status(error)),
    }
}

/// List objects through the admin object API.
/// 通过管理台对象 API 列出对象。
pub async fn list_objects(
    State(state): State<GatewayState>,
    Query(params): Query<ListObjectsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let offset = params
        .page_token
        .as_deref()
        .and_then(|value| value.parse::<usize>().ok());
    let store = state.blob_store.clone();
    let page = store
        .list_blobs(params.prefix.as_deref(), params.limit, offset)
        .await
        .map_err(blob_store_error_to_status)?;
    let consumed = page.items.len();
    let offset = offset.unwrap_or(0);
    let items = page
        .items
        .into_iter()
        .map(|item| {
            json!({
                "key": item.key,
                "name": item.name,
                "size": item.size,
                "updated_at": item.updated_at,
                "state": "ready",
                "ref_count": item.ref_count,
                "pinned": item.pinned,
            })
        })
        .collect::<Vec<_>>();
    let next_page_token = if offset + consumed < page.total_count {
        Some((offset + consumed).to_string())
    } else {
        None
    };
    Ok(Json(json!({
        "items": items,
        "total_count": page.total_count,
        "next_page_token": next_page_token,
    })))
}

/// Return object storage status for the admin UI.
/// 为管理台返回对象存储状态。
pub async fn get_object_storage_status(
    State(state): State<GatewayState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = state.blob_store.clone();
    let status = store.status().await.map_err(blob_store_error_to_status)?;
    Ok(Json(json!({
        "backend": status.backend,
        "healthy": status.healthy,
        "message": status.message,
    })))
}

/// Convert canonical blob metadata into the public object detail payload.
/// 将标准 Blob 元数据转换为公开对象详情载荷。
fn blob_metadata_response(meta: BlobMetadata) -> serde_json::Value {
    json!({
        "found": true,
        "key": meta.key,
        "name": meta.name,
        "content_type": meta.content_type,
        "size": meta.size,
        "state": "ready",
        "created_at": meta.created_at,
        "updated_at": meta.updated_at.or(meta.created_at),
        "ref_count": meta.ref_count,
        "pinned": meta.pinned,
    })
}

/// Convert lifecycle updates into the shared object action payload.
/// 将生命周期更新转换为统一的对象操作响应。
fn lifecycle_response(meta: BlobMetadata) -> serde_json::Value {
    json!({
        "success": true,
        "key": meta.key,
        "ref_count": meta.ref_count,
        "pinned": meta.pinned,
        "state": "ready",
    })
}

/// Translate storage errors into HTTP status codes.
/// 将存储错误翻译为 HTTP 状态码。
fn blob_store_error_to_status(error: BlobStoreError) -> StatusCode {
    match error {
        BlobStoreError::NotFound => StatusCode::NOT_FOUND,
        BlobStoreError::Conflict(_) => StatusCode::CONFLICT,
        BlobStoreError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

struct UploadHeaders {
    file_name: String,
    content_type: String,
}

fn file_headers_from_request(req: &axum::http::Request<Body>) -> UploadHeaders {
    let headers = req.headers();
    let file_name = headers
        .get(HeaderName::from_static("x-file-name"))
        .and_then(|value| value.to_str().ok())
        .unwrap_or("blob")
        .to_string();
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    UploadHeaders {
        file_name,
        content_type,
    }
}

/// Parse the compatibility blob identifier from a file id or object key.
/// 从文件 ID 或对象 key 中解析兼容层的 blob 标识。
fn parse_blob_id(id_or_key: &str) -> Result<String, StatusCode> {
    let id = std::path::Path::new(id_or_key)
        .file_name()
        .and_then(|segment| segment.to_str())
        .ok_or(StatusCode::NOT_FOUND)?;
    Uuid::parse_str(id)
        .map(|uuid| uuid.to_string())
        .map_err(|_| StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_blob_id_accepts_canonical_uuid_keys() {
        let key = Uuid::new_v4().to_string();
        assert_eq!(parse_blob_id(&key).unwrap(), key);
    }

    #[test]
    fn parse_blob_id_rejects_invalid_keys() {
        assert_eq!(parse_blob_id("not-a-uuid"), Err(StatusCode::NOT_FOUND));
    }
}
