use crate::spearlet::execution::ai::backends::openai_compatible::OpenAICompatibleBackendAdapter;
use crate::spearlet::execution::ai::backends::stub::StubBackendAdapter;
use crate::spearlet::execution::ai::ir::{Operation, ResultPayload};
use crate::spearlet::execution::ai::normalize::chat::normalize_cchat_session;
use crate::spearlet::execution::ai::router::capabilities::Capabilities;
use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
use crate::spearlet::execution::ai::router::registry::{BackendInstance, BackendRegistry};
use crate::spearlet::execution::ai::router::Router;
use crate::spearlet::execution::ai::AiEngine;
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
    ai_engine: Arc<AiEngine>,
}

impl DefaultHostApi {
    pub fn new(runtime_config: super::runtime::RuntimeConfig) -> Self {
        let (registry, policy) = build_registry_from_runtime_config(&runtime_config);
        let router = Router::new(registry, policy);
        let ai_engine = Arc::new(AiEngine::new(router));
        Self {
            runtime_config,
            chat_state: Arc::new(Mutex::new(ChatHostState::new())),
            ai_engine,
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
        let (snapshot, resp_fd) = {
            let mut st = match self.chat_state.lock() {
                Ok(v) => v,
                Err(_) => return Err(-5),
            };
            let snapshot = st.get_session_snapshot(fd)?;
            let resp_fd = st.create_response_fd();
            (snapshot, resp_fd)
        };

        let req = normalize_cchat_session(&snapshot);
        let resp = match self.ai_engine.invoke(&req) {
            Ok(r) => r,
            Err(e) => {
                let body = json!({"error": {"message": e.to_string()}});
                let bytes = serde_json::to_vec(&body).map_err(|_| -5)?;
                let mut st = match self.chat_state.lock() {
                    Ok(v) => v,
                    Err(_) => return Err(-5),
                };
                let metrics_bytes = if metrics_enabled {
                    b"{}".to_vec()
                } else {
                    Vec::new()
                };
                st.put_response(resp_fd, bytes, metrics_bytes);
                return Ok(resp_fd);
            }
        };

        let bytes = match resp.result {
            ResultPayload::Payload(v) => serde_json::to_vec(&v).map_err(|_| -5)?,
            ResultPayload::Error(e) => {
                let body = json!({"error": {"code": e.code, "message": e.message}});
                serde_json::to_vec(&body).map_err(|_| -5)?
            }
        };
        let metrics_bytes = if metrics_enabled {
            let usage = json!({
                "prompt_tokens": snapshot.messages.len() as i64,
                "completion_tokens": 1,
                "total_tokens": (snapshot.messages.len() as i64) + 1,
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

fn build_registry_from_runtime_config(
    runtime_config: &super::runtime::RuntimeConfig,
) -> (BackendRegistry, SelectionPolicy) {
    let mut policy = SelectionPolicy::WeightedRandom;
    let mut instances: Vec<BackendInstance> = Vec::new();

    if let Some(cfg) = runtime_config.spearlet_config.as_ref() {
        if let Some(p) = cfg.llm.default_policy.as_ref() {
            policy = match p.as_str() {
                "weighted_random" => SelectionPolicy::WeightedRandom,
                _ => SelectionPolicy::WeightedRandom,
            };
        }

        for b in cfg.llm.backends.iter() {
            let ops = b
                .ops
                .iter()
                .filter_map(|s| parse_operation(s))
                .collect::<Vec<_>>();
            if ops.is_empty() {
                continue;
            }

            if let Some(env_name) = b.api_key_env.as_ref() {
                if !runtime_config.global_environment.contains_key(env_name) {
                    continue;
                }
            }

            let adapter: Arc<dyn crate::spearlet::execution::ai::backends::BackendAdapter> =
                match b.kind.as_str() {
                    "openai_compatible" => Arc::new(OpenAICompatibleBackendAdapter::new(
                        b.name.clone(),
                        b.base_url.clone(),
                        b.api_key_env.clone(),
                        runtime_config.global_environment.clone(),
                    )),
                    _ => continue,
                };

            instances.push(BackendInstance {
                name: b.name.clone(),
                weight: b.weight,
                priority: b.priority,
                capabilities: Capabilities {
                    ops,
                    features: b.features.clone(),
                    transports: b.transports.clone(),
                },
                adapter,
            });
        }
    }

    if instances.is_empty() {
        let stub = Arc::new(StubBackendAdapter::new("stub"));
        instances.push(BackendInstance {
            name: "stub".to_string(),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![
                    "supports_tools".to_string(),
                    "supports_json_schema".to_string(),
                    "supports_stream".to_string(),
                ],
                transports: vec!["in_process".to_string()],
            },
            adapter: stub,
        });
    }

    (BackendRegistry::new(instances), policy)
}

fn parse_operation(s: &str) -> Option<Operation> {
    match s {
        "chat_completions" => Some(Operation::ChatCompletions),
        "embeddings" => Some(Operation::Embeddings),
        "image_generation" => Some(Operation::ImageGeneration),
        "speech_to_text" => Some(Operation::SpeechToText),
        "text_to_speech" => Some(Operation::TextToSpeech),
        "realtime_voice" => Some(Operation::RealtimeVoice),
        _ => None,
    }
}

#[derive(Default, Clone, Debug)]
struct ChatSession {
    messages: Vec<(String, String)>,
    tools: Vec<(i32, String)>,
    params: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct ChatSessionSnapshot {
    pub fd: i32,
    pub messages: Vec<(String, String)>,
    pub tools: Vec<(i32, String)>,
    pub params: HashMap<String, serde_json::Value>,
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

    fn get_session_snapshot(&self, fd: i32) -> Result<ChatSessionSnapshot, i32> {
        let sess = self.sessions.get(&fd).cloned().ok_or(-1)?;
        Ok(ChatSessionSnapshot {
            fd,
            messages: sess.messages,
            tools: sess.tools,
            params: sess.params,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::runtime::{ResourcePoolConfig, RuntimeConfig, RuntimeType};
    use std::collections::HashMap;

    #[test]
    fn test_cchat_send_pipeline_stub_backend() {
        let api = DefaultHostApi::new(RuntimeConfig {
            runtime_type: RuntimeType::Wasm,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: ResourcePoolConfig::default(),
        });

        let fd = api.cchat_create();
        assert!(fd > 0);
        assert_eq!(
            api.cchat_write_msg(fd, "user".to_string(), "hello".to_string()),
            0
        );
        let resp_fd = api.cchat_send(fd, 0).unwrap();
        let bytes = api.cchat_recv(resp_fd).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
        assert!(content.contains("hello"));
    }

    #[test]
    fn test_configured_openai_backend_missing_key_is_filtered() {
        let mut cfg = crate::spearlet::config::SpearletConfig::default();
        cfg.llm
            .backends
            .push(crate::spearlet::config::LlmBackendConfig {
                name: "openai-us".to_string(),
                kind: "openai_compatible".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_env: Some("OPENAI_API_KEY".to_string()),
                weight: 100,
                priority: 0,
                ops: vec!["chat_completions".to_string()],
                features: vec![],
                transports: vec!["http".to_string()],
            });

        let api = DefaultHostApi::new(RuntimeConfig {
            runtime_type: RuntimeType::Wasm,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: Some(cfg),
            resource_pool: ResourcePoolConfig::default(),
        });

        let fd = api.cchat_create();
        assert_eq!(
            api.cchat_write_msg(fd, "user".to_string(), "hello".to_string()),
            0
        );
        let resp_fd = api.cchat_send(fd, 0).unwrap();
        let bytes = api.cchat_recv(resp_fd).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.get("choices").is_some());
    }
}
