use crate::spearlet::execution::ExecutionError;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

pub trait SpearHostApi: Send + Sync {
    fn log(&self, level: &str, message: &str);
    fn time_now_ms(&self) -> u64;
    fn random_bytes(&self, len: usize) -> Vec<u8>;
    fn get_env(&self, key: &str) -> Option<String>;
    fn http_call(
        &self,
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<(i32, Vec<u8>, HashMap<String, String>), ExecutionError>;
    fn put_result(
        &self,
        task_id: &str,
        data: Vec<u8>,
        metadata: HashMap<String, String>,
    ) -> Result<String, ExecutionError>;
    fn get_object(&self, id: &str) -> Result<Vec<u8>, ExecutionError>;
    fn put_object(&self, name: &str, bytes: Vec<u8>) -> Result<String, ExecutionError>;
}

#[derive(Clone, Debug)]
pub struct DefaultHostApi {
    runtime_config: super::runtime::RuntimeConfig,
    chat_state: Arc<Mutex<ChatHostState>>,
}

impl DefaultHostApi {
    pub fn new(runtime_config: super::runtime::RuntimeConfig) -> Self {
        Self {
            runtime_config,
            chat_state: Arc::new(Mutex::new(ChatHostState::new())),
        }
    }

    pub fn cchat_create(&self) -> i32 {
        let mut st = match self.chat_state.lock() {
            Ok(v) => v,
            Err(_) => return -5,
        };
        st.create_session()
    }

    pub fn cchat_write_msg(&self, fd: i32, role: String, content: String) -> i32 {
        let mut st = match self.chat_state.lock() {
            Ok(v) => v,
            Err(_) => return -5,
        };
        st.write_msg(fd, role, content)
    }

    pub fn cchat_write_fn(&self, fd: i32, fn_offset: i32, fn_json: String) -> i32 {
        let mut st = match self.chat_state.lock() {
            Ok(v) => v,
            Err(_) => return -5,
        };
        st.write_fn(fd, fn_offset, fn_json)
    }

    pub fn cchat_ctl_set_param(&self, fd: i32, key: String, value: serde_json::Value) -> i32 {
        let mut st = match self.chat_state.lock() {
            Ok(v) => v,
            Err(_) => return -5,
        };
        st.set_param(fd, key, value)
    }

    pub fn cchat_ctl_get_metrics(&self, fd: i32) -> Result<Vec<u8>, i32> {
        let st = match self.chat_state.lock() {
            Ok(v) => v,
            Err(_) => return Err(-5),
        };
        st.get_metrics(fd)
    }

    pub fn cchat_send(&self, fd: i32, flags: i32) -> Result<i32, i32> {
        let metrics_enabled = (flags & 1) != 0;
        let (sess, resp_fd) = {
            let mut st = match self.chat_state.lock() {
                Ok(v) => v,
                Err(_) => return Err(-5),
            };
            let sess = st.get_session(fd)?;
            let resp_fd = st.create_response_fd();
            (sess, resp_fd)
        };

        let last_user = sess
            .messages
            .iter()
            .rev()
            .find(|(r, _)| r.eq_ignore_ascii_case("user"))
            .map(|(_, c)| c.clone())
            .unwrap_or_default();

        let assistant_content = if last_user.is_empty() {
            "stub chat completion".to_string()
        } else {
            format!("stub chat completion: {}", last_user)
        };

        let model = sess
            .params
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("stub-model")
            .to_string();

        let response_json = json!({
            "id": format!("chatcmpl_{}", fd),
            "object": "chat.completion",
            "created": (SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64),
            "model": model,
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": assistant_content,
                    },
                    "finish_reason": "stop"
                }
            ]
        });
        let bytes = serde_json::to_vec(&response_json).map_err(|_| -5)?;
        let metrics_bytes = if metrics_enabled {
            let usage = json!({
                "prompt_tokens": sess.messages.len() as i64,
                "completion_tokens": 1,
                "total_tokens": (sess.messages.len() as i64) + 1,
            });
            serde_json::to_vec(&usage).unwrap_or_else(|_| b"{}".to_vec())
        } else {
            Vec::new()
        };

        let mut st = match self.chat_state.lock() {
            Ok(v) => v,
            Err(_) => return Err(-5),
        };
        st.put_response(resp_fd, bytes, metrics_bytes);
        Ok(resp_fd)
    }

    pub fn cchat_recv(&self, response_fd: i32) -> Result<Vec<u8>, i32> {
        let st = match self.chat_state.lock() {
            Ok(v) => v,
            Err(_) => return Err(-5),
        };
        st.get_response_bytes(response_fd)
    }

    pub fn cchat_close(&self, fd: i32) -> i32 {
        let mut st = match self.chat_state.lock() {
            Ok(v) => v,
            Err(_) => return -5,
        };
        st.close_fd(fd)
    }
}

