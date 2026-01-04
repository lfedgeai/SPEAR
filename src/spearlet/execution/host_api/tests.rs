use super::*;
use crate::spearlet::execution::hostcall::fd_table::EP_CTL_ADD;
use crate::spearlet::execution::hostcall::types::PollEvents;
use crate::spearlet::execution::runtime::{ResourcePoolConfig, RuntimeConfig, RuntimeType};
use std::collections::HashMap;

#[test]
fn test_cchat_send_pipeline_stub_backend() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let fd = api.cchat_create();
    assert!(fd > 0);
    assert_eq!(
        api.cchat_write_msg(fd, "user".to_string(), "hello".to_string()),
        0
    );
    let resp_fd = api.cchat_send(fd, 0).unwrap();
    let bytes = api.cchat_recv(resp_fd).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
    assert!(content.contains("hello"));
}

#[test]
fn test_configured_openai_backend_missing_key_is_filtered() {
    let mut cfg = crate::spearlet::config::SpearletConfig::default();
    cfg.llm
        .backends
        .push(crate::spearlet::config::LlmBackendConfig {
            name: "openai-us".to_string(),
            kind: "openai_compatible".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key_env: Some("OPENAI_API_KEY".to_string()),
            weight: 100,
            priority: 0,
            ops: vec!["chat_completions".to_string()],
            features: vec![],
            transports: vec!["http".to_string()],
        });

    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: Some(cfg),
        resource_pool: ResourcePoolConfig::default(),
    });

    let fd = api.cchat_create();
    assert_eq!(
        api.cchat_write_msg(fd, "user".to_string(), "hello".to_string()),
        0
    );
    let resp_fd = api.cchat_send(fd, 0).unwrap();
    let bytes = api.cchat_recv(resp_fd).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.get("choices").is_some());
}

#[test]
fn test_cchat_response_fd_is_epollin() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let epfd = api.spear_ep_create();
    let sess_fd = api.cchat_create();
    assert_eq!(
        api.cchat_write_msg(sess_fd, "user".to_string(), "hello".to_string()),
        0
    );
    let resp_fd = api.cchat_send(sess_fd, 0).unwrap();

    assert_eq!(
        api.spear_ep_ctl(epfd, EP_CTL_ADD, resp_fd, PollEvents::IN.bits() as i32),
        0
    );
    let ready = api.spear_ep_wait_ready(epfd, 0).unwrap();
    assert!(ready
        .iter()
        .any(|(fd, ev)| *fd == resp_fd && ((*ev as u32) & PollEvents::IN.bits()) != 0));
}

#[test]
fn test_close_makes_epollhup() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let epfd = api.spear_ep_create();
    let sess_fd = api.cchat_create();
    assert_eq!(
        api.cchat_write_msg(sess_fd, "user".to_string(), "hello".to_string()),
        0
    );
    let resp_fd = api.cchat_send(sess_fd, 0).unwrap();
    assert_eq!(
        api.spear_ep_ctl(
            epfd,
            EP_CTL_ADD,
            resp_fd,
            PollEvents::IN.or(PollEvents::HUP).bits() as i32
        ),
        0
    );

    assert_eq!(api.cchat_close(resp_fd), 0);
    let ready = api.spear_ep_wait_ready(epfd, 0).unwrap();
    assert!(ready
        .iter()
        .any(|(fd, ev)| *fd == resp_fd && ((*ev as u32) & PollEvents::HUP.bits()) != 0));
}

