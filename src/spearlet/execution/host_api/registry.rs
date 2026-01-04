use crate::spearlet::execution::ai::backends::openai_compatible::OpenAICompatibleBackendAdapter;
use crate::spearlet::execution::ai::backends::openai_realtime_ws::OpenAIRealtimeWsBackendAdapter;
use crate::spearlet::execution::ai::backends::stub::StubBackendAdapter;
use crate::spearlet::execution::ai::ir::Operation;
use crate::spearlet::execution::ai::router::capabilities::Capabilities;
use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
use crate::spearlet::execution::ai::router::registry::{BackendInstance, BackendRegistry};
use std::sync::Arc;

pub(super) fn build_registry_from_runtime_config(
    runtime_config: &super::super::runtime::RuntimeConfig,
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
                    "openai_realtime_ws" => Arc::new(OpenAIRealtimeWsBackendAdapter::new(
                        b.name.clone(),
                        b.base_url.clone(),
                        b.api_key_env.clone(),
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