#[derive(Default, Clone, Debug)]
struct ChatSession {
    messages: Vec<(String, String)>,
    tools: Vec<(i32, String)>,
    params: HashMap<String, serde_json::Value>,
}

#[derive(Default, Clone, Debug)]
struct ChatResponse {
    bytes: Vec<u8>,
    metrics_bytes: Vec<u8>,
}

#[derive(Default, Debug)]
struct ChatHostState {
    next_fd: i32,
    sessions: HashMap<i32, ChatSession>,
    responses: HashMap<i32, ChatResponse>,
}

impl ChatHostState {
    fn new() -> Self {
        Self {
            next_fd: 1000,
            sessions: HashMap::new(),
            responses: HashMap::new(),
        }
    }

    fn create_session(&mut self) -> i32 {
        let fd = self.next_fd;
        self.next_fd += 1;
        self.sessions.insert(fd, ChatSession::default());
        fd
    }

    fn create_response_fd(&mut self) -> i32 {
        let fd = self.next_fd;
        self.next_fd += 1;
        fd
    }

    fn get_session(&self, fd: i32) -> Result<ChatSession, i32> {
        self.sessions.get(&fd).cloned().ok_or(-1)
    }

    fn write_msg(&mut self, fd: i32, role: String, content: String) -> i32 {
        let Some(sess) = self.sessions.get_mut(&fd) else {
            return -1;
        };
        sess.messages.push((role, content));
        0
    }

    fn write_fn(&mut self, fd: i32, fn_offset: i32, fn_json: String) -> i32 {
        let Some(sess) = self.sessions.get_mut(&fd) else {
            return -1;
        };
        sess.tools.push((fn_offset, fn_json));
        0
    }

    fn set_param(&mut self, fd: i32, key: String, value: serde_json::Value) -> i32 {
        if let Some(sess) = self.sessions.get_mut(&fd) {
            sess.params.insert(key, value);
            return 0;
        }
        if self.responses.contains_key(&fd) {
            return 0;
        }
        -1
    }

    fn put_response(&mut self, response_fd: i32, bytes: Vec<u8>, metrics_bytes: Vec<u8>) {
        self.responses.insert(
            response_fd,
            ChatResponse {
                bytes,
                metrics_bytes,
            },
        );
    }

    fn get_response_bytes(&self, response_fd: i32) -> Result<Vec<u8>, i32> {
        self.responses
            .get(&response_fd)
            .map(|r| r.bytes.clone())
            .ok_or(-1)
    }

    fn get_metrics(&self, response_fd: i32) -> Result<Vec<u8>, i32> {
        self.responses
            .get(&response_fd)
            .map(|r| {
                if r.metrics_bytes.is_empty() {
                    b"{}".to_vec()
                } else {
                    r.metrics_bytes.clone()
                }
            })
            .ok_or(-1)
    }

    fn close_fd(&mut self, fd: i32) -> i32 {
        let removed = self.sessions.remove(&fd).is_some() || self.responses.remove(&fd).is_some();
        if removed {
            0
        } else {
            -1
        }
    }
}

impl SpearHostApi for DefaultHostApi {
    fn log(&self, level: &str, message: &str) {
        match level.to_ascii_lowercase().as_str() {
            "debug" => debug!("{}", message),
            "info" => info!("{}", message),
            "warn" => warn!("{}", message),
            _ => info!("{}", message),
        }
    }

