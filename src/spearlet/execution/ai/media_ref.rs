use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaRef {
    InlineBase64 { mime: String, data: String },
    SmsFile { mime: String, uri: String },
    HttpUrl { mime: String, url: String },
}
