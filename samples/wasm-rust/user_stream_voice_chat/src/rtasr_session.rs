//! RTASR transcription session for the Rust voice chat sample.
//! Rust 版语音对话 sample 的 RTASR 转写会话。

use spear_wasm::{errorln, warnln, Fd, RtAsrStatus, SpearError};
use spear_wasm_helper::{
    rtasr_protocol::TranscriptEvent,
    rtasr_session::{BaseRtasrSession, RtasrConnectOptions},
};

use crate::{
    config::RTASR_TRANSPORT,
};

fn report_guest_warn(message: impl std::fmt::Display) {
    warnln!("[voice_chat] {message}");
}

fn report_guest_error(message: impl std::fmt::Display) {
    errorln!("[voice_chat] {message}");
}

fn format_rtasr_status(status: Option<&RtAsrStatus>) -> String {
    match status {
        Some(status) => match status.last_error.as_deref() {
            Some(last_error) if !last_error.trim().is_empty() => {
                format!("state={} last_error={last_error}", status.state)
            }
            _ => format!("state={} last_error=<none>", status.state),
        },
        None => "state=<unavailable> last_error=<unavailable>".to_string(),
    }
}

#[derive(Debug)]
pub struct VoiceTranscriptionSession {
    base: BaseRtasrSession,
    written_bytes: usize,
}

impl VoiceTranscriptionSession {
    pub fn connect() -> Result<Self, SpearError> {
        let mut base = BaseRtasrSession::create(RtasrConnectOptions::new(RTASR_TRANSPORT))?;
        if let Err(err) = base.connect_transport() {
            report_guest_error(format!(
                "rtasr connect failed: err={err}; {}",
                format_rtasr_status(base.status().ok().as_ref())
            ));
            let _ = base.close();
            return Err(err);
        }
        Ok(Self {
            base,
            written_bytes: 0,
        })
    }

    pub fn fd(&self) -> Fd {
        self.base.fd()
    }

    pub fn prepare_for_new_utterance(&mut self) -> Result<(), SpearError> {
        if let Err(err) = self.base.clear() {
            report_guest_error(format!(
                "rtasr clear failed: err={err}; {}",
                self.status_summary()
            ));
            return Err(err);
        }
        self.written_bytes = 0;
        Ok(())
    }

    pub fn append_audio(&mut self, audio: &[u8]) -> Result<(), SpearError> {
        if audio.is_empty() {
            return Ok(());
        }
        if let Err(err) = self.base.write_audio(audio) {
            report_guest_error(format!(
                "rtasr write_audio failed bytes={}: err={err}; {}",
                audio.len(),
                self.status_summary()
            ));
            return Err(err);
        }
        self.written_bytes += audio.len();
        Ok(())
    }

    pub fn written_bytes(&self) -> usize {
        self.written_bytes
    }

    pub fn flush(&mut self) -> Result<(), SpearError> {
        if let Err(err) = self.base.flush() {
            report_guest_error(format!(
                "rtasr flush failed: err={err}; {}",
                self.status_summary()
            ));
            return Err(err);
        }
        Ok(())
    }

    pub fn drain_events(&mut self) -> Result<Vec<TranscriptEvent>, SpearError> {
        match self.base.drain_transcript_events() {
            Ok(events) => Ok(events),
            Err(err) => {
                report_guest_error(format!(
                    "rtasr read events failed: err={err}; {}",
                    self.status_summary()
                ));
                Err(err)
            }
        }
    }

    pub fn close(self) -> Result<(), SpearError> {
        self.base.close()
    }

    pub fn log_disconnect_reason(&self, stage: &str) {
        report_guest_warn(format!("rtasr {stage}: {}", self.status_summary()));
    }

    fn status_summary(&self) -> String {
        format_rtasr_status(self.base.status().ok().as_ref())
    }
}
