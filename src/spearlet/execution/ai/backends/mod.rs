pub mod http_json;
pub mod chat_http;
pub mod ollama_chat;
pub mod rt_asr_openai_ws;
pub mod rt_asr_qwen_ws;
pub mod stub;

pub use crate::ai_backend_types::{
    BACKEND_FAMILY_CHAT_HTTP as KIND_CHAT_HTTP, BACKEND_FAMILY_OLLAMA_CHAT as KIND_OLLAMA_CHAT,
    BACKEND_FAMILY_RT_ASR_OPENAI_WS as KIND_RT_ASR_OPENAI_WS,
    BACKEND_FAMILY_RT_ASR_QWEN_WS as KIND_RT_ASR_QWEN_WS, BACKEND_FAMILY_STUB as KIND_STUB,
};

use crate::spearlet::execution::ai::ir::{
    CanonicalError, CanonicalRequestEnvelope, CanonicalResponseEnvelope,
};
use crate::spearlet::execution::ai::streaming::StreamingPlan;

pub trait BackendAdapter: Send + Sync {
    fn name(&self) -> &str;

    fn invoke(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<CanonicalResponseEnvelope, CanonicalError>;

    fn streaming_plan(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<StreamingPlan, CanonicalError> {
        Err(CanonicalError {
            code: "unsupported_streaming".to_string(),
            message: "streaming not supported".to_string(),
            retryable: false,
            operation: Some(req.operation.clone()),
        })
    }
}
