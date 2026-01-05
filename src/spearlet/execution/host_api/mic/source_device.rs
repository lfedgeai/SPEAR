use crate::spearlet::execution::host_api::DefaultHostApi;
use crate::spearlet::execution::hostcall::types::MicConfig;

#[cfg(feature = "mic-device")]
use crate::spearlet::execution::hostcall::types::{FdInner, PollEvents};

#[allow(dead_code)]
pub(super) struct DeviceMicStartRequest {
    pub fd: i32,
    pub config: MicConfig,
    pub device_name: Option<String>,
    pub generation: u64,
}

#[allow(dead_code)]
pub(super) enum DeviceMicStartError {
    NotImplemented,
    Failed(String),
}

impl DefaultHostApi {
    #[cfg(not(feature = "mic-device"))]
    #[allow(dead_code)]
    pub(super) fn spawn_mic_device_task(
        &self,
        req: DeviceMicStartRequest,
    ) -> Result<(), DeviceMicStartError> {
        let _ = req;
        Err(DeviceMicStartError::NotImplemented)
    }

    #[cfg(feature = "mic-device")]
    #[allow(dead_code)]
    pub(super) fn spawn_mic_device_task(
        &self,
        req: DeviceMicStartRequest,
    ) -> Result<(), DeviceMicStartError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use std::collections::VecDeque;
        use std::sync::Arc;
        use std::time::Duration;

        if req.config.format != "pcm16" {
            return Err(DeviceMicStartError::Failed(format!(
                "unsupported format: {}",
                req.config.format
            )));
        }
        if req.config.channels == 0 {
            return Err(DeviceMicStartError::Failed("channels must be >= 1".into()));
        }

        let table = self.fd_table.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let fd = req.fd;
        let generation = req.generation;
        let cfg = req.config.clone();
        let device_name = req.device_name.clone();

