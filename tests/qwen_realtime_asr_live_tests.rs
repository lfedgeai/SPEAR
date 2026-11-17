use spear_next::spearlet::config::{AiBackendConfig, AiCredentialConfig, SpearletConfig};
use spear_next::spearlet::execution::host_api::DefaultHostApi;
use spear_next::spearlet::execution::runtime::{ResourcePoolConfig, RuntimeConfig, RuntimeType};
use std::collections::HashMap;

mod common;

/// Minimal live E2E for Qwen realtime ASR.
/// Qwen realtime ASR 的最小 live E2E。
///
/// Required setup:
/// - `SPEAR_RUN_LIVE_TESTS=1`
/// - a `rt_asr_qwen_ws` backend in `config/spearlet/config.toml` or `SPEAR_TEST_CONFIG`
/// - an `env` credential referenced by that backend, with the real key exported in the shell
///
/// Optional env vars:
/// - `QWEN_LIVE_MODEL` (default: `qwen3-asr-flash-realtime`)
///
/// Example:
/// `SPEAR_RUN_LIVE_TESTS=1 DASHSCOPE_API_KEY=... cargo test test_qwen_realtime_asr_live_transcribes_fixture_audio --test qwen_realtime_asr_live_tests -- --nocapture`
///
/// 必需前置条件：
/// - `SPEAR_RUN_LIVE_TESTS=1`
/// - 在 `config/spearlet/config.toml` 或 `SPEAR_TEST_CONFIG` 中配置一个 `rt_asr_qwen_ws` backend
/// - 该 backend 引用 `env` 类型凭据，并在 shell 中导出真实 key
///
/// 可选环境变量：
/// - `QWEN_LIVE_MODEL`（默认：`qwen3-asr-flash-realtime`）
///
/// 运行示例：
/// `SPEAR_RUN_LIVE_TESTS=1 DASHSCOPE_API_KEY=... cargo test test_qwen_realtime_asr_live_transcribes_fixture_audio --test qwen_realtime_asr_live_tests -- --nocapture`
const SPEAR_EAGAIN: i32 = 11;
// Realtime ASR model used unless the caller overrides it via env.
// 若调用方未通过环境变量覆盖，则使用该 realtime ASR 模型。
const DEFAULT_QWEN_MODEL: &str = "qwen3-asr-flash-realtime";
// The embedded fixture is PCM mono 16 kHz, so the session config must match it.
// 内置夹具为 16 kHz 单声道 PCM，因此会话配置必须与之匹配。
const SAMPLE_RATE_HZ: u32 = 16_000;
// Small chunks reduce queue spikes while still keeping the live test fast.
// 较小的 chunk 可降低队列尖峰，同时保持 live test 足够快。
const AUDIO_CHUNK_BYTES: usize = 4_800;
// Embedded self-contained audio fixture saying "hello this is a qwen live e2e test".
// 内置自包含音频夹具，内容大致为 “hello this is a qwen live e2e test”。
const AUDIO_PCM: &[u8] = include_bytes!("fixtures/qwen_live_e2e_hello.pcm");

/// Build one minimal runtime config that exercises the repository's RTASR path.
/// 构造最小运行时配置，走仓库自身的 RTASR 接入链路。
fn build_qwen_runtime(
    backend_name: String,
    api_key_env: String,
    api_key: String,
    base_url: String,
) -> DefaultHostApi {
    let mut cfg = SpearletConfig::default();
    cfg.ai.credentials.push(AiCredentialConfig {
        name: "qwen_live".to_string(),
        kind: "env".to_string(),
        api_key_env: api_key_env.clone(),
    });
    cfg.ai.backends.push(AiBackendConfig {
        name: backend_name,
        kind: "rt_asr_qwen_ws".to_string(),
        base_url,
        hosting: None,
        model: None,
        credential_ref: Some("qwen_live".to_string()),
        provider: None,
        origin: None,
        deployment_id: None,
        weight: 100,
        priority: 0,
        ops: vec!["speech_to_text".to_string()],
        features: vec![],
        transports: vec!["websocket".to_string()],
    });

    // Inject the secret into the runtime environment so the env credential path
    // is exercised exactly like production host-side resolution.
    // 将 secret 注入运行时环境，使 env credential 路径与生产 host 侧解析方式一致。
    let mut global_env = HashMap::new();
    global_env.insert(api_key_env, api_key);

    DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: global_env,
        spearlet_config: Some(cfg),
        resource_pool: ResourcePoolConfig::default(),
    })
}

