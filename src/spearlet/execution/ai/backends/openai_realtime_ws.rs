use crate::spearlet::execution::ai::backends::BackendAdapter;
use crate::spearlet::execution::ai::ir::{
    CanonicalError, CanonicalRequestEnvelope, CanonicalResponseEnvelope,
};
use crate::spearlet::execution::ai::streaming::{
    StreamingPlan, StreamingWebsocketPlan, WebsocketPlan,
};
use url::Url;

pub struct OpenAIRealtimeWsBackendAdapter {
    name: String,
    base_url: String,
    api_key_env: Option<String>,
}

impl OpenAIRealtimeWsBackendAdapter {
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key_env: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            api_key_env,
        }
    }
}

impl BackendAdapter for OpenAIRealtimeWsBackendAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn invoke(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<CanonicalResponseEnvelope, CanonicalError> {
        Err(CanonicalError {
            code: "unsupported_operation".to_string(),
            message: format!(
                "openai_realtime_ws is streaming-only; invoke() is unsupported for {:?}",
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
        if req.operation != crate::spearlet::execution::ai::ir::Operation::SpeechToText {
            return Err(CanonicalError {
                code: "unsupported_operation".to_string(),
                message: "openai_realtime_ws only supports speech_to_text streaming".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        let model = match &req.payload {
            crate::spearlet::execution::ai::ir::Payload::SpeechToText(p) => p
                .model
                .clone()
                .unwrap_or_else(|| "gpt-4o-mini-transcribe".to_string()),
            _ => "gpt-4o-mini-transcribe".to_string(),
        };
        let ws_url = derive_openai_realtime_ws_url(&self.base_url)?;

        Ok(StreamingPlan::Websocket(StreamingWebsocketPlan {
            prepare: Vec::new(),
            websocket: WebsocketPlan {
                url: ws_url,
                headers: vec![
                    (
                        "authorization".to_string(),
                        format!(
                            "Bearer ${{env:{}}}",
                            self.api_key_env.as_deref().unwrap_or("OPENAI_API_KEY")
                        ),
                    ),
                    ("OpenAI-Beta".to_string(), "realtime=v1".to_string()),
                ],
                client_events: vec![serde_json::json!({
                    "type": "session.update",
                    "session": {
                        "input_audio_format": "pcm16",
                        "input_audio_transcription": {
                            "model": model,
                        },
                    }
                })],
                supports_turn_detection: true,
            },
        }))
    }
}

fn derive_openai_realtime_ws_url(base_url: &str) -> Result<String, CanonicalError> {
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

    let mut path = u.path().trim_end_matches('/').to_string();
    if !path.ends_with("/v1") {
        path = format!("{path}/v1");
    }
    u.set_path(&format!("{}/realtime", path));
    u.set_query(Some("model=gpt-realtime"));
    Ok(u.to_string())
}
