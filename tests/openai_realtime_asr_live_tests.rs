use spear_next::spearlet::config::{LlmBackendConfig, SpearletConfig};
use spear_next::spearlet::execution::host_api::DefaultHostApi;
use spear_next::spearlet::execution::runtime::{ResourcePoolConfig, RuntimeConfig, RuntimeType};
use std::collections::HashMap;

#[test]
fn test_openai_realtime_asr_websocket_connect() {
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("skipped: missing OPENAI_API_KEY");
            return;
        }
    };
    let base_url = match std::env::var("OPENAI_API_BASE") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("skipped: missing OPENAI_API_BASE");
            return;
        }
    };

    let model = std::env::var("OPENAI_STT_MODEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "gpt-4o-mini-transcribe".to_string());

    let mut cfg = SpearletConfig::default();
    let base_url_for_cfg = base_url.clone();
    cfg.llm.backends.push(LlmBackendConfig {
        name: "openai-realtime".to_string(),
        kind: "openai_realtime_ws".to_string(),
        base_url: base_url_for_cfg,
        api_key_env: Some("OPENAI_API_KEY".to_string()),
        weight: 100,
        priority: 0,
        ops: vec!["speech_to_text".to_string()],
        features: vec![],
        transports: vec!["websocket".to_string()],
    });

    let mut global_env = HashMap::new();
    global_env.insert("OPENAI_API_KEY".to_string(), api_key.clone());

    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: global_env,
        spearlet_config: Some(cfg),
        resource_pool: ResourcePoolConfig::default(),
    });

    let fd = api.rtasr_create();
    assert!(fd > 0);

    let autoflush = serde_json::json!({
        "strategy": "server_vad",
        "vad": {"silence_ms": 300}
    });
    let autoflush_bytes = serde_json::to_vec(&autoflush).unwrap();
    api.rtasr_ctl(fd, 7, Some(&autoflush_bytes)).unwrap();

    let transport_param = serde_json::json!({"key": "transport", "value": "websocket"});
    let backend_param = serde_json::json!({"key": "backend", "value": "openai-realtime"});
    let model_param = serde_json::json!({"key": "model", "value": model});
    api.rtasr_ctl(fd, 1, Some(transport_param.to_string().as_bytes()))
        .unwrap();
    api.rtasr_ctl(fd, 1, Some(backend_param.to_string().as_bytes()))
        .unwrap();
    api.rtasr_ctl(fd, 1, Some(model_param.to_string().as_bytes()))
        .unwrap();

    api.rtasr_ctl(fd, 2, None).unwrap();

    let phrase = "Hello, this is a test.";
    let tts_model = std::env::var("OPENAI_TTS_MODEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "gpt-4o-mini-tts".to_string());
    let voice = std::env::var("OPENAI_TTS_VOICE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "alloy".to_string());

    let audio_pcm = {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let url = format!("{}/audio/speech", base_url.trim_end_matches('/'));
        let api_key = api_key.clone();
        rt.block_on(async move {
            let client = reqwest::Client::new();
            let resp = client
                .post(url)
                .header("authorization", format!("Bearer {}", api_key))
                .header("content-type", "application/json")
                .json(&serde_json::json!({
                    "model": tts_model,
                    "voice": voice,
                    "input": phrase,
                    "response_format": "pcm",
                }))
                .send()
                .await
                .unwrap();
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                panic!("tts failed: {}: {}", status.as_u16(), body);
            }
            resp.bytes().await.unwrap().to_vec()
        })
    };

    let mut off = 0;
    let chunk = 4800;
    while off < audio_pcm.len() {
        let end = std::cmp::min(off + chunk, audio_pcm.len());
        let rc = api.rtasr_write(fd, &audio_pcm[off..end]);
        if rc == -libc::EAGAIN {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }
        assert!(rc >= 0);
        off = end;
    }

    let silence_bytes = vec![0u8; 24000];
    let mut off = 0;
    while off < silence_bytes.len() {
        let end = std::cmp::min(off + chunk, silence_bytes.len());
        let rc = api.rtasr_write(fd, &silence_bytes[off..end]);
        if rc == -libc::EAGAIN {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }
        assert!(rc >= 0);
        off = end;
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    let mut got_transcript = None::<String>;
    while std::time::Instant::now() < deadline {
        match api.rtasr_read(fd) {
            Ok(bytes) => {
                let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
                if t == "error" {
                    panic!("realtime error: {}", v);
                }
                if t == "conversation.item.input_audio_transcription.completed" {
                    got_transcript = v
                        .get("transcript")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    if got_transcript.is_some() {
                        break;
                    }
                }
                if t == "conversation.item.input_audio_transcription.delta" {
                    got_transcript = v
                        .get("delta")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                }
            }
            Err(rc) if rc == -libc::EAGAIN => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(rc) => {
                let status = api.rtasr_ctl(fd, 3, None).ok().flatten();
                panic!("rtasr_read failed: rc={}; status={:?}", rc, status);
            }
        }
    }

    let transcript = got_transcript.unwrap_or_default();
    let l = transcript.to_lowercase();
    assert!(l.contains("hello"));
    assert!(l.contains("test"));

    api.rtasr_close(fd);
}
