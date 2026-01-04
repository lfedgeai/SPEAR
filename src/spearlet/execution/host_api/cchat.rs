use crate::spearlet::execution::ai::ir::ResultPayload;
use crate::spearlet::execution::ai::normalize::chat::normalize_cchat_session;
use crate::spearlet::execution::host_api::DefaultHostApi;
use crate::spearlet::execution::hostcall::types::{
    ChatResponseState, ChatSessionState, FdEntry, FdFlags, FdInner, FdKind, PollEvents,
};
use libc::{EBADF, EIO};
use serde_json::json;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct ChatSessionSnapshot {
    pub fd: i32,
    pub messages: Vec<(String, String)>,
    pub tools: Vec<(i32, String)>,
    pub params: HashMap<String, serde_json::Value>,
}

impl DefaultHostApi {
    pub fn cchat_create(&self) -> i32 {
        self.fd_table.alloc(FdEntry {
            kind: FdKind::ChatSession,
            flags: FdFlags::default(),
            poll_mask: PollEvents::default(),
            watchers: HashSet::new(),
            closed: false,
            inner: FdInner::ChatSession(ChatSessionState::default()),
        })
    }

    pub fn cchat_write_msg(&self, fd: i32, role: String, content: String) -> i32 {
        let Some(entry) = self.fd_table.get(fd) else {
            return -EBADF;
        };
        let mut e = match entry.lock() {
            Ok(v) => v,
            Err(_) => return -EIO,
        };
        if e.closed {
            return -EBADF;
        }
        let FdInner::ChatSession(s) = &mut e.inner else {
            return -EBADF;
        };
        s.messages.push((role, content));
        0
    }

    pub fn cchat_write_fn(&self, fd: i32, fn_offset: i32, fn_json: String) -> i32 {
        let Some(entry) = self.fd_table.get(fd) else {
            return -EBADF;
        };
        let mut e = match entry.lock() {
            Ok(v) => v,
            Err(_) => return -EIO,
        };
        if e.closed {
            return -EBADF;
        }
        let FdInner::ChatSession(s) = &mut e.inner else {
            return -EBADF;
        };
        s.tools.push((fn_offset, fn_json));
        0
    }

    pub fn cchat_ctl_set_param(&self, fd: i32, key: String, value: serde_json::Value) -> i32 {
        let Some(entry) = self.fd_table.get(fd) else {
            return -EBADF;
        };
        let mut e = match entry.lock() {
            Ok(v) => v,
            Err(_) => return -EIO,
        };
        if e.closed {
            return -EBADF;
        }
        match &mut e.inner {
            FdInner::ChatSession(s) => {
                s.params.insert(key, value);
                0
            }
            FdInner::ChatResponse(_) => 0,
            _ => -EBADF,
        }
    }

    pub fn cchat_ctl_get_metrics(&self, fd: i32) -> Result<Vec<u8>, i32> {
        let Some(entry) = self.fd_table.get(fd) else {
            return Err(-EBADF);
        };
        let e = match entry.lock() {
            Ok(v) => v,
            Err(_) => return Err(-EIO),
        };
        let FdInner::ChatResponse(r) = &e.inner else {
            return Err(-EBADF);
        };
        if r.metrics_bytes.is_empty() {
            Ok(b"{}".to_vec())
        } else {
            Ok(r.metrics_bytes.clone())
        }
    }

    pub fn cchat_send(&self, fd: i32, flags: i32) -> Result<i32, i32> {
        let metrics_enabled = (flags & 1) != 0;
        let snapshot = self.cchat_get_session_snapshot(fd)?;
        let resp_fd = self.fd_table.alloc(FdEntry {
            kind: FdKind::ChatResponse,
            flags: FdFlags::default(),
            poll_mask: PollEvents::default(),
            watchers: HashSet::new(),
            closed: false,
            inner: FdInner::ChatResponse(ChatResponseState::default()),
        });

        let req = normalize_cchat_session(&snapshot);
        let resp = match self.ai_engine.invoke(&req) {
            Ok(r) => r,
            Err(e) => {
                let body = json!({"error": {"message": e.to_string()}});
                let bytes = serde_json::to_vec(&body).map_err(|_| -EIO)?;
                let metrics_bytes = if metrics_enabled { b"{}".to_vec() } else { Vec::new() };
                self.cchat_put_response(resp_fd, bytes, metrics_bytes)?;
                return Ok(resp_fd);
            }
        };

        let bytes = match resp.result {
            ResultPayload::Payload(v) => serde_json::to_vec(&v).map_err(|_| -EIO)?,
            ResultPayload::Error(e) => {
                let body = json!({"error": {"code": e.code, "message": e.message}});
                serde_json::to_vec(&body).map_err(|_| -EIO)?
            }
        };
        let metrics_bytes = if metrics_enabled {
            let usage = json!({
                "prompt_tokens": snapshot.messages.len() as i64,
                "completion_tokens": 1,
                "total_tokens": (snapshot.messages.len() as i64) + 1,
            });
            serde_json::to_vec(&usage).unwrap_or_else(|_| b"{}".to_vec())
        } else {
            Vec::new()
        };

        self.cchat_put_response(resp_fd, bytes, metrics_bytes)?;
        Ok(resp_fd)
    }

    pub fn cchat_recv(&self, response_fd: i32) -> Result<Vec<u8>, i32> {
        let Some(entry) = self.fd_table.get(response_fd) else {
            return Err(-EBADF);
        };
        let e = match entry.lock() {
            Ok(v) => v,
            Err(_) => return Err(-EIO),
        };
        let FdInner::ChatResponse(r) = &e.inner else {
            return Err(-EBADF);
        };
        Ok(r.bytes.clone())
    }

    pub fn cchat_close(&self, fd: i32) -> i32 {
        self.fd_table.close(fd)
    }

    fn cchat_get_session_snapshot(&self, fd: i32) -> Result<ChatSessionSnapshot, i32> {
        let Some(entry) = self.fd_table.get(fd) else {
            return Err(-EBADF);
        };
        let e = entry.lock().map_err(|_| -EIO)?;
        if e.closed {
            return Err(-EBADF);
        }
        let FdInner::ChatSession(s) = &e.inner else {
            return Err(-EBADF);
        };
        Ok(ChatSessionSnapshot {
            fd,
            messages: s.messages.clone(),
            tools: s.tools.clone(),
            params: s.params.clone(),
        })
    }

    fn cchat_put_response(
        &self,
        resp_fd: i32,
        bytes: Vec<u8>,
        metrics_bytes: Vec<u8>,
    ) -> Result<(), i32> {
        let Some(entry) = self.fd_table.get(resp_fd) else {
            return Err(-EBADF);
        };
        {
            let mut e = entry.lock().map_err(|_| -EIO)?;
            let FdInner::ChatResponse(r) = &mut e.inner else {
                return Err(-EBADF);
            };
            r.bytes = bytes;
            r.metrics_bytes = metrics_bytes;
            e.poll_mask.insert(PollEvents::IN);
        }
        self.fd_table.notify_watchers(resp_fd);
        Ok(())
    }
}