#[test]
fn test_rtasr_write_backpressure_and_epollout() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let epfd = api.spear_ep_create();
    let fd = api.rtasr_create();

    let cfg =
        serde_json::to_vec(&serde_json::json!({"key":"max_send_queue_bytes","value":16}))
            .unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&cfg)).unwrap();

    assert_eq!(
        api.spear_ep_ctl(epfd, EP_CTL_ADD, fd, PollEvents::OUT.bits() as i32),
        0
    );
    let ready = api.spear_ep_wait_ready(epfd, 0).unwrap();
    assert!(ready
        .iter()
        .any(|(rfd, ev)| *rfd == fd && ((*ev as u32) & PollEvents::OUT.bits()) != 0));

    let bytes = vec![1u8; 16];
    assert_eq!(api.rtasr_write(fd, &bytes), 16);
    let ready2 = api.spear_ep_wait_ready(epfd, 0).unwrap();
    assert!(ready2.is_empty());

    let one = vec![2u8; 1];
    assert_eq!(api.rtasr_write(fd, &one), -libc::EAGAIN);
}

#[tokio::test]
async fn test_rtasr_read_epollin_and_eagain() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let epfd = api.spear_ep_create();
    let fd = api.rtasr_create();
    assert_eq!(
        api.spear_ep_ctl(
            epfd,
            EP_CTL_ADD,
            fd,
            PollEvents::IN
                .or(PollEvents::ERR)
                .or(PollEvents::HUP)
                .bits() as i32
        ),
        0
    );

    let cfg =
        serde_json::to_vec(&serde_json::json!({"key":"stub_event_interval_ms","value":20}))
            .unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&cfg)).unwrap();
    let _ = api.rtasr_ctl(fd, 2, None).unwrap();

    let api2 = api.clone();
    let ready = tokio::task::spawn_blocking(move || api2.spear_ep_wait_ready(epfd, 500))
        .await
        .unwrap()
        .unwrap();
    assert!(ready
        .iter()
        .any(|(rfd, ev)| *rfd == fd && ((*ev as u32) & PollEvents::IN.bits()) != 0));

    let mut got_one = false;
    for _ in 0..20 {
        match api.rtasr_read(fd) {
            Ok(bytes) => {
                got_one = true;
                let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                assert!(v.get("type").is_some());
            }
            Err(e) => {
                assert_eq!(e, -libc::EAGAIN);
                break;
            }
        }
    }
    assert!(got_one);
}

#[tokio::test]
async fn test_rtasr_websocket_transport_receives_events() {
    use futures::{SinkExt, StreamExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let (mut w, mut r) = ws.split();
        let _ = r.next().await;
        let msg = serde_json::json!({
            "type": "conversation.item.input_audio_transcription.delta",
            "delta": "hello",
        });
        w.send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&msg).unwrap(),
        ))
        .await
        .unwrap();
    });

    let mut cfg = crate::spearlet::config::SpearletConfig::default();
    cfg.llm
        .backends
        .push(crate::spearlet::config::LlmBackendConfig {
            name: "rt-ws".to_string(),
            kind: "openai_realtime_ws".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key_env: None,
            weight: 100,
            priority: 0,
            ops: vec!["speech_to_text".to_string()],
            features: vec![],
            transports: vec!["websocket".to_string()],
        });

    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: Some(cfg),
        resource_pool: ResourcePoolConfig::default(),
    });

    let epfd = api.spear_ep_create();
    let fd = api.rtasr_create();
    assert_eq!(
        api.spear_ep_ctl(epfd, EP_CTL_ADD, fd, PollEvents::IN.bits() as i32),
        0
    );

    let ws_url = format!("ws://{}/v1/realtime?intent=transcription", addr);

    let p1 =
        serde_json::to_vec(&serde_json::json!({"key":"transport","value":"websocket"}))
            .unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p1)).unwrap();
    let p2 = serde_json::to_vec(&serde_json::json!({"key":"backend","value":"rt-ws"})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p2)).unwrap();
    let p3 = serde_json::to_vec(&serde_json::json!({"key":"ws_url","value":ws_url})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p3)).unwrap();
    let p4 = serde_json::to_vec(&serde_json::json!({"key":"client_secret","value":"dummy"})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p4)).unwrap();

    let _ = api.rtasr_ctl(fd, 2, None).unwrap();
    assert_eq!(api.rtasr_write(fd, b"abc"), 3);

    let api2 = api.clone();
    let ready = tokio::task::spawn_blocking(move || api2.spear_ep_wait_ready(epfd, 500))
        .await
        .unwrap()
        .unwrap();
    assert!(ready
        .iter()
        .any(|(rfd, ev)| *rfd == fd && ((*ev as u32) & PollEvents::IN.bits()) != 0));

    let bytes = api.rtasr_read(fd).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("type").and_then(|x| x.as_str()).unwrap_or(""),
        "conversation.item.input_audio_transcription.delta"
    );

    server.await.unwrap();
}

