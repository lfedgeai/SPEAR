#[cfg(feature = "mic-device")]
fn main() {
    use cpal::traits::{DeviceTrait, HostTrait};
    use spear_next::spearlet::execution::host_api::DefaultHostApi;
    use spear_next::spearlet::execution::hostcall::types::PollEvents;
    use spear_next::spearlet::execution::runtime::{
        ResourcePoolConfig, RuntimeConfig, RuntimeType,
    };
    use std::collections::HashMap;

    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_else(|| "<none>".to_string());
    println!("default_input_device: {}", default_name);

    if let Ok(devs) = host.input_devices() {
        for d in devs {
            let name = d.name().unwrap_or_else(|_| "<unknown>".to_string());
            println!("input_device: {}", name);
        }
    }

    let cfg = RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    };
    let api = DefaultHostApi::new(cfg);

    let epfd = api.spear_ep_create();
    let mic_fd = api.mic_create();
    let rc = api.spear_ep_ctl(epfd, 1, mic_fd, PollEvents::IN.bits() as i32);
    println!("epoll_ctl(add mic_fd): {}", rc);

    let device_name = std::env::var("SPEAR_MIC_DEVICE_NAME").ok();
    let mic_cfg = serde_json::to_vec(&serde_json::json!({
        "sample_rate_hz": 24000,
        "channels": 1,
        "format": "pcm16",
        "frame_ms": 20,
        "source": "device",
        "device": device_name.as_ref().map(|n| serde_json::json!({"name": n})),
        "fallback": {"to_stub": false}
    }))
    .unwrap();

    match api.mic_ctl(mic_fd, 1, Some(&mic_cfg)) {
        Ok(_) => println!("mic_ctl ok"),
        Err(e) => {
            println!("mic_ctl err: {}", e);
            std::process::exit(2);
        }
    }

    let ready = api.spear_ep_wait_ready(epfd, 2000).unwrap_or_default();
    let has_in = ready
        .iter()
        .any(|(rfd, ev)| *rfd == mic_fd && ((*ev as u32) & PollEvents::IN.bits()) != 0);
    println!("epoll_ready: {}", has_in);

    let bytes = match api.mic_read(mic_fd) {
        Ok(b) => b,
        Err(e) => {
            println!("mic_read err: {}", e);
            std::process::exit(3);
        }
    };

    let expected = 24000usize * 20 / 1000 * 2;
    println!("mic_read bytes: {} (expected: {})", bytes.len(), expected);
    if bytes.len() >= 2 {
        let mut sum_sq = 0f64;
        let mut n = 0u64;
        for chunk in bytes.chunks_exact(2) {
            let v = i16::from_le_bytes([chunk[0], chunk[1]]) as f64;
            sum_sq += v * v;
            n += 1;
        }
        let rms = (sum_sq / (n.max(1) as f64)).sqrt();
        println!("pcm16 rms: {:.2}", rms);
    }

    let _ = api.mic_close(mic_fd);
    let _ = api.spear_ep_close(epfd);
}

#[cfg(not(feature = "mic-device"))]
fn main() {
    println!("mic_device_probe requires --features mic-device");
    std::process::exit(1);
}
