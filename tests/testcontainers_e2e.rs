// E2E test using testcontainers to run SMS and SPEARlet in Docker
// 端到端测试：使用testcontainers在Docker中运行SMS与SPEARlet并建立连接
// Run with Docker installed; test is ignored by default
// 运行需安装Docker；默认忽略该测试

use std::path::PathBuf;
use std::time::Duration;

use std::process::Command;
use testcontainers::{clients, core::WaitFor, GenericImage, RunnableImage};

fn target_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        PathBuf::from(dir)
    } else {
        PathBuf::from("target")
    }
}

fn binary_path(name: &str) -> PathBuf {
    // Prefer externally provided binary directory (Linux target) / 优先使用外部提供的二进制目录（Linux目标）
    if let Ok(dir) = std::env::var("E2E_BIN_DIR") {
        let mut p = PathBuf::from(dir);
        p.push(name);
        return p;
    }
    // Fallback to debug / 回退到debug
    let mut p = target_dir();
    p.push("debug");
    p.push(name);
    p
}

#[test]
#[ignore]
fn e2e_spearlet_connects_to_sms_with_testcontainers() {
    // Skip on non-Linux hosts unless E2E_BIN_DIR is provided (Linux binary) / 非Linux主机跳过，除非提供E2E_BIN_DIR（Linux二进制）
    if std::env::consts::OS != "linux" && std::env::var("E2E_BIN_DIR").is_err() {
        eprintln!("E2E requires Linux binaries. Set E2E_BIN_DIR to Linux-target binaries (e.g., target/x86_64-unknown-linux-musl/release). Skipping.");
        return;
    }
    // Check docker availability / 检查docker可用性
    if std::process::Command::new("docker")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("Docker not available, skipping E2E test");
        return;
    }
    if std::process::Command::new("docker")
        .arg("info")
        .output()
        .is_err()
    {
        eprintln!("Docker daemon not reachable, skipping E2E test");
        return;
    }

    let docker = clients::Cli::default();

    // Paths to host-built binaries / 宿主机已构建的二进制路径
    let sms_bin = binary_path("sms");
    let spearlet_bin = binary_path("spearlet");
    println!(
        "Using binaries: sms={:?}, spearlet={:?}",
        sms_bin, spearlet_bin
    );
    assert!(sms_bin.exists(), "sms binary not found at {:?}", sms_bin);
    assert!(
        spearlet_bin.exists(),
        "spearlet binary not found at {:?}",
        spearlet_bin
    );

    // Cleanup existing containers / 清理已有容器
    let _ = Command::new("docker")
        .args(["rm", "-f", "spear-e2e-sms", "spear-e2e-spearlet"])
        .output();

    // Start SMS container / 启动SMS容器
    let sms_name = "spear-e2e-sms";
    let sms_image = GenericImage::new("debian", "bookworm-slim")
        .with_exposed_port(50051)
        .with_exposed_port(8080)
        .with_wait_for(WaitFor::message_on_stdout("SMS gRPC server listening"))
        .with_volume(sms_bin.to_string_lossy().to_string(), "/usr/local/bin/sms")
        .with_env_var("SMS_GRPC_ADDR", "0.0.0.0:50051")
        .with_env_var("SMS_HTTP_ADDR", "0.0.0.0:8080")
        .with_env_var("RUST_LOG", "info")
        .with_entrypoint("/usr/local/bin/sms");

    // Attach to user-defined network at creation / 在创建时附加到用户自定义网络
    let _sms_container = match std::panic::catch_unwind(|| {
        docker.run(
            RunnableImage::from(sms_image)
                .with_container_name(sms_name)
                .with_network("spear-e2e-net"),
        )
    }) {
        Ok(c) => c,
        Err(_) => {
            let out = Command::new("docker")
                .args(["logs", sms_name])
                .output()
                .ok();
            let logs = out
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();
            panic!("Failed to start SMS container. Logs:\n{}", logs);
        }
    };

    // Brief wait to ensure SMS is up / 等待SMS启动
    std::thread::sleep(Duration::from_millis(300));

    // Start SPEARlet container to connect to SMS via host gateway / 启动SPEARlet容器并通过宿主网关连接SMS
    // testcontainers injects host.testcontainers.internal -> docker host
    let spear_name = "spear-e2e-spearlet";
    let spear_image = GenericImage::new("debian", "bookworm-slim")
        .with_volume(
            spearlet_bin.to_string_lossy().to_string(),
            "/usr/local/bin/spearlet",
        )
        .with_env_var("RUST_LOG", "info")
        .with_env_var("SPEARLET_AUTO_REGISTER", "true")
        .with_env_var("SPEARLET_SMS_ADDR", format!("{}:{}", sms_name, 50051))
        .with_entrypoint("/usr/local/bin/spearlet");

    let _spear_container = match std::panic::catch_unwind(|| {
        docker.run(
            RunnableImage::from(spear_image)
                .with_container_name(spear_name)
                .with_network("spear-e2e-net"),
        )
    }) {
        Ok(c) => c,
        Err(_) => {
            let out = Command::new("docker")
                .args(["logs", spear_name])
                .output()
                .ok();
            let logs = out
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();
            panic!("Failed to start SPEARlet container. Logs:\n{}", logs);
        }
    };

    let start = std::time::Instant::now();
    loop {
        let out = Command::new("docker")
            .args(["logs", spear_name])
            .output()
            .ok();
        let logs = out
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if logs.contains("Connected to SMS successfully") {
            break;
        }
        if start.elapsed() > Duration::from_secs(30) {
            panic!("Failed to start SPEARlet container. Logs:\n{}", logs);
        }
        std::thread::sleep(Duration::from_millis(300));
    }

    // Print logs for visibility when running with --nocapture / 输出日志以便查看
    if let Ok(out) = Command::new("docker").args(["logs", sms_name]).output() {
        println!("[SMS logs]\n{}", String::from_utf8_lossy(&out.stdout));
    }
    if let Ok(out) = Command::new("docker").args(["logs", spear_name]).output() {
        println!("[SPEARlet logs]\n{}", String::from_utf8_lossy(&out.stdout));
    }

    // If we reach here without panic, basic E2E succeeded / 到此处且未panic则E2E基本成功
}
