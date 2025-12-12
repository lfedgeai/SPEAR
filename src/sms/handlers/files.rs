use crate::sms::gateway::GatewayState;
use axum::body::Body;
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
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

const FILES_DIR: &str = "./data/files";

#[derive(Deserialize)]
pub struct PresignUploadRequest {
    pub bucket: Option<String>,
    pub key: Option<String>,
    pub content_type: Option<String>,
    pub max_size_bytes: Option<u64>,
}

pub async fn presign_upload(
    Json(_req): Json<PresignUploadRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(
        json!({ "upload_url": "/admin/api/files", "method": "POST", "expires_in": 900 }),
    ))
}

pub async fn upload_file(
    State(state): State<GatewayState>,
    req: axum::http::Request<Body>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    fs::create_dir_all(FILES_DIR)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let id = Uuid::new_v4().to_string();
    let path = std::path::Path::new(FILES_DIR).join(&id);
    let headers = file_headers_from_request(&req);
    let mut file = fs::File::create(&path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let body = req.into_body();
    let bytes = axum::body::to_bytes(body, state.max_upload_bytes)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    file.write_all(&bytes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_secs();
    let meta_path = std::path::Path::new(FILES_DIR).join(format!("{}.json", id));
    let meta = json!({
        "id": id,
        "name": headers.file_name,
        "content_type": headers.content_type,
        "len": bytes.len(),
        "created_at": created_at,
    });
    fs::write(
        &meta_path,
        serde_json::to_vec(&meta).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        json!({ "success": true, "id": id, "name": headers.file_name, "uri": format!("sms+file://{}", id) }),
    ))
}

pub async fn download_file(Path(id): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    let path = std::path::Path::new(FILES_DIR).join(&id);
    let file = fs::File::open(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let stream = ReaderStream::new(file);
    let resp = axum::response::Response::builder()
        .header("Content-Type", "application/octet-stream")
        .body(Body::from_stream(stream))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(resp)
}

pub async fn delete_file(Path(id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    let path = std::path::Path::new(FILES_DIR).join(&id);
    fs::remove_file(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let meta_path = std::path::Path::new(FILES_DIR).join(format!("{}.json", id));
    let _ = fs::remove_file(&meta_path).await;
    Ok(Json(json!({ "success": true })))
}

pub async fn get_file_meta(Path(id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    let path = std::path::Path::new(FILES_DIR).join(&id);
    match fs::metadata(&path).await {
        Ok(meta) => {
            let meta_path = std::path::Path::new(FILES_DIR).join(format!("{}.json", id));
            let mut obj = json!({ "found": true, "len": meta.len() });
            if let Ok(bytes) = fs::read(&meta_path).await {
                if let Ok(m) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if let Some(name) = m.get("name").and_then(|v| v.as_str()) {
                        obj["name"] = json!(name);
                    }
                    if let Some(ct) = m.get("content_type").and_then(|v| v.as_str()) {
                        obj["content_type"] = json!(ct);
                    }
                    if let Some(ts) = m.get("created_at").and_then(|v| v.as_u64()) {
                        obj["created_at"] = json!(ts);
                    }
                }
            }
            Ok(Json(obj))
        }
        Err(_) => Ok(Json(json!({ "found": false }))),
    }
}

pub async fn list_files() -> Result<Json<serde_json::Value>, StatusCode> {
    fs::create_dir_all(FILES_DIR)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut rd = fs::read_dir(FILES_DIR)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut items = Vec::new();
    while let Some(ent) = rd
        .next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        if let Ok(meta) = ent.metadata().await {
            if let Some(fname) = ent.file_name().to_str() {
                // Skip sidecar meta files
                if fname.ends_with(".json") {
                    continue;
                }
                let modified_at = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                // Try read sidecar name
                let meta_path = std::path::Path::new(FILES_DIR).join(format!("{}.json", fname));
                let mut name_field: Option<String> = None;
                let mut created_at: Option<u64> = None;
                if let Ok(bytes) = fs::read(&meta_path).await {
                    if let Ok(m) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        name_field = m
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        created_at = m.get("created_at").and_then(|v| v.as_u64());
                    }
                }
                items.push(json!({ "id": fname, "name": name_field, "len": meta.len(), "modified_at": created_at.unwrap_or(modified_at) }));
            }
        }
    }
    items.sort_by(|a, b| {
        b.get("modified_at")
            .and_then(|v| v.as_u64())
            .cmp(&a.get("modified_at").and_then(|v| v.as_u64()))
    });
    Ok(Json(json!({ "files": items })))
}

struct UploadHeaders {
    file_name: String,
    content_type: String,
}

fn file_headers_from_request(req: &axum::http::Request<Body>) -> UploadHeaders {
    let headers = req.headers();
    let file_name = headers
        .get(HeaderName::from_static("x-file-name"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("blob")
        .to_string();
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    UploadHeaders {
        file_name,
        content_type,
    }
}
