//! Shared RTASR event parsing helpers.
//! 共享的 RTASR 事件解析辅助模块。

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptEvent {
    SpeechStarted,
    /// Incremental transcript fragment to append (OpenAI-style `delta`).
    /// 需要追加的增量转写片段（OpenAI 风格的 `delta`）。
    Delta(String),
    /// Full-replacement live preview of the current sentence (Qwen-style
    /// `text` + `stash`). `text` is the confirmed prefix, `stash` is the
    /// provisional suffix. Consumers should render `text + stash` as the
    /// current preview, replacing any previous preview for the same sentence.
    /// 当前句子的全量实时预览（Qwen 风格的 `text` + `stash`）。`text` 为已确认
    /// 前缀，`stash` 为临时后缀。消费者应以 `text + stash` 作为当前预览，
    /// 并替换同一句子之前的预览。
    Preview { text: String, stash: String },
    Completed(Option<String>),
}

pub fn parse_transcript_event(payload: &[u8]) -> Option<TranscriptEvent> {
    let value: Value = serde_json::from_slice(payload).ok()?;
    let event_type = value.get("type")?.as_str()?;

    match event_type {
        "speech_started" | "input_audio_buffer.speech_started" => {
            Some(TranscriptEvent::SpeechStarted)
        }
        "input_audio_transcription.delta"
        | "conversation.item.input_audio_transcription.delta"
        | "output_audio_transcript.delta" => {
            let delta = value
                .get("delta")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Some(TranscriptEvent::Delta(delta.to_string()))
        }
        // Qwen realtime ASR live preview: cumulative `text` (confirmed prefix)
        // plus `stash` (provisional suffix), sent at high frequency.
        // Qwen 实时 ASR 实时预览：累积的 `text`（已确认前缀）加上 `stash`
        // （临时后缀），高频发送。
        "input_audio_transcription.text"
        | "conversation.item.input_audio_transcription.text" => {
            let text = value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let stash = value
                .get("stash")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            Some(TranscriptEvent::Preview { text, stash })
        }
        "input_audio_transcription.completed"
        | "conversation.item.input_audio_transcription.completed"
        | "output_audio_transcript.done" => {
            let transcript = value
                .get("transcript")
                .and_then(Value::as_str)
                .or_else(|| value.get("text").and_then(Value::as_str))
                .map(str::to_string);
            Some(TranscriptEvent::Completed(transcript))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_delta_supports_conversation_item_namespace() {
        let payload =
            br#"{"type":"conversation.item.input_audio_transcription.delta","delta":"hello"}"#;
        let event = parse_transcript_event(payload).unwrap();
        assert_eq!(event, TranscriptEvent::Delta("hello".to_string()));
    }

    #[test]
    fn parse_completed_supports_text_fallback() {
        let payload = br#"{"type":"output_audio_transcript.done","text":"done"}"#;
        let event = parse_transcript_event(payload).unwrap();
        assert_eq!(event, TranscriptEvent::Completed(Some("done".to_string())));
    }

    #[test]
    fn parse_speech_started_supports_input_audio_buffer_namespace() {
        let payload = br#"{"type":"input_audio_buffer.speech_started"}"#;
        let event = parse_transcript_event(payload).unwrap();
        assert_eq!(event, TranscriptEvent::SpeechStarted);
    }

    #[test]
    fn parse_qwen_text_event_yields_preview_with_text_and_stash() {
        // Keep the byte payload ASCII-only so the test can use raw bytes directly.
        // 保持字节 payload 为纯 ASCII，这样测试可以直接使用原始字节。
        let payload = "{\"type\":\"conversation.item.input_audio_transcription.text\",\"text\":\"\\u4eca\\u5929\",\"stash\":\"\\u5929\\u6c14\"}".as_bytes();
        let event = parse_transcript_event(payload).unwrap();
        assert_eq!(
            event,
            TranscriptEvent::Preview {
                text: "今天".to_string(),
                stash: "天气".to_string(),
            }
        );
    }

    #[test]
    fn parse_qwen_text_event_tolerates_missing_fields() {
        // Keep the byte payload ASCII-only so the test can use raw bytes directly.
        // 保持字节 payload 为纯 ASCII，这样测试可以直接使用原始字节。
        let payload = "{\"type\":\"input_audio_transcription.text\",\"stash\":\"\\u5317\\u4eac\\u7684\"}".as_bytes();
        let event = parse_transcript_event(payload).unwrap();
        assert_eq!(
            event,
            TranscriptEvent::Preview {
                text: String::new(),
                stash: "北京的".to_string(),
            }
        );
    }
}
