use spear_next::spearlet::config::{AiBackendConfig, AiCredentialConfig, SpearletConfig};
use spear_next::spearlet::execution::host_api::DefaultHostApi;
use spear_next::spearlet::execution::runtime::{ResourcePoolConfig, RuntimeConfig, RuntimeType};
use std::collections::HashMap;

mod common;

const SPEAR_EAGAIN: i32 = 11;
const QWEN_DEFAULT_MODEL: &str = "qwen3-asr-flash-realtime";
const OPENAI_DEFAULT_MODEL: &str = "gpt-4o-mini-transcribe";
const FIXTURE_SAMPLE_RATE_16K: u32 = 16_000;
const OPENAI_SAMPLE_RATE_24K: u32 = 24_000;
const AUDIO_CHUNK_BYTES: usize = 4_800;
const AUDIO_PCM_16K: &[u8] = include_bytes!("fixtures/qwen_live_e2e_hello.pcm");

/// Build one minimal runtime config that mirrors the selected live-test backend.
/// 构造最小运行时配置，并镜像共享 live-test 配置中选中的 backend。
fn build_runtime(backend: &common::ResolvedBackend) -> DefaultHostApi {
    let mut cfg = SpearletConfig::default();
    cfg.ai.credentials.push(AiCredentialConfig {
        name: "live_test_credential".to_string(),
        kind: "env".to_string(),
        api_key_env: backend.api_key_env.clone(),
    });
    cfg.ai.backends.push(AiBackendConfig {
        name: backend.name.clone(),
        kind: backend.kind.clone(),
        base_url: backend.base_url.clone(),
        hosting: None,
        model: None,
        credential_ref: Some("live_test_credential".to_string()),
        provider: None,
        origin: None,
        deployment_id: None,
        weight: 100,
        priority: 0,
        ops: vec!["speech_to_text".to_string()],
        features: vec![],
        transports: vec!["websocket".to_string()],
    });

    let mut global_env = HashMap::new();
    global_env.insert(backend.api_key_env.clone(), backend.api_key.clone());

    DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: global_env,
        spearlet_config: Some(cfg),
        resource_pool: ResourcePoolConfig::default(),
    })
}

