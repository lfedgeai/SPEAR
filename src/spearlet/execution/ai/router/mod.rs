pub mod capabilities;
pub mod policy;
pub mod registry;

use crate::spearlet::execution::ai::ir::{CanonicalError, CanonicalRequestEnvelope};
use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
use crate::spearlet::execution::ai::router::registry::{BackendInstance, BackendRegistry};

#[derive(Clone)]
pub struct Router {
    registry: BackendRegistry,
    policy: SelectionPolicy,
}

impl Router {
    pub fn new(registry: BackendRegistry, policy: SelectionPolicy) -> Self {
        Self { registry, policy }
    }

    pub fn route<'a>(
        &'a self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<&'a BackendInstance, CanonicalError> {
        let mut candidates = self.registry.candidates(req);

        if let Some(name) = req.routing.backend.as_ref() {
            candidates.retain(|c| c.name == *name);
        }

        if !req.routing.allowlist.is_empty() {
            candidates.retain(|c| req.routing.allowlist.iter().any(|x| x == &c.name));
        }

        if !req.routing.denylist.is_empty() {
            candidates.retain(|c| !req.routing.denylist.iter().any(|x| x == &c.name));
        }

        if candidates.is_empty() {
            return Err(CanonicalError {
                code: "no_candidate_backend".to_string(),
                message: "no candidate backend".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        self.policy.select(req, candidates)
    }
}