#[test]
fn test_rtasr_autoflush_set_and_get() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let fd = api.rtasr_create();
    let cfg = serde_json::json!({
        "strategy": "client_commit",
        "flush_on_close": false,
        "client_commit": {
            "max_buffer_bytes": 123,
            "min_flush_gap_ms": 7,
        }
    });
    let bytes = serde_json::to_vec(&cfg).unwrap();
    api.rtasr_ctl(fd, 7, Some(&bytes)).unwrap();
    let got = api.rtasr_ctl(fd, 8, None).unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&got).unwrap();
    assert_eq!(
        v.get("strategy").and_then(|x| x.as_str()),
        Some("client_commit")
    );
    assert_eq!(
        v.get("client_commit")
            .and_then(|x| x.get("max_buffer_bytes"))
            .and_then(|x| x.as_u64()),
        Some(123)
    );
    assert_eq!(
        v.get("client_commit")
            .and_then(|x| x.get("min_flush_gap_ms"))
            .and_then(|x| x.as_u64()),
        Some(7)
    );
}

#[test]
fn test_rtasr_autoflush_legacy_payload_is_accepted() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let fd = api.rtasr_create();
    let cfg = serde_json::json!({
        "enabled": true,
        "mode": "bytes",
        "max_buffer_bytes": 123,
        "min_flush_gap_ms": 7,
        "flush_on_close": false,
    });
    let bytes = serde_json::to_vec(&cfg).unwrap();
    api.rtasr_ctl(fd, 7, Some(&bytes)).unwrap();
    let got = api.rtasr_ctl(fd, 8, None).unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&got).unwrap();
    assert_eq!(
        v.get("strategy").and_then(|x| x.as_str()),
        Some("client_commit")
    );
    assert_eq!(
        v.get("client_commit")
            .and_then(|x| x.get("max_buffer_bytes"))
            .and_then(|x| x.as_u64()),
        Some(123)
    );
}

#[test]
fn test_rtasr_default_segmentation_is_server_vad() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let fd = api.rtasr_create();
    let got = api.rtasr_ctl(fd, 8, None).unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&got).unwrap();
    assert_eq!(v.get("strategy").and_then(|x| x.as_str()), Some("server_vad"));
    assert_eq!(
        v.get("vad")
            .and_then(|x| x.get("silence_ms"))
            .and_then(|x| x.as_u64()),
        Some(500)
    );
}

