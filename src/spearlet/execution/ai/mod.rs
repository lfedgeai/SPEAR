pub mod backends;
pub mod ir;
pub mod media_ref;
pub mod normalize;
pub mod router;
pub mod streaming;

use std::fmt;
use std::sync::Arc;

use crate::spearlet::execution::ai::ir::{CanonicalRequestEnvelope, CanonicalResponseEnvelope};
use crate::spearlet::execution::ai::router::Router;
use crate::spearlet::execution::ai::streaming::StreamingInvocation;

#[derive(Clone)]
pub struct AiEngine {
    router: Arc<Router>,
}

impl fmt::Debug for AiEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiEngine").finish()
    }
}

impl AiEngine {
    pub fn new(router: Router) -> Self {
        Self {
            router: Arc::new(router),
        }
    }

    pub fn invoke(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<CanonicalResponseEnvelope, crate::spearlet::execution::ExecutionError> {
        req.validate_basic().map_err(|e| {
            crate::spearlet::execution::ExecutionError::InvalidRequest { message: e.message }
        })?;
        let inst = self.router.route(req).map_err(|e| {
            crate::spearlet::execution::ExecutionError::NotSupported {
                operation: e.message,
            }
        })?;
        inst.adapter.invoke(req).map_err(|e| {
            crate::spearlet::execution::ExecutionError::RuntimeError { message: e.message }
        })
    }

    pub fn invoke_streaming(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<StreamingInvocation, crate::spearlet::execution::ExecutionError> {
        req.validate_basic().map_err(|e| {
            crate::spearlet::execution::ExecutionError::InvalidRequest { message: e.message }
        })?;
        let inst = self.router.route(req).map_err(|e| {
            crate::spearlet::execution::ExecutionError::NotSupported {
                operation: e.message,
            }
        })?;

        let plan = inst.adapter.streaming_plan(req).map_err(|e| {
            crate::spearlet::execution::ExecutionError::NotSupported {
                operation: e.message,
            }
        })?;
        Ok(StreamingInvocation {
            backend: inst.name.clone(),
            plan,
        })
    }
}
