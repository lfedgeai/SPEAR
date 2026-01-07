use crate::spearlet::execution::host_api::DefaultHostApi;
use crate::spearlet::execution::hostcall::types::{FdInner, PollEvents};

impl DefaultHostApi {
    pub(super) fn spawn_mic_stub_task(&self, fd: i32, generation: u64) {
        let table = self.fd_table.clone();
        self.spawn_background(async move {
            loop {
                let frame_ms = {
                    let Some(entry) = table.get(fd) else {
                        return;
                    };
                    let e = match entry.lock() {
                        Ok(v) => v,
                        Err(_) => return,
                    };
                    if e.closed {
                        return;
                    }
                    let FdInner::Mic(st) = &e.inner else {
                        return;
                    };
                    if !st.running || st.generation != generation {
                        return;
                    }
                    st.config.as_ref().map(|c| c.frame_ms).unwrap_or(20)
                };

                let sleep_ms = if frame_ms == 0 { 20 } else { frame_ms };
                tokio::time::sleep(std::time::Duration::from_millis(sleep_ms as u64)).await;

                let Some(entry) = table.get(fd) else {
                    return;
                };

                let notify = {
                    let mut e = match entry.lock() {
                        Ok(v) => v,
                        Err(_) => return,
                    };

                    if e.closed {
                        return;
                    }

                    let old = e.poll_mask;
                    let new_mask = {
                        let FdInner::Mic(st) = &mut e.inner else {
                            return;
                        };
                        if !st.running || st.generation != generation {
                            return;
                        }

                        let Some(cfg) = &st.config else {
                            continue;
                        };

                        let frame_samples =
                            (cfg.sample_rate_hz as u64 * cfg.frame_ms as u64) / 1000;
                        let bytes_len = frame_samples
                            .saturating_mul(cfg.channels as u64)
                            .saturating_mul(2) as usize;
                        let payload = if let Some(buf) = st.stub_pcm16.as_ref() {
                            if buf.is_empty() {
                                vec![0u8; bytes_len]
                            } else {
                                let mut out: Vec<u8> = Vec::with_capacity(bytes_len);
                                while out.len() < bytes_len {
                                    let remain = bytes_len - out.len();
                                    let start = st.stub_pcm16_offset % buf.len();
                                    let chunk = std::cmp::min(remain, buf.len() - start);
                                    out.extend_from_slice(&buf[start..start + chunk]);
                                    st.stub_pcm16_offset = (start + chunk) % buf.len();
                                }
                                out
                            }
                        } else {
                            vec![0u8; bytes_len]
                        };

                        st.queue_bytes = st.queue_bytes.saturating_add(payload.len());
                        st.queue.push_back(payload);
                        while st.queue_bytes > st.max_queue_bytes {
                            let Some(old) = st.queue.pop_front() else {
                                st.queue_bytes = 0;
                                break;
                            };
                            st.queue_bytes = st.queue_bytes.saturating_sub(old.len());
                            st.dropped_frames = st.dropped_frames.wrapping_add(1);
                        }

                        let mut m = PollEvents::EMPTY;
                        if !st.queue.is_empty() {
                            m.insert(PollEvents::IN);
                        }
                        if st.last_error.is_some() {
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