#[tokio::test]
async fn test_rtasr_websocket_flush_sends_commit() {
    use futures::{SinkExt, StreamExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let (mut w, mut r) = ws.split();

        let mut commit_seen = false;
        while let Some(msg) = r.next().await {
            let msg = msg.unwrap();
            let s = match msg {
                tokio_tungstenite::tungstenite::Message::Text(s) => s,
                tokio_tungstenite::tungstenite::Message::Binary(b) => {
                    String::from_utf8_lossy(&b).to_string()
                }
                _ => continue,
            };
            let v: serde_json::Value = serde_json::from_str(&s).unwrap_or_default();
            let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
            if t == "input_audio_buffer.commit" {
                commit_seen = true;
                break;
            }
        }

        if commit_seen {
            let msg = serde_json::json!({
                "type": "conversation.item.input_audio_transcription.delta",
                "delta": "hello",
            });
            let _ = w
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    serde_json::to_string(&msg).unwrap(),
                ))
                .await;
        }

        let _ = tx.send(commit_seen);
    });

    let mut cfg = crate::spearlet::config::SpearletConfig::default();
    cfg.llm
        .backends
        .push(crate::spearlet::config::LlmBackendConfig {
            name: "rt-ws".to_string(),
            kind: "openai_realtime_ws".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key_env: None,
            weight: 100,
            priority: 0,
            ops: vec!["speech_to_text".to_string()],
            features: vec![],
            transports: vec!["websocket".to_string()],
        });

    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: Some(cfg),
        resource_pool: ResourcePoolConfig::default(),
    });

    let epfd = api.spear_ep_create();
    let fd = api.rtasr_create();
    assert_eq!(
        api.spear_ep_ctl(epfd, EP_CTL_ADD, fd, PollEvents::IN.bits() as i32),
        0
    );

    let ws_url = format!("ws://{}/v1/realtime?model=gpt-realtime", addr);
    let p1 =
        serde_json::to_vec(&serde_json::json!({"key":"transport","value":"websocket"}))
            .unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p1)).unwrap();
    let p2 = serde_json::to_vec(&serde_json::json!({"key":"backend","value":"rt-ws"})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p2)).unwrap();
    let p3 = serde_json::to_vec(&serde_json::json!({"key":"ws_url","value":ws_url})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p3)).unwrap();
    let p4 = serde_json::to_vec(&serde_json::json!({"key":"client_secret","value":"dummy"})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p4)).unwrap();

    let _ = api.rtasr_ctl(fd, 2, None).unwrap();
    assert_eq!(api.rtasr_write(fd, b"abc"), 3);
    let _ = api.rtasr_ctl(fd, 5, None).unwrap();

    let api2 = api.clone();
    let ready = tokio::task::spawn_blocking(move || api2.spear_ep_wait_ready(epfd, 1000))
        .await
        .unwrap()
        .unwrap();
    assert!(ready
        .iter()
        .any(|(rfd, ev)| *rfd == fd && ((*ev as u32) & PollEvents::IN.bits()) != 0));

    let bytes = api.rtasr_read(fd).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("type").and_then(|x| x.as_str()).unwrap_or(""),
        "conversation.item.input_audio_transcription.delta"
    );

    assert!(rx.await.unwrap());
    server.await.unwrap();
}

