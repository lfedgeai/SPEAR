pub mod openai_compatible;
pub mod stub;

use crate::spearlet::execution::ai::ir::{
    CanonicalError, CanonicalRequestEnvelope, CanonicalResponseEnvelope,
};

pub trait BackendAdapter: Send + Sync {
    fn name(&self) -> &str;

    fn invoke(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<CanonicalResponseEnvelope, CanonicalError>;
}
