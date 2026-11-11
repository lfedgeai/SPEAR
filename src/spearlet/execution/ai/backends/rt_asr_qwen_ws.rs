//! Qwen (DashScope / Model Studio) realtime ASR WebSocket backend adapter.
//! Qwen（百炼）实时 ASR WebSocket 后端适配器。
//!
//! This adapter only produces a `StreamingPlan`; the actual WebSocket data
//! plane (audio append / commit / server events) is shared with the generic
//! RTASR executor, identical to the OpenAI realtime adapter. It hides all
//! provider-specific wiring (URL shape, flat `session.update` layout, auth
//! header) and exposes only user-facing knobs via the request payload.
//!
//! 该适配器仅生成 `StreamingPlan`；真正的 WebSocket 数据面（音频 append /
//! commit / 服务端事件）与通用 RTASR 执行器共享，与 OpenAI realtime 适配器
//! 完全一致。它隐藏所有服务商相关细节（URL 形态、扁平的 `session.update`
//! 结构、鉴权头），只通过请求 payload 暴露用户需要的配置项。

use crate::spearlet::ai::credential_resolver::{CredentialResolution, CredentialResolver};
use crate::spearlet::execution::ai::backends::BackendAdapter;
use crate::spearlet::execution::ai::ir::{
    CanonicalError, CanonicalRequestEnvelope, CanonicalResponseEnvelope, Operation, Payload,
};
use crate::spearlet::execution::ai::streaming::{
    StreamingPlan, StreamingWebsocketPlan, WebsocketFeatures, WebsocketPlan,
};
use url::Url;

/// Default Qwen realtime ASR model when the caller does not specify one.
/// 调用方未指定时使用的默认 Qwen 实时 ASR 模型。
const DEFAULT_MODEL: &str = "qwen3-asr-flash-realtime";

/// Default input sample rate (Hz) matching the Qwen API default.
/// 与 Qwen API 默认值一致的默认输入采样率（Hz）。
const DEFAULT_SAMPLE_RATE: u32 = 16000;

/// Qwen realtime ASR WebSocket backend adapter.
/// Qwen 实时 ASR WebSocket 后端适配器。
pub struct QwenAsrRealtimeWsBackendAdapter {
    name: String,
    base_url: String,
    credential_ref: Option<String>,
    credential_resolver: Option<CredentialResolver>,
}

impl QwenAsrRealtimeWsBackendAdapter {
    /// Create a new adapter instance.
    /// 创建新的适配器实例。
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        credential_ref: Option<String>,
        credential_resolver: Option<CredentialResolver>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            credential_ref: credential_ref.and_then(|s| {
                let t = s.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            }),
            credential_resolver,
        }
    }

    /// Resolve the API key state via the shared credential resolver.
    /// 通过共享凭据解析器解析 API key 状态。
    fn resolve_api_key_state(&self) -> CredentialResolution {
        let Some(resolver) = self.credential_resolver.as_ref() else {
            return CredentialResolution::Missing;
        };
        resolver.resolve_api_key_state(self.credential_ref.as_deref())
    }
}

