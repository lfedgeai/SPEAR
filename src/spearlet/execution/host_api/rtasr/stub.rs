use crate::spearlet::execution::host_api::DefaultHostApi;
use crate::spearlet::execution::hostcall::types::{FdInner, PollEvents, RtAsrConnState};
use serde_json::json;

impl DefaultHostApi {
    pub(super) fn spawn_rtasr_stub_tasks(&self, fd: i32) {
        let table = self.fd_table.clone();

        self.spawn_background(async move {
            let mut last_emit = tokio::time::Instant::now();
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                let Some(entry) = table.get(fd) else {
                    break;
                };

                let notify = {
                    let mut e = match entry.lock() {
                        Ok(v) => v,
                        Err(_) => break,
                    };

                    if e.closed {
                        break;
                    }

                    let old = e.poll_mask;
                    let new_mask = {
                        let FdInner::RtAsr(st) = &mut e.inner else {
                            break;
                        };

                        if let Some(item) = st.send_queue.pop_front() {
                            st.send_queue_bytes = st.send_queue_bytes.saturating_sub(item.byte_len());
                        }

                        let interval_ms = st
                            .params
                            .get("stub_event_interval_ms")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(100);
                        let payload_bytes = st
                            .params
                            .get("stub_event_payload_bytes")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(256);
                        let completed_every = st
                            .params
                            .get("stub_emit_completed_every")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(20);

                        if last_emit.elapsed() >= std::time::Duration::from_millis(interval_ms) {
                            last_emit = tokio::time::Instant::now();
                            st.stub_event_seq = st.stub_event_seq.wrapping_add(1);
                            let seq = st.stub_event_seq;
                            let is_completed = completed_every != 0 && (seq % completed_every) == 0;
                            let ty = if is_completed {
                                "transcription.completed"
                            } else {
                                "transcription.delta"
                            };
                            let text_len = (payload_bytes as usize).saturating_sub(80);
                            let text = "a".repeat(text_len);
                            let body = json!({
                                "type": ty,
                                "seq": seq,
                                "text": text,
                            });
                            let payload =
                                serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());

                            st.recv_queue_bytes = st.recv_queue_bytes.saturating_add(payload.len());
                            st.recv_queue.push_back(payload);
                            while st.recv_queue_bytes > st.max_recv_queue_bytes {
                                let Some(old) = st.recv_queue.pop_front() else {
                                    st.recv_queue_bytes = 0;
                                    break;
                                };
                                st.recv_queue_bytes = st.recv_queue_bytes.saturating_sub(old.len());
                                st.dropped_events = st.dropped_events.wrapping_add(1);
                            }
                        }

                        let mut m = PollEvents::EMPTY;
                        if !st.recv_queue.is_empty() {
                            m.insert(PollEvents::IN);
                        }
                        let writable = st.send_queue_bytes < st.max_send_queue_bytes
                            && st.state != RtAsrConnState::Draining
                            && st.state != RtAsrConnState::Closed
                            && st.state != RtAsrConnState::Error;
                        if writable {
                            m.insert(PollEvents::OUT);
                        }
                        if st.state == RtAsrConnState::Error {
                            m.insert(PollEvents::ERR);
                        }
                        m
                    };

                    e.poll_mask = new_mask;
                    e.poll_mask.bits() != old.bits()
                };

                if notify {
                    table.notify_watchers(fd);
                }
            }
        });
    }
}

