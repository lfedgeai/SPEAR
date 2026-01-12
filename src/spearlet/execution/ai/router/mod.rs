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
            let mut supporting: Vec<String> = Vec::new();
            for inst in self.registry.instances().iter() {
                if !inst.capabilities.supports_operation(&req.operation) {
                    continue;
                }

                let mut missing_features: Vec<&str> = Vec::new();
                for f in req.requirements.required_features.iter() {
                    if !inst.capabilities.has_feature(f) {
                        missing_features.push(f);
                    }
                }
                let mut missing_transports: Vec<&str> = Vec::new();
                for t in req.requirements.required_transports.iter() {
                    if !inst.capabilities.transports.iter().any(|x| x == t) {
                        missing_transports.push(t);
                    }
                }

                supporting.push(format!(
                    "{}(missing_features={:?}, missing_transports={:?}, features={:?}, transports={:?})",
                    inst.name, missing_features, missing_transports, inst.capabilities.features, inst.capabilities.transports
                ));
            }

            let msg = format!(
                "no candidate backend: op={:?} required_features={:?} required_transports={:?} routing_backend={:?} allowlist={:?} denylist={:?} backends={:?}",
                req.operation,
                req.requirements.required_features,
                req.requirements.required_transports,
                req.routing.backend,
                req.routing.allowlist,
                req.routing.denylist,
                supporting,
            );
            return Err(CanonicalError {
                code: "no_candidate_backend".to_string(),
                message: msg,
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        self.policy.select(req, candidates)
    }
}
