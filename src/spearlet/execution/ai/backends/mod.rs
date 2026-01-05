pub mod openai_chat_completion;
pub mod openai_realtime_ws;
pub mod stub;

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
