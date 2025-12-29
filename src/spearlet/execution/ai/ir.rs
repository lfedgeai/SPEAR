use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    ChatCompletions,
    Embeddings,
    ImageGeneration,
    SpeechToText,
    TextToSpeech,
    RealtimeVoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CanonicalError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
    pub operation: Option<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RoutingHints {
    pub backend: Option<String>,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub denylist: Vec<String>,
}

impl Default for RoutingHints {
    fn default() -> Self {
        Self {
            backend: None,
            allowlist: Vec::new(),
            denylist: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Requirements {
    #[serde(default)]
    pub required_features: Vec<String>,
}

impl Default for Requirements {
    fn default() -> Self {
        Self {
            required_features: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRequestEnvelope {
    pub version: u32,
    pub request_id: String,
    pub operation: Operation,
    #[serde(default)]
    pub meta: HashMap<String, String>,
    #[serde(default)]
    pub routing: RoutingHints,
    #[serde(default)]
    pub requirements: Requirements,
    pub timeout_ms: Option<u64>,
    pub payload: Payload,
    #[serde(default)]
    pub extra: HashMap<String, Value>,
}

impl CanonicalRequestEnvelope {
    pub fn validate_basic(&self) -> Result<(), CanonicalError> {
        if self.version != 1 {
            return Err(CanonicalError {
                code: "unsupported_version".to_string(),
                message: format!("unsupported ir version: {}", self.version),
                retryable: false,
                operation: Some(self.operation.clone()),
            });
        }
        let ok = match (&self.operation, &self.payload) {
            (Operation::ChatCompletions, Payload::ChatCompletions(_)) => true,
            (Operation::Embeddings, Payload::Embeddings(_)) => true,
            (Operation::ImageGeneration, Payload::ImageGeneration(_)) => true,
            (Operation::SpeechToText, Payload::SpeechToText(_)) => true,
            (Operation::TextToSpeech, Payload::TextToSpeech(_)) => true,
            (Operation::RealtimeVoice, Payload::RealtimeVoice(_)) => true,
            _ => false,
        };
        if !ok {
            return Err(CanonicalError {
                code: "payload_mismatch".to_string(),
                message: "operation and payload mismatch".to_string(),
                retryable: false,
                operation: Some(self.operation.clone()),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsPayload {
    pub model: String,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub tools: Vec<Value>,
    #[serde(default)]
    pub params: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsPayload {
    #[serde(default)]
    pub input: Vec<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationPayload {
    pub prompt: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechToTextPayload {
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextToSpeechPayload {
    pub model: Option<String>,
    pub input: String,
    pub voice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeVoicePayload {
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Payload {
    ChatCompletions(ChatCompletionsPayload),
    Embeddings(EmbeddingsPayload),
    ImageGeneration(ImageGenerationPayload),
    SpeechToText(SpeechToTextPayload),
    TextToSpeech(TextToSpeechPayload),
    RealtimeVoice(RealtimeVoicePayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ResultPayload {
    Payload(Value),
    Error(CanonicalError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalResponseEnvelope {
    pub version: u32,
    pub request_id: String,
    pub operation: Operation,
    pub backend: String,
    pub result: ResultPayload,
    pub raw: Option<Vec<u8>>,
}