        std::thread::spawn(move || {
            type InitOk = (
                cpal::Stream,
                Arc<parking_lot::Mutex<VecDeque<f32>>>,
                u32,
                usize,
            );
            let init = (|| -> Result<InitOk, String> {
                let host = cpal::default_host();
                let device = match device_name.as_ref() {
                    Some(name) => {
                        let mut found = None;
                        let devices = host.input_devices().map_err(|e| e.to_string())?;
                        for dev in devices {
                            let dev_name = dev.name().unwrap_or_default();
                            if dev_name == *name {
                                found = Some(dev);
                                break;
                            }
                        }
                        found.ok_or_else(|| format!("input device not found: {}", name))?
                    }
                    None => host
                        .default_input_device()
                        .ok_or_else(|| "no default input device".to_string())?,
                };

                let supported = device.default_input_config().map_err(|e| e.to_string())?;
                let input_sample_rate = supported.sample_rate().0;
                let input_channels = supported.channels() as usize;
                let sample_format = supported.sample_format();
                let stream_config: cpal::StreamConfig = supported.config();

                let raw = Arc::new(parking_lot::Mutex::new(VecDeque::<f32>::new()));
                let max_raw_samples = (input_sample_rate as usize).saturating_mul(5);

                let raw_cb = raw.clone();
                let data_cb_f32 = move |data: &[f32]| {
                    let mut q = raw_cb.lock();
                    if input_channels <= 1 {
                        for &s in data {
                            q.push_back(s);
                        }
                    } else {
                        let mut i = 0;
                        while i + input_channels <= data.len() {
                            let mut acc = 0.0f32;
                            for c in 0..input_channels {
                                acc += data[i + c];
                            }
                            q.push_back(acc / input_channels as f32);
                            i += input_channels;
                        }
                    }
                    if max_raw_samples > 0 {
                        while q.len() > max_raw_samples {
                            q.pop_front();
                        }
                    }
                };

                let raw_cb = raw.clone();
                let data_cb_i16 = move |data: &[i16]| {
                    let mut q = raw_cb.lock();
                    if input_channels <= 1 {
                        for &s in data {
                            q.push_back(s as f32 / 32768.0);
                        }
                    } else {
                        let mut i = 0;
                        while i + input_channels <= data.len() {
                            let mut acc = 0.0f32;
                            for c in 0..input_channels {
                                acc += data[i + c] as f32 / 32768.0;
                            }
                            q.push_back(acc / input_channels as f32);
                            i += input_channels;
                        }
                    }
                    if max_raw_samples > 0 {
                        while q.len() > max_raw_samples {
                            q.pop_front();
                        }
                    }
                };

                let raw_cb = raw.clone();
                let data_cb_u16 = move |data: &[u16]| {
                    let mut q = raw_cb.lock();
                    if input_channels <= 1 {
                        for &s in data {
                            let v = (s as f32 / 65535.0) * 2.0 - 1.0;
                            q.push_back(v);
                        }
                    } else {
                        let mut i = 0;
                        while i + input_channels <= data.len() {
                            let mut acc = 0.0f32;
                            for c in 0..input_channels {
                                let v = (data[i + c] as f32 / 65535.0) * 2.0 - 1.0;
                                acc += v;
                            }
                            q.push_back(acc / input_channels as f32);
                            i += input_channels;
                        }
                    }
                    if max_raw_samples > 0 {
                        while q.len() > max_raw_samples {
                            q.pop_front();
                        }
                    }
                };

                let err_table = table.clone();
                let err_fn = move |err: cpal::StreamError| {
                    let Some(entry) = err_table.get(fd) else {
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
                        if let FdInner::Mic(st) = &mut e.inner {
                            if st.generation != generation {
                                return;
                            }
                            st.last_error = Some(err.to_string());
                            let mut m = PollEvents::EMPTY;
                            if !st.queue.is_empty() {
                                m.insert(PollEvents::IN);
                            }
                            if st.last_error.is_some() {
                                m.insert(PollEvents::ERR);
                            }
                            if e.closed {
                                m.insert(PollEvents::HUP);
                            }
                            e.poll_mask = m;
                        }
                        e.poll_mask.bits() != old.bits()
                    };
                    if notify {
                        err_table.notify_watchers(fd);
                    }
                };

                let stream = match sample_format {
                    cpal::SampleFormat::F32 => device
                        .build_input_stream(
                            &stream_config,
                            move |data: &[f32], _: &cpal::InputCallbackInfo| data_cb_f32(data),
                            err_fn,
                            None,
                        )
                        .map_err(|e| e.to_string())?,
                    cpal::SampleFormat::I16 => device
                        .build_input_stream(
                            &stream_config,
                            move |data: &[i16], _: &cpal::InputCallbackInfo| data_cb_i16(data),
                            err_fn,
                            None,
                        )
                        .map_err(|e| e.to_string())?,
                    cpal::SampleFormat::U16 => device
                        .build_input_stream(
                            &stream_config,
                            move |data: &[u16], _: &cpal::InputCallbackInfo| data_cb_u16(data),
                            err_fn,
                            None,
                        )
                        .map_err(|e| e.to_string())?,
                    _ => return Err("unsupported sample format".into()),
                };

                stream.play().map_err(|e| e.to_string())?;
                Ok((stream, raw, input_sample_rate, input_channels))
            })();

            let (stream, raw, input_sample_rate, _input_channels) = match init {
                Ok(v) => {
                    let _ = tx.send(Ok(()));
                    v
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };

            let output_rate = cfg.sample_rate_hz.max(1);
            let frame_ms = cfg.frame_ms.max(1);
            let out_channels = cfg.channels as usize;
            let out_frame_samples =
                ((output_rate as u64).saturating_mul(frame_ms as u64) / 1000) as usize;
            let step = (input_sample_rate as f64) / (output_rate as f64);
            let mut cursor: f64 = 0.0;

            loop {
                std::thread::sleep(Duration::from_millis(frame_ms as u64));

                let Some(entry) = table.get(fd) else {
                    break;
                };

                {
                    let e = match entry.lock() {
                        Ok(v) => v,
                        Err(_) => break,
                    };
                    if e.closed {
                        break;
                    }
                    let FdInner::Mic(st) = &e.inner else {
                        break;
                    };
                    if !st.running || st.generation != generation {
                        break;
                    }
                }

                let mut out_mono: Vec<f32> = Vec::with_capacity(out_frame_samples);
                {
                    let mut q = raw.lock();
                    if q.len() < 2 {
                        continue;
                    }
                    for _ in 0..out_frame_samples {
                        let need = cursor + 1.0;
                        if (q.len() as f64) <= need {
                            break;
                        }
                        let i0 = cursor.floor() as usize;
                        let frac = (cursor - i0 as f64) as f32;
                        let s0 = q[i0];
                        let s1 = q[i0 + 1];
                        out_mono.push(s0 * (1.0 - frac) + s1 * frac);
                        cursor += step;

                        if cursor >= 1.0 {
                            let drop_n = cursor.floor() as usize;
                            for _ in 0..drop_n {
                                q.pop_front();
                            }
                            cursor -= drop_n as f64;
                        }
                    }
                }

                if out_mono.len() != out_frame_samples {
                    continue;
                }

                let mut payload: Vec<u8> = Vec::with_capacity(
                    out_frame_samples
                        .saturating_mul(out_channels)
                        .saturating_mul(2),
                );
                for &s in out_mono.iter() {
                    let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                    let b = v.to_le_bytes();
                    for _ in 0..out_channels {
                        payload.extend_from_slice(&b);
                    }
                }

                let notify = {
                    let mut e = match entry.lock() {
                        Ok(v) => v,
                        Err(_) => break,
                    };
                    if e.closed {
                        break;
                    }
                    let old = e.poll_mask;
                    let FdInner::Mic(st) = &mut e.inner else {
                        break;
                    };
                    if !st.running || st.generation != generation {
                        break;
                    }

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
                    if e.closed {
                        m.insert(PollEvents::HUP);
                    }
                    e.poll_mask = m;
                    e.poll_mask.bits() != old.bits()
                };
                if notify {
                    table.notify_watchers(fd);
                }
            }

            drop(stream);
        });

        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(DeviceMicStartError::Failed(e)),
            Err(_) => Err(DeviceMicStartError::Failed("device init timeout".into())),
        }
    }
}
