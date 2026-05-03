//! WASM user stream echo integration test.
//! WASM 用户流回显集成测试。

#[cfg(feature = "wasmedge")]
#[tokio::test]
async fn test_wasm_user_stream_echo_roundtrip() {
    use spear_next::spearlet::config::SpearletConfig;
    use spear_next::spearlet::execution::host_api::{
        map_ws_close_to_channels, set_current_wasm_execution_id, ws_pop_any_outbound, ws_push_frame,
    };
    use spear_next::spearlet::execution::runtime::wasm_hostcalls::build_spear_import_with_api;
    use spear_next::spearlet::execution::runtime::{
        ResourcePoolConfig, RuntimeConfig, RuntimeType,
    };
    use spear_next::spearlet::mcp::task_subset::McpTaskPolicy;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;
    use wasmedge_sdk::config::{CommonConfigOptions, ConfigBuilder};
    use wasmedge_sdk::vm::SyncInst;
    use wasmedge_sdk::wasi::WasiModule;
    use wasmedge_sdk::{params, wat2wasm, Module, Store, Vm};

    fn build_ssf_v1_frame(stream_id: u32, msg_type: u16, meta: &[u8], data: &[u8]) -> Vec<u8> {
        let header_len: u16 = 32;
        let mut out = Vec::with_capacity(header_len as usize + meta.len() + data.len());
        out.extend_from_slice(b"SPST");
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&header_len.to_le_bytes());
        out.extend_from_slice(&msg_type.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&stream_id.to_le_bytes());
        out.extend_from_slice(&1u64.to_le_bytes());
        out.extend_from_slice(&(meta.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(meta);
        out.extend_from_slice(data);
        out
    }

    let wat = r#"(module
      (type $t_ep_create (func (result i32)))
      (type $t_ep_ctl (func (param i32 i32 i32 i32) (result i32)))
      (type $t_ep_wait (func (param i32 i32 i32 i32) (result i32)))
      (type $t_ep_close (func (param i32) (result i32)))
      (type $t_us_open (func (param i32 i32) (result i32)))
      (type $t_us_read (func (param i32 i32 i32) (result i32)))
      (type $t_us_write (func (param i32 i32 i32) (result i32)))
      (type $t_us_close (func (param i32) (result i32)))

      (import "spear" "spear_epoll_create" (func $ep_create (type $t_ep_create)))
      (import "spear" "spear_epoll_ctl" (func $ep_ctl (type $t_ep_ctl)))
      (import "spear" "spear_epoll_wait" (func $ep_wait (type $t_ep_wait)))
      (import "spear" "spear_epoll_close" (func $ep_close (type $t_ep_close)))

      (import "spear" "user_stream_open" (func $us_open (type $t_us_open)))
      (import "spear" "user_stream_read" (func $us_read (type $t_us_read)))
      (import "spear" "user_stream_write" (func $us_write (type $t_us_write)))
      (import "spear" "user_stream_close" (func $us_close (type $t_us_close)))

      (memory (export "memory") 4)

      (func (export "main") (result i32)
        (local $epfd i32)
        (local $fd i32)
        (local $n i32)
        (local $i i32)
        (local $p i32)
        (local $ev i32)
        (local $len i32)

        (local.set $epfd (call $ep_create))
        (local.set $fd (call $us_open (i32.const 1) (i32.const 3)))

        (call $ep_ctl
          (local.get $epfd)
          (i32.const 1)
          (local.get $fd)
          (i32.or
            (i32.or (i32.or (i32.const 1) (i32.const 4)) (i32.const 8))
            (i32.const 16)
          )
        )
        drop

        (block $exit
          (loop $outer
            (i32.store (i32.const 0x0ff0) (i32.const 512))
            (local.set $n (call $ep_wait (local.get $epfd) (i32.const 0x1000) (i32.const 0x0ff0) (i32.const 200)))
            (br_if $outer (i32.le_s (local.get $n) (i32.const 0)))

            (local.set $i (i32.const 0))
            (loop $inner
              (br_if $outer (i32.ge_s (local.get $i) (local.get $n)))

              (local.set $p (i32.add (i32.const 0x1000) (i32.mul (local.get $i) (i32.const 8))))
              (local.set $ev (i32.load (i32.add (local.get $p) (i32.const 4))))

              (br_if $exit
                (i32.ne
                  (i32.and (local.get $ev) (i32.const 24))
                  (i32.const 0)
                )
              )

              (if
                (i32.ne (i32.and (local.get $ev) (i32.const 1)) (i32.const 0))
                (then
                  (i32.store (i32.const 0x1ff0) (i32.const 65536))
                  (local.set $len (call $us_read (local.get $fd) (i32.const 0x2000) (i32.const 0x1ff0)))
                  (if
                    (i32.gt_s (local.get $len) (i32.const 0))
                    (then
                      (call $us_write (local.get $fd) (i32.const 0x2000) (local.get $len))
                      drop
                    )
                  )
                )
              )

              (local.set $i (i32.add (local.get $i) (i32.const 1)))
              (br $inner)
            )
          )
        )

        (call $us_close (local.get $fd))
        drop
        (call $ep_close (local.get $epfd))
        drop
        (i32.const 0)
      )
    )"#;

    let wasm = wat2wasm(wat.as_bytes()).unwrap();
    let exec_id = "exec-user-stream-wasm-echo".to_string();
    let inbound = build_ssf_v1_frame(1, 2, b"{}", b"hello");

    let c = ConfigBuilder::new(CommonConfigOptions::default().threads(true))
        .build()
        .unwrap();

    let mut cfg = SpearletConfig::default();
    cfg.sms_http_addr = "127.0.0.1:8080".to_string();

    let runtime_config = RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: HashMap::new(),
        global_environment: HashMap::new(),
        spearlet_config: Some(cfg),
        resource_pool: ResourcePoolConfig::default(),
    };

    let (done_tx, done_rx) = std::sync::mpsc::channel::<Result<i32, String>>();
    let exec_id2 = exec_id.clone();
    std::thread::spawn(move || {
        set_current_wasm_execution_id(Some(exec_id2.clone()));

        let mut wasi_module = WasiModule::create(None, None, None).unwrap();
        let mut instances: std::collections::HashMap<String, &mut dyn SyncInst> =
            std::collections::HashMap::new();
        instances.insert(wasi_module.name().to_string(), wasi_module.as_mut());

        let import = build_spear_import_with_api(
            runtime_config,
            "t-user-stream-echo".to_string(),
            Arc::new(McpTaskPolicy::default()),
            "inst-user-stream-echo".to_string(),
        )
        .unwrap();
        let mut spear_import = import;
        instances.insert("spear".to_string(), &mut spear_import);

        let store = Store::new(Some(&c), instances).unwrap();
        let mut vm = Vm::new(store);
        let module = Module::from_bytes(None, &wasm).unwrap();
        vm.register_module(Some("m"), module).unwrap();

        let rc = vm
            .run_func(Some("m"), "main", params!())
            .map(|ret| ret.first().map(|v| v.to_i32()).unwrap_or(0))
            .map_err(|e| e.to_string());

        set_current_wasm_execution_id(None);
        let _ = done_tx.send(rc);
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    let rc = ws_push_frame(&exec_id, inbound.clone());
    assert_eq!(rc, 0);

    let echoed = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(frame) = ws_pop_any_outbound(&exec_id) {
                return frame;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(echoed, inbound);

    map_ws_close_to_channels(&exec_id);

    let done = tokio::task::spawn_blocking(move || done_rx.recv_timeout(Duration::from_secs(2)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.unwrap_or_default(), 0);
}