impl BackendAdapter for QwenAsrRealtimeWsBackendAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn invoke(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<CanonicalResponseEnvelope, CanonicalError> {
        // Qwen realtime ASR is streaming-only; non-streaming invoke is unsupported.
        // Qwen 实时 ASR 仅支持流式；不支持非流式 invoke。
        Err(CanonicalError {
            code: "unsupported_operation".to_string(),
            message: format!(
                "rt_asr_qwen_ws is streaming-only; invoke() is unsupported for {:?}",
                req.operation
            ),
            retryable: false,
            operation: Some(req.operation.clone()),
        })
    }

    fn streaming_plan(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<StreamingPlan, CanonicalError> {
        if req.operation != Operation::SpeechToText {
            return Err(CanonicalError {
                code: "unsupported_operation".to_string(),
                message:
                    "rt_asr_qwen_ws only supports speech_to_text streaming"
                        .to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        // Extract user-facing knobs (model + sample_rate) from the payload.
        // 从 payload 中提取用户可配置项（model + sample_rate）。
        let (model, sample_rate) = match &req.payload {
            Payload::SpeechToText(p) => (
                p.model
                    .clone()
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
                p.sample_rate.unwrap_or(DEFAULT_SAMPLE_RATE),
            ),
            _ => (DEFAULT_MODEL.to_string(), DEFAULT_SAMPLE_RATE),
        };

        let ws_url = derive_qwen_realtime_ws_url(&self.base_url, &model)?;

        // Resolve credential and inject the Authorization header when present.
        // 解析凭据，并在存在时注入 Authorization 头。
        let mut headers = Vec::new();
        let credential_state = self.resolve_api_key_state();
        let api_key = match credential_state {
            CredentialResolution::Ready(secret) => Some(secret),
            CredentialResolution::Disabled
            | CredentialResolution::NotSynced
            | CredentialResolution::Missing
                if self.credential_ref.is_some() =>
            {
                return Err(CanonicalError {
                    code: credential_state.code().to_string(),
                    message: credential_state
                        .message(self.credential_ref.as_deref().unwrap_or_default()),
                    retryable: !matches!(credential_state, CredentialResolution::Disabled),
                    operation: Some(Operation::SpeechToText),
                });
            }
            CredentialResolution::Disabled
            | CredentialResolution::NotSynced
            | CredentialResolution::Missing => None,
        };
        if let Some(api_key) = api_key.as_deref() {
            headers.push(("authorization".to_string(), format!("Bearer {}", api_key)));
        }

        // Build the initial `session.update` event using Qwen's flat layout.
        // `turn_detection` is intentionally set to null here and later rewritten
        // by the RTASR segmentation layer (server_vad / manual) via the shared
        // `apply_turn_detection_to_client_events`.
        //
        // 使用 Qwen 的扁平结构构建初始 `session.update` 事件。此处 `turn_detection`
        // 刻意置为 null，随后由 RTASR 分段层（server_vad / manual）通过共享的
        // `apply_turn_detection_to_client_events` 改写。
        let session_update = serde_json::json!({
            "type": "session.update",
            "session": {
                "input_audio_format": "pcm",
                "sample_rate": sample_rate,
                "input_audio_transcription": {},
                "turn_detection": serde_json::Value::Null,
            }
        });

        Ok(StreamingPlan::Websocket(StreamingWebsocketPlan {
            prepare: Vec::new(),
            websocket: WebsocketPlan {
                url: ws_url,
                headers,
                client_events: vec![session_update],
                features: WebsocketFeatures {
                    turn_detection: true,
                },
            },
        }))
    }
}

/// Derive the Qwen realtime ASR WebSocket URL from a configured base URL.
///
/// - `https`/`http` schemes are upgraded to `wss`/`ws`.
/// - The canonical path `/api-ws/v1/realtime` is appended when missing.
/// - OpenAI-compatible HTTP base paths such as `/compatible-mode/v1` are
///   normalized back to the host root before the realtime path is applied.
/// - The `model` query parameter is set (replacing any existing one).
///
/// 根据配置的 base URL 推导 Qwen 实时 ASR WebSocket URL。
///
/// - `https`/`http` 协议升级为 `wss`/`ws`。
/// - 缺失时补齐规范路径 `/api-ws/v1/realtime`。
/// - 将 `/compatible-mode/v1` 这类 OpenAI-compatible HTTP 路径先归一化到根路径，
///   再拼接 realtime 路径。
/// - 设置 `model` 查询参数（覆盖已有值）。
fn derive_qwen_realtime_ws_url(base_url: &str, model: &str) -> Result<String, CanonicalError> {
    let mut u = Url::parse(base_url).map_err(|e| CanonicalError {
        code: "invalid_configuration".to_string(),
        message: format!("invalid base_url: {e}"),
        retryable: false,
        operation: None,
    })?;

    let scheme = match u.scheme() {
        "https" => "wss",
        "http" => "ws",
        "wss" => "wss",
        "ws" => "ws",
        s => {
            return Err(CanonicalError {
                code: "invalid_configuration".to_string(),
                message: format!("unsupported base_url scheme: {s}"),
                retryable: false,
                operation: None,
            })
        }
    };
    u.set_scheme(scheme).map_err(|_| CanonicalError {
        code: "invalid_configuration".to_string(),
        message: "set scheme failed".to_string(),
        retryable: false,
        operation: None,
    })?;

    let normalized_path = normalize_qwen_realtime_base_path(u.path());
    if normalized_path.ends_with("/api-ws/v1/realtime") {
        u.set_path(&normalized_path);
    } else {
        u.set_path(&format!("{normalized_path}/api-ws/v1/realtime"));
    }

    // Set the model query parameter, replacing any existing one.
    // 设置 model 查询参数，覆盖已有值。
    let mut pairs: Vec<(String, String)> = u
        .query_pairs()
        .filter(|(k, _)| k != "model")
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    pairs.push(("model".to_string(), model.to_string()));
    u.query_pairs_mut()
        .clear()
        .extend_pairs(pairs.iter().map(|(k, v)| (&**k, &**v)));
    Ok(u.to_string())
}

/// Normalize user-facing Qwen base URLs into the root that should own the
/// realtime endpoint path.
/// 将用户配置的 Qwen base URL 归一化到可承载 realtime endpoint 的根路径。
fn normalize_qwen_realtime_base_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }

    // Some users copy the OpenAI-compatible HTTP base URL from provider docs.
    // Realtime ASR is hosted on the same origin but not under `/compatible-mode/v1`.
    // 一些用户会直接复用 OpenAI-compatible HTTP 的 base URL。
    // realtime ASR 与其同域，但不挂在 `/compatible-mode/v1` 路径下。
    if trimmed.ends_with("/compatible-mode/v1") {
        let base = trimmed
            .strip_suffix("/compatible-mode/v1")
            .unwrap_or(trimmed);
        return base.to_string();
    }

    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::sms::CredentialMaterial;
    use crate::spearlet::config::SpearletConfig;
    use crate::spearlet::execution::ai::ir::SpeechToTextPayload;

    fn s2t_req(model: Option<String>, sample_rate: Option<u32>) -> CanonicalRequestEnvelope {
        CanonicalRequestEnvelope {
            version: 1,
            request_id: "r1".to_string(),
            operation: Operation::SpeechToText,
            meta: Default::default(),
            routing: Default::default(),
            requirements: Default::default(),
            timeout_ms: None,
            payload: Payload::SpeechToText(SpeechToTextPayload { model, sample_rate }),
            extra: Default::default(),
        }
    }

    #[test]
    fn derive_url_upgrades_scheme_and_appends_path_and_model() {
        let url = derive_qwen_realtime_ws_url(
            "https://ws-id.cn-beijing.maas.aliyuncs.com",
            "qwen3-asr-flash-realtime",
        )
        .unwrap();
        assert!(url.starts_with("wss://ws-id.cn-beijing.maas.aliyuncs.com/api-ws/v1/realtime"));
        assert!(url.contains("model=qwen3-asr-flash-realtime"));
    }

    #[test]
    fn derive_url_keeps_existing_realtime_path() {
        let url = derive_qwen_realtime_ws_url(
            "wss://ws-id.ap-southeast-1.maas.aliyuncs.com/api-ws/v1/realtime",
            "m1",
        )
        .unwrap();
        assert!(url.contains("/api-ws/v1/realtime"));
        assert!(url.contains("model=m1"));
        // Path must not be duplicated / 路径不应重复
        assert_eq!(url.matches("/realtime").count(), 1);
    }

    #[test]
    fn derive_url_normalizes_compatible_mode_http_base_url() {
        let url = derive_qwen_realtime_ws_url(
            "https://maas.qianwenaiapi.com/compatible-mode/v1",
            "qwen3-asr-flash-realtime",
        )
        .unwrap();
        assert_eq!(
            url,
            "wss://maas.qianwenaiapi.com/api-ws/v1/realtime?model=qwen3-asr-flash-realtime"
        );
    }

    #[test]
    fn streaming_plan_uses_flat_session_update_with_defaults() {
        let adapter = QwenAsrRealtimeWsBackendAdapter::new(
            "qwen",
            "https://ws-id.cn-beijing.maas.aliyuncs.com",
            None,
            None,
        );
        let plan = adapter.streaming_plan(&s2t_req(None, None)).unwrap();
        let StreamingPlan::Websocket(p) = plan;
        assert!(p.websocket.url.contains("model=qwen3-asr-flash-realtime"));
        assert_eq!(p.websocket.client_events.len(), 1);
        let session = &p.websocket.client_events[0]["session"];
        assert_eq!(session["input_audio_format"], "pcm");
        assert_eq!(session["sample_rate"], 16000);
        assert!(session.get("turn_detection").unwrap().is_null());
        assert!(p.websocket.features.turn_detection);
    }

    #[test]
    fn streaming_plan_respects_sample_rate_override() {
        let adapter = QwenAsrRealtimeWsBackendAdapter::new(
            "qwen",
            "https://ws-id.cn-beijing.maas.aliyuncs.com",
            None,
            None,
        );
        let plan = adapter
            .streaming_plan(&s2t_req(Some("m1".to_string()), Some(8000)))
            .unwrap();
        let StreamingPlan::Websocket(p) = plan;
        assert_eq!(p.websocket.client_events[0]["session"]["sample_rate"], 8000);
        assert!(p.websocket.url.contains("model=m1"));
    }

    #[test]
    fn streaming_plan_rejects_non_speech_to_text() {
        let adapter = QwenAsrRealtimeWsBackendAdapter::new(
            "qwen",
            "https://ws-id.cn-beijing.maas.aliyuncs.com",
            None,
            None,
        );
        let mut req = s2t_req(None, None);
        req.operation = Operation::ChatCompletions;
        let err = adapter.streaming_plan(&req).expect_err("should reject");
        assert_eq!(err.code, "unsupported_operation");
    }

    #[test]
    fn streaming_plan_returns_credential_disabled_for_disabled_dynamic_credential() {
        let _guard =
            crate::spearlet::ai::dynamic_credential_store::global_dynamic_credentials_test_lock()
                .lock()
                .expect("lock");
        let store = crate::spearlet::ai::dynamic_credential_store::global_dynamic_credentials();
        store.clear();
        store.set_credentials(vec![CredentialMaterial {
            name: "qwen-realtime".to_string(),
            provider_kind: "inline_encrypted".to_string(),
            secret: "sk-test".to_string(),
            version: 1,
            disabled: true,
            updated_at_ms: 0,
        }]);

        let adapter = QwenAsrRealtimeWsBackendAdapter::new(
            "qwen",
            "https://ws-id.cn-beijing.maas.aliyuncs.com",
            Some("qwen-realtime".to_string()),
            Some(CredentialResolver::from_config(&SpearletConfig::default())),
        );
        let err = adapter
            .streaming_plan(&s2t_req(Some("m1".to_string()), None))
            .expect_err("should fail");
        assert_eq!(err.code, "credential_disabled");
        store.clear();
    }
}