#[tokio::test]
async fn test_rtasr_websocket_autoflush_bytes_sends_commit() {
    use futures::{SinkExt, StreamExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<serde_json::Value>();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let (mut w, mut r) = ws.split();

        let first = r.next().await.unwrap().unwrap();
        let s = match first {
            tokio_tungstenite::tungstenite::Message::Text(s) => s,
            tokio_tungstenite::tungstenite::Message::Binary(b) => {
                String::from_utf8_lossy(&b).to_string()
            }
            _ => "{}".to_string(),
        };
        let v: serde_json::Value = serde_json::from_str(&s).unwrap_or_default();

        let msg = serde_json::json!({
            "type": "conversation.item.input_audio_transcription.delta",
            "delta": "hello",
        });
        let _ = w
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::to_string(&msg).unwrap(),
            ))
            .await;

        let _ = tx.send(v);
    });

    let mut cfg = crate::spearlet::config::SpearletConfig::default();
    cfg.llm
        .backends
        .push(crate::spearlet::config::LlmBackendConfig {
            name: "rt-ws".to_string(),
            kind: "openai_realtime_ws".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key_env: None,
            weight: 100,
            priority: 0,
            ops: vec!["speech_to_text".to_string()],
            features: vec![],
            transports: vec!["websocket".to_string()],
        });

    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: Some(cfg),
        resource_pool: ResourcePoolConfig::default(),
    });

    let epfd = api.spear_ep_create();
    let fd = api.rtasr_create();
    assert_eq!(
        api.spear_ep_ctl(epfd, EP_CTL_ADD, fd, PollEvents::IN.bits() as i32),
        0
    );

    let autoflush = serde_json::json!({
        "strategy": "server_vad",
        "vad": {
            "silence_ms": 321
        }
    });
    let autoflush_bytes = serde_json::to_vec(&autoflush).unwrap();
    api.rtasr_ctl(fd, 7, Some(&autoflush_bytes)).unwrap();

    let ws_url = format!("ws://{}/v1/realtime?model=gpt-realtime", addr);
    let p1 =
        serde_json::to_vec(&serde_json::json!({"key":"transport","value":"websocket"}))
            .unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p1)).unwrap();
    let p2 = serde_json::to_vec(&serde_json::json!({"key":"backend","value":"rt-ws"})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p2)).unwrap();
    let p3 = serde_json::to_vec(&serde_json::json!({"key":"ws_url","value":ws_url})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p3)).unwrap();
    let p4 = serde_json::to_vec(&serde_json::json!({"key":"client_secret","value":"dummy"})).unwrap();
    let _ = api.rtasr_ctl(fd, 1, Some(&p4)).unwrap();

    let _ = api.rtasr_ctl(fd, 2, None).unwrap();
    assert_eq!(api.rtasr_write(fd, b"abc"), 3);

    let api2 = api.clone();
    let ready = tokio::task::spawn_blocking(move || api2.spear_ep_wait_ready(epfd, 1000))
        .await
        .unwrap()
        .unwrap();
    assert!(ready
        .iter()
        .any(|(rfd, ev)| *rfd == fd && ((*ev as u32) & PollEvents::IN.bits()) != 0));

    let bytes = api.rtasr_read(fd).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("type").and_then(|x| x.as_str()).unwrap_or(""),
        "conversation.item.input_audio_transcription.delta"
    );

    let first = rx.await.unwrap();
    assert_eq!(
        first.get("type").and_then(|x| x.as_str()),
        Some("session.update")
    );
    let turn = first
        .get("session")
        .and_then(|x| x.get("turn_detection"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    assert_eq!(turn.get("type").and_then(|x| x.as_str()), Some("server_vad"));
    assert_eq!(
        turn.get("silence_duration_ms").and_then(|x| x.as_u64()),
        Some(321)
    );
    server.await.unwrap();
}

#[tokio::test]
async fn test_mic_read_epollin_and_close_hup() {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });

    let epfd = api.spear_ep_create();
    let mic_fd = api.mic_create();
    assert_eq!(
        api.spear_ep_ctl(
            epfd,
            EP_CTL_ADD,
            mic_fd,
            PollEvents::IN
                .or(PollEvents::HUP)
                .or(PollEvents::ERR)
                .bits() as i32
        ),
        0
    );

    let mic_cfg = serde_json::to_vec(&serde_json::json!({
        "sample_rate_hz": 24000,
        "channels": 1,
        "format": "pcm16",
        "frame_ms": 20
    }))
    .unwrap();
    let _ = api.mic_ctl(mic_fd, 1, Some(&mic_cfg)).unwrap();

    let api2 = api.clone();
    let ready = tokio::task::spawn_blocking(move || api2.spear_ep_wait_ready(epfd, 500))
        .await
        .unwrap()
        .unwrap();
    assert!(ready
        .iter()
        .any(|(rfd, ev)| *rfd == mic_fd && ((*ev as u32) & PollEvents::IN.bits()) != 0));

    let bytes = api.mic_read(mic_fd).unwrap();
    assert!(!bytes.is_empty());

    assert_eq!(api.mic_close(mic_fd), 0);
    let api3 = api.clone();
    let ready2 = tokio::task::spawn_blocking(move || api3.spear_ep_wait_ready(epfd, 200))
        .await
        .unwrap()
        .unwrap();
    assert!(ready2
        .iter()
        .any(|(rfd, ev)| *rfd == mic_fd && ((*ev as u32) & PollEvents::HUP.bits()) != 0));
}
