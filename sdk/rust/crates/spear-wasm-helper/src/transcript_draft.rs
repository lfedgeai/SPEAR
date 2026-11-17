//! Shared transcript-draft state for realtime ASR samples.
//! 实时 ASR 示例共享的转写草稿状态。

use crate::rtasr_protocol::TranscriptEvent;

/// Draft-state machine shared by Rust realtime ASR samples.
/// Rust 实时 ASR 示例共享的草稿状态机。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RealtimeTranscriptDraft {
    draft: String,
    confirmed_chars_emitted: usize,
}

/// One normalized update produced after consuming a transcript event.
/// 消费一条 transcript 事件后产出的归一化更新结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscriptDraftUpdate {
    /// Current full draft after applying the event.
    /// 应用该事件后的当前完整草稿。
    pub current_draft: Option<String>,
    /// Confirmed suffix that has not yet been emitted to an append-only sink.
    /// 尚未输出到追加式下游的已确认尾部。
    pub newly_confirmed_suffix: String,
    /// Final transcript resolved from the completed event or the current draft.
    /// 从 completed 事件或当前草稿解析出的最终 transcript。
    pub completed_transcript: Option<String>,
}

impl RealtimeTranscriptDraft {
    /// Reset the draft when a new utterance starts.
    /// 在新 utterance 开始时重置草稿状态。
    pub fn reset(&mut self) {
        self.draft.clear();
        self.confirmed_chars_emitted = 0;
    }

    /// Return the current transcript draft.
    /// 返回当前的转写草稿。
    pub fn current_draft(&self) -> &str {
        &self.draft
    }

    /// Apply one transcript event and normalize provider-specific semantics.
    /// 应用一条 transcript 事件，并归一化不同 provider 的语义差异。
    pub fn apply(&mut self, event: &TranscriptEvent) -> TranscriptDraftUpdate {
        match event {
            TranscriptEvent::SpeechStarted => TranscriptDraftUpdate::default(),
            TranscriptEvent::Delta(delta) => self.apply_delta(delta),
            TranscriptEvent::Preview { text, stash } => self.apply_preview(text, stash),
            TranscriptEvent::Completed(transcript) => self.apply_completed(transcript.as_deref()),
        }
    }

    fn apply_delta(&mut self, delta: &str) -> TranscriptDraftUpdate {
        if delta.is_empty() {
            return TranscriptDraftUpdate {
                current_draft: Some(self.draft.clone()),
                ..TranscriptDraftUpdate::default()
            };
        }
        self.draft.push_str(delta);
        self.confirmed_chars_emitted += delta.chars().count();
        TranscriptDraftUpdate {
            current_draft: Some(self.draft.clone()),
            newly_confirmed_suffix: delta.to_string(),
            completed_transcript: None,
        }
    }

    fn apply_preview(&mut self, text: &str, stash: &str) -> TranscriptDraftUpdate {
        self.draft = format!("{text}{stash}");
        let (newly_confirmed_suffix, confirmed_chars) =
            take_unwritten_char_suffix(text, self.confirmed_chars_emitted);
        self.confirmed_chars_emitted = self.confirmed_chars_emitted.max(confirmed_chars);
        TranscriptDraftUpdate {
            current_draft: Some(self.draft.clone()),
            newly_confirmed_suffix,
            completed_transcript: None,
        }
    }

    fn apply_completed(&mut self, transcript: Option<&str>) -> TranscriptDraftUpdate {
        let resolved = transcript
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                if self.draft.is_empty() {
                    None
                } else {
                    Some(self.draft.clone())
                }
            });
        let mut update = TranscriptDraftUpdate {
            current_draft: resolved.clone(),
            newly_confirmed_suffix: String::new(),
            completed_transcript: resolved.clone(),
        };
        if let Some(final_text) = resolved {
            let (newly_confirmed_suffix, confirmed_chars) =
                take_unwritten_char_suffix(&final_text, self.confirmed_chars_emitted);
            self.confirmed_chars_emitted = self.confirmed_chars_emitted.max(confirmed_chars);
            self.draft = final_text;
            update.newly_confirmed_suffix = newly_confirmed_suffix;
        }
        update
    }
}

/// Return the suffix that has not yet been emitted when `written_chars`
/// characters from the same transcript have already been written.
/// 返回同一段 transcript 中尚未输出的尾部；`written_chars` 表示已经写出的
/// 字符数量。
fn take_unwritten_char_suffix(text: &str, written_chars: usize) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= written_chars {
        return (String::new(), chars.len());
    }
    let appended: String = chars[written_chars..].iter().collect();
    (appended, chars.len())
}

#[cfg(test)]
mod tests {
    use super::{RealtimeTranscriptDraft, TranscriptDraftUpdate};
    use crate::rtasr_protocol::TranscriptEvent;

    #[test]
    fn delta_events_extend_the_draft_and_emit_the_same_suffix() {
        let mut draft = RealtimeTranscriptDraft::default();
        let update = draft.apply(&TranscriptEvent::Delta("hello".to_string()));
        assert_eq!(
            update,
            TranscriptDraftUpdate {
                current_draft: Some("hello".to_string()),
                newly_confirmed_suffix: "hello".to_string(),
                completed_transcript: None,
            }
        );
        assert_eq!(draft.current_draft(), "hello");
    }

    #[test]
    fn qwen_preview_emits_only_newly_confirmed_prefix() {
        let mut draft = RealtimeTranscriptDraft::default();
        let first = draft.apply(&TranscriptEvent::Preview {
            text: "hel".to_string(),
            stash: "lo".to_string(),
        });
        assert_eq!(first.current_draft, Some("hello".to_string()));
        assert_eq!(first.newly_confirmed_suffix, "hel");

        let second = draft.apply(&TranscriptEvent::Preview {
            text: "hello".to_string(),
            stash: " world".to_string(),
        });
        assert_eq!(second.current_draft, Some("hello world".to_string()));
        assert_eq!(second.newly_confirmed_suffix, "lo");
    }

    #[test]
    fn completed_event_fills_missing_tail_from_final_transcript() {
        let mut draft = RealtimeTranscriptDraft::default();
        let _ = draft.apply(&TranscriptEvent::Preview {
            text: String::new(),
            stash: "hello".to_string(),
        });

        let update = draft.apply(&TranscriptEvent::Completed(Some("hello".to_string())));
        assert_eq!(update.completed_transcript, Some("hello".to_string()));
        assert_eq!(update.newly_confirmed_suffix, "hello".to_string());
    }

    #[test]
    fn completed_event_falls_back_to_existing_draft() {
        let mut draft = RealtimeTranscriptDraft::default();
        let _ = draft.apply(&TranscriptEvent::Preview {
            text: "你好".to_string(),
            stash: "世界".to_string(),
        });

        let update = draft.apply(&TranscriptEvent::Completed(None));
        assert_eq!(update.completed_transcript, Some("你好世界".to_string()));
        assert_eq!(update.newly_confirmed_suffix, "世界".to_string());
    }

    #[test]
    fn reset_clears_draft_and_confirmed_progress() {
        let mut draft = RealtimeTranscriptDraft::default();
        let _ = draft.apply(&TranscriptEvent::Delta("done".to_string()));
        draft.reset();
        assert_eq!(draft.current_draft(), "");

        let update = draft.apply(&TranscriptEvent::Preview {
            text: "新".to_string(),
            stash: "句子".to_string(),
        });
        assert_eq!(update.newly_confirmed_suffix, "新");
    }
}