/// Feed raw PCM bytes into rtasr_write with simple backpressure handling.
/// 向 rtasr_write 写入原始 PCM 字节，并处理最基本的背压。
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

#[test]
fn test_qwen_realtime_asr_live_transcribes_fixture_audio() {
    let resolved = match common::resolve_qwen_realtime_asr_backend() {
        Some(v) => v,
        None => {
            eprintln!(
                "skipped: missing rt_asr_qwen_ws backend or env credential in live test config"
            );
            return;
        }
    };

    if resolved.kind != "rt_asr_qwen_ws" {
        eprintln!(
            "skipped: resolved backend kind is {}, expected rt_asr_qwen_ws",
            resolved.kind
        );
        return;
    }

    let model = std::env::var("QWEN_LIVE_MODEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_QWEN_MODEL.to_string());
    let backend_name = resolved.name.clone();
    let api = build_qwen_runtime(
        resolved.name,
        resolved.api_key_env,
        resolved.api_key,
        resolved.base_url,
    );

    let fd = api.rtasr_create();
    assert!(fd > 0);

    // Force manual mode so the test owns the turn boundary and can finish deterministically.
    // 强制使用 manual 模式，让测试自己控制句边界与结束时机。
    let segmentation = serde_json::json!({"strategy": "manual"});
    let transport_param = serde_json::json!({"key": "transport", "value": "websocket"});
    let backend_param = serde_json::json!({"key": "backend", "value": backend_name});
    let model_param = serde_json::json!({"key": "model", "value": model});
    let sample_rate_param =
        serde_json::json!({"key": "sample_rate", "value": SAMPLE_RATE_HZ});

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

    write_pcm(&api, fd, AUDIO_PCM);
    api.rtasr_ctl(fd, 5, None).unwrap();

    let finish_event = serde_json::json!({"type": "session.finish"});
    api.rtasr_ctl(fd, 4, Some(finish_event.to_string().as_bytes()))
        .unwrap();

    // Allow enough time for the remote service to complete the turn and emit
    // both the final transcript and `session.finished`.
    // 预留足够时间等待远端完成本轮识别，并返回最终 transcript 与 `session.finished`。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut transcript = None::<String>;
    let mut saw_session_finished = false;
    while std::time::Instant::now() < deadline {
        match api.rtasr_read(fd) {
            Ok(bytes) => {
                let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
                if t == "error" {
                    panic!("realtime error: {}", v);
                }
                if t == "conversation.item.input_audio_transcription.completed" {
                    transcript = v
                        .get("transcript")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                }
                if t == "session.finished" {
                    saw_session_finished = true;
                    if transcript.is_some() {
                        break;
                    }
                }
            }
            Err(rc) if rc == -SPEAR_EAGAIN => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(rc) => {
                let status = api.rtasr_ctl(fd, 3, None).ok().flatten();
                panic!("rtasr_read failed: rc={}; status={:?}", rc, status);
            }
        }
    }

    let transcript = transcript.unwrap_or_default();
    let normalized = transcript.to_lowercase();
    assert!(!normalized.trim().is_empty(), "transcript={transcript}");
    assert!(
        normalized.contains("hello")
            || normalized.contains("qwen")
            || normalized.contains("test"),
        "unexpected transcript={transcript}"
    );
    assert!(saw_session_finished, "session.finished not observed");

    api.rtasr_close(fd);
}