    fn time_now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn random_bytes(&self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            out.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
        }
        out.truncate(len);
        out
    }

    fn get_env(&self, key: &str) -> Option<String> {
        self.runtime_config.global_environment.get(key).cloned()
    }

    fn http_call(
        &self,
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<(i32, Vec<u8>, HashMap<String, String>), ExecutionError> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| ExecutionError::RuntimeError {
            message: e.to_string(),
        })?;
        rt.block_on(async move {
            let client = reqwest::Client::new();
            let mut req = match method.to_ascii_uppercase().as_str() {
                "GET" => client.get(url),
                "POST" => client.post(url),
                "PUT" => client.put(url),
                "DELETE" => client.delete(url),
                _ => {
                    return Err(ExecutionError::InvalidRequest {
                        message: format!("Unsupported method: {}", method),
                    })
                }
            };
            for (k, v) in headers.iter() {
                req = req.header(k, v);
            }
            let resp =
                if method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("DELETE") {
                    req.send().await.map_err(|e| ExecutionError::RuntimeError {
                        message: e.to_string(),
                    })?
                } else {
                    req.body(body)
                        .send()
                        .await
                        .map_err(|e| ExecutionError::RuntimeError {
                            message: e.to_string(),
                        })?
                };
            let status = resp.status().as_u16() as i32;
            let mut out_headers = HashMap::new();
            for (k, v) in resp.headers().iter() {
                out_headers.insert(k.to_string(), v.to_str().unwrap_or("").to_string());
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| ExecutionError::RuntimeError {
                    message: e.to_string(),
                })?
                .to_vec();
            Ok((status, bytes, out_headers))
        })
    }

    fn put_result(
        &self,
        task_id: &str,
        data: Vec<u8>,
        metadata: HashMap<String, String>,
    ) -> Result<String, ExecutionError> {
        let name = format!("{}-{}.bin", task_id, self.time_now_ms());
        let id = self.put_object(&name, data)?;
        let result_uri = format!("sms+file://{}", id);
        let addr = self
            .runtime_config
            .spearlet_config
            .as_ref()
            .map(|c| c.sms_grpc_addr.clone())
            .unwrap_or_else(|| "127.0.0.1:50051".to_string());
        let url = format!("http://{}", addr);
        let completed_at = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()) as i64;
        let req = crate::proto::sms::UpdateTaskResultRequest {
            task_id: task_id.to_string(),
            result_uri: result_uri.clone(),
            result_status: "completed".to_string(),
            completed_at,
            result_metadata: metadata,
        };
        let rt = tokio::runtime::Runtime::new().map_err(|e| ExecutionError::RuntimeError {
            message: e.to_string(),
        })?;
        rt.block_on(async move {
            match tonic::transport::Channel::from_shared(url)
                .unwrap()
                .connect()
                .await
            {
                Ok(channel) => {
                    let mut client =
                        crate::proto::sms::task_service_client::TaskServiceClient::new(channel);
                    if let Err(e) = client.update_task_result(req).await {
                        return Err(ExecutionError::RuntimeError {
                            message: e.to_string(),
                        });
                    }
                }
                Err(e) => {
                    return Err(ExecutionError::RuntimeError {
                        message: e.to_string(),
                    })
                }
            }
            Ok(())
        })?;
        Ok(result_uri)
    }

    fn get_object(&self, id: &str) -> Result<Vec<u8>, ExecutionError> {
        let addr = self
            .runtime_config
            .spearlet_config
            .as_ref()
            .map(|c| c.sms_http_addr.clone())
            .ok_or_else(|| ExecutionError::InvalidConfiguration {
                message: "Missing SpearletConfig.sms_http_addr".to_string(),
            })?;
        let path = format!("/api/v1/files/{}", id);
        let rt = tokio::runtime::Runtime::new().map_err(|e| ExecutionError::RuntimeError {
            message: e.to_string(),
        })?;
        let bytes = rt.block_on(async move {
            crate::spearlet::execution::artifact_fetch::fetch_sms_file(&addr, &path).await
        })?;
        Ok(bytes)
    }

    fn put_object(&self, name: &str, bytes: Vec<u8>) -> Result<String, ExecutionError> {
        let addr = self
            .runtime_config
            .spearlet_config
            .as_ref()
            .map(|c| c.sms_http_addr.clone())
            .ok_or_else(|| ExecutionError::InvalidConfiguration {
                message: "Missing SpearletConfig.sms_http_addr".to_string(),
            })?;
        let url = format!("http://{}/admin/api/files", addr);
        let rt = tokio::runtime::Runtime::new().map_err(|e| ExecutionError::RuntimeError {
            message: e.to_string(),
        })?;
        let id = rt.block_on(async move {
            let client = reqwest::Client::new();
            let resp = client
                .post(&url)
                .header("content-type", "application/octet-stream")
                .header("x-file-name", name)
                .body(bytes)
                .send()
                .await
                .map_err(|e| ExecutionError::RuntimeError {
                    message: e.to_string(),
                })?;
            if !resp.status().is_success() {
                return Err(ExecutionError::RuntimeError {
                    message: format!("Upload failed: {}", resp.status()),
                });
            }
            let v: serde_json::Value =
                resp.json()
                    .await
                    .map_err(|e| ExecutionError::RuntimeError {
                        message: e.to_string(),
                    })?;
            let id = v
                .get("id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                return Err(ExecutionError::RuntimeError {
                    message: "Upload response missing id".to_string(),
                });
            }
            Ok::<String, ExecutionError>(id)
        })?;
        Ok(id)
    }
}