/// Pick the default model for each provider, while still allowing env overrides.
/// 为每个 provider 选择默认模型，同时允许用环境变量覆盖。
fn resolve_model(kind: &str) -> String {
    match kind {
        "rt_asr_openai_ws" => std::env::var("OPENAI_STT_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| OPENAI_DEFAULT_MODEL.to_string()),
        "rt_asr_qwen_ws" => std::env::var("QWEN_LIVE_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| QWEN_DEFAULT_MODEL.to_string()),
        other => panic!("unsupported realtime backend kind: {other}"),
    }
}

/// OpenAI currently fixes transcription input to 24 kHz PCM, while Qwen exposes
/// sample-rate configuration; reuse one fixture and adapt it per backend.
/// OpenAI 当前将 transcription 输入固定为 24 kHz PCM，而 Qwen 暴露 sample rate 配置；
/// 因此这里复用一份夹具，并按 backend 做轻量适配。
fn build_audio_for_backend(kind: &str) -> (Vec<u8>, u32) {
    match kind {
        "rt_asr_openai_ws" => (
            upsample_pcm_s16le_mono_16k_to_24k(AUDIO_PCM_16K),
            OPENAI_SAMPLE_RATE_24K,
        ),
        "rt_asr_qwen_ws" => (AUDIO_PCM_16K.to_vec(), FIXTURE_SAMPLE_RATE_16K),
        other => panic!("unsupported realtime backend kind: {other}"),
    }
}

/// Convert mono 16 kHz signed-16 PCM into mono 24 kHz signed-16 PCM with a
/// simple nearest-neighbor resampler. This keeps the live-test fixture small
/// and avoids storing one binary asset per provider sample rate.
/// 用简单的最近邻重采样，把 16 kHz 单声道 s16 PCM 转成 24 kHz 单声道 s16 PCM。
/// 这样可以保持 live-test 夹具精简，避免为不同 provider 采样率各存一份二进制文件。
fn upsample_pcm_s16le_mono_16k_to_24k(src: &[u8]) -> Vec<u8> {
    assert_eq!(src.len() % 2, 0, "pcm fixture must be aligned to i16 samples");
    let samples: Vec<i16> = src
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let out_len = samples.len() * 3 / 2;
    let mut out = Vec::with_capacity(out_len * 2);
    for out_idx in 0..out_len {
        let src_idx = (out_idx * 2) / 3;
        out.extend_from_slice(&samples[src_idx].to_le_bytes());
    }
    out
}

/// Feed PCM bytes into `rtasr_write` with minimal backpressure handling.
/// 向 `rtasr_write` 送入 PCM 字节，并处理最基本的背压。
fn write_pcm(api: &DefaultHostApi, fd: i32, pcm: &[u8]) {
    let mut off = 0;
    while off < pcm.len() {
        let end = std::cmp::min(off + AUDIO_CHUNK_BYTES, pcm.len());
        let rc = api.rtasr_write(fd, &pcm[off..end]);
        if rc == -SPEAR_EAGAIN {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }
        assert!(rc >= 0, "rtasr_write rc={rc}");
        off = end;
    }
}

/// Run one provider-agnostic live transcription pass against the selected backend.
/// 针对选中的 backend 执行一轮 provider-agnostic 的 live 转写验证。
fn run_backend_live_case(backend: &common::ResolvedBackend) {
    let api = build_runtime(backend);
    let model = resolve_model(&backend.kind);
    let (audio_pcm, sample_rate) = build_audio_for_backend(&backend.kind);

    let fd = api.rtasr_create();
    assert!(fd > 0, "backend={} kind={}", backend.name, backend.kind);

    // Use manual segmentation so all providers share the same deterministic turn
    // boundary in the test. Qwen additionally receives `session.finish`.
    // 统一使用 manual segmentation，让不同 provider 在测试里共享确定性的句边界。
    // 其中 Qwen 会额外发送 `session.finish`。
    let segmentation = serde_json::json!({"strategy": "manual"});
    let transport_param = serde_json::json!({"key": "transport", "value": "websocket"});
    let backend_param = serde_json::json!({"key": "backend", "value": backend.name});
    let model_param = serde_json::json!({"key": "model", "value": model});
    let sample_rate_param = serde_json::json!({"key": "sample_rate", "value": sample_rate});

    api.rtasr_ctl(fd, 7, Some(segmentation.to_string().as_bytes()))
        .unwrap();
    api.rtasr_ctl(fd, 1, Some(transport_param.to_string().as_bytes()))
        .unwrap();
    api.rtasr_ctl(fd, 1, Some(backend_param.to_string().as_bytes()))
        .unwrap();
    api.rtasr_ctl(fd, 1, Some(model_param.to_string().as_bytes()))
        .unwrap();
    api.rtasr_ctl(fd, 1, Some(sample_rate_param.to_string().as_bytes()))
        .unwrap();
    api.rtasr_ctl(fd, 2, None).unwrap();

    write_pcm(&api, fd, &audio_pcm);
    api.rtasr_ctl(fd, 5, None).unwrap();

    if backend.kind == "rt_asr_qwen_ws" {
        let finish_event = serde_json::json!({"type": "session.finish"});
        api.rtasr_ctl(fd, 4, Some(finish_event.to_string().as_bytes()))
            .unwrap();
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    let mut transcript = String::new();
    while std::time::Instant::now() < deadline {
        match api.rtasr_read(fd) {
            Ok(bytes) => {
                let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
                if t == "error" {
                    panic!("realtime error for backend={}: {}", backend.name, v);
                }
                if t == "conversation.item.input_audio_transcription.completed" {
                    transcript = v
                        .get("transcript")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if !transcript.trim().is_empty() {
                        break;
                    }
                }
                if t == "conversation.item.input_audio_transcription.delta" {
                    if let Some(delta) = v.get("delta").and_then(|x| x.as_str()) {
                        transcript.push_str(delta);
                    }
                }
                if t == "conversation.item.input_audio_transcription.text" {
                    let text = v.get("text").and_then(|x| x.as_str()).unwrap_or_default();
                    let stash = v.get("stash").and_then(|x| x.as_str()).unwrap_or_default();
                    let combined = format!("{text}{stash}");
                    if !combined.trim().is_empty() {
                        transcript = combined;
                    }
                }
            }
            Err(rc) if rc == -SPEAR_EAGAIN => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(rc) => {
                let status = api.rtasr_ctl(fd, 3, None).ok().flatten();
                panic!(
                    "rtasr_read failed for backend={}: rc={}; status={:?}",
                    backend.name, rc, status
                );
            }
        }
    }

    let normalized = transcript.to_lowercase();
    assert!(
        !normalized.trim().is_empty(),
        "empty transcript for backend={} kind={}",
        backend.name,
        backend.kind
    );
    assert!(
        normalized.contains("hello")
            || normalized.contains("qwen")
            || normalized.contains("test"),
        "unexpected transcript for backend={} kind={}: {}",
        backend.name,
        backend.kind,
        transcript
    );

    api.rtasr_close(fd);
}

#[test]
fn test_all_realtime_asr_live_backends_transcribe_fixture_audio() {
    let backends = common::resolve_all_realtime_asr_backends();
    if backends.is_empty() {
        eprintln!(
            "skipped: no live realtime ASR backends resolved; enable SPEAR_RUN_LIVE_TESTS and export the required env credentials"
        );
        return;
    }

    for backend in &backends {
        run_backend_live_case(backend);
    }
}
