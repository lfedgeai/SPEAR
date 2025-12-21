#![cfg(feature = "wasmedge")]
use crate::spearlet::execution::host_api::{DefaultHostApi, SpearHostApi};
use crate::spearlet::execution::runtime::{ResourcePoolConfig, RuntimeConfig};
use crate::spearlet::execution::ExecutionError;
use crate::spearlet::execution::RuntimeType;
use std::time::{SystemTime, UNIX_EPOCH};
use wasmedge_sdk::{
    error::CoreError, AsInstance, CallingFrame, ImportObject, ImportObjectBuilder, Instance,
    WasmValue,
};

// Helper function to extract DefaultHostApi from host data
fn _unused() {}

const CCHAT_OK: i32 = 0;
const CCHAT_ERR_INVALID_FD: i32 = -1;
const CCHAT_ERR_INVALID_PTR: i32 = -2;
const CCHAT_ERR_BUFFER_TOO_SMALL: i32 = -3;
const CCHAT_ERR_INVALID_CMD: i32 = -4;
const CCHAT_ERR_INTERNAL: i32 = -5;

const CTL_SET_PARAM: i32 = 1;
const CTL_GET_METRICS: i32 = 2;

fn get_i32_arg(input: &[WasmValue], idx: usize) -> Option<i32> {
    input.get(idx).map(|v| v.to_i32())
}

fn mem_read(instance: &mut Instance, ptr: i32, len: i32) -> Result<Vec<u8>, i32> {
    if ptr < 0 || len < 0 {
        return Err(CCHAT_ERR_INVALID_PTR);
    }
    let mem = instance
        .get_memory_ref("memory")
        .map_err(|_| CCHAT_ERR_INVALID_PTR)?;
    mem.get_data(ptr as u32, len as u32)
        .map_err(|_| CCHAT_ERR_INVALID_PTR)
}

fn mem_write(instance: &mut Instance, ptr: i32, data: &[u8]) -> Result<(), i32> {
    if ptr < 0 {
        return Err(CCHAT_ERR_INVALID_PTR);
    }
    let mut mem = instance
        .get_memory_mut("memory")
        .map_err(|_| CCHAT_ERR_INVALID_PTR)?;
    mem.set_data(data, ptr as u32)
        .map_err(|_| CCHAT_ERR_INVALID_PTR)
}

fn mem_read_u32(instance: &mut Instance, ptr: i32) -> Result<u32, i32> {
    let bytes = mem_read(instance, ptr, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn mem_write_u32(instance: &mut Instance, ptr: i32, v: u32) -> Result<(), i32> {
    mem_write(instance, ptr, &v.to_le_bytes())
}

fn mem_write_with_len(
    instance: &mut Instance,
    out_ptr: i32,
    out_len_ptr: i32,
    payload: &[u8],
) -> i32 {
    let max_len = match mem_read_u32(instance, out_len_ptr) {
        Ok(v) => v as usize,
        Err(e) => return e,
    };
    let need = payload.len();
    if max_len < need {
        let _ = mem_write_u32(instance, out_len_ptr, need as u32);
        return CCHAT_ERR_BUFFER_TOO_SMALL;
    }
    if mem_write(instance, out_ptr, payload).is_err() {
        return CCHAT_ERR_INVALID_PTR;
    }
    if mem_write_u32(instance, out_len_ptr, need as u32).is_err() {
        return CCHAT_ERR_INVALID_PTR;
    }
    need as i32
}

pub fn cchat_create(
    host_data: &mut DefaultHostApi,
    _instance: &mut Instance,
    _frame: &mut CallingFrame,
    input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if !input.is_empty() {
        return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]);
    }
    Ok(vec![WasmValue::from_i32(host_data.cchat_create())])
}

pub fn cchat_write_msg(
    host_data: &mut DefaultHostApi,
    instance: &mut Instance,
    _frame: &mut CallingFrame,
    input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if input.len() != 5 {
        return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]);
    }

    let fd = get_i32_arg(&input, 0).unwrap_or(CCHAT_ERR_INVALID_FD);
    let role_ptr = get_i32_arg(&input, 1).unwrap_or(-1);
    let role_len = get_i32_arg(&input, 2).unwrap_or(-1);
    let content_ptr = get_i32_arg(&input, 3).unwrap_or(-1);
    let content_len = get_i32_arg(&input, 4).unwrap_or(-1);

    let role = match mem_read(instance, role_ptr, role_len) {
        Ok(b) => String::from_utf8_lossy(&b).to_string(),
        Err(e) => return Ok(vec![WasmValue::from_i32(e)]),
    };
    let content = match mem_read(instance, content_ptr, content_len) {
        Ok(b) => String::from_utf8_lossy(&b).to_string(),
        Err(e) => return Ok(vec![WasmValue::from_i32(e)]),
    };

    let rc = host_data.cchat_write_msg(fd, role, content);
    Ok(vec![WasmValue::from_i32(rc)])
}

pub fn cchat_write_fn(
    host_data: &mut DefaultHostApi,
    instance: &mut Instance,
    _frame: &mut CallingFrame,
    input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if input.len() != 4 {
        return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]);
    }

    let fd = get_i32_arg(&input, 0).unwrap_or(CCHAT_ERR_INVALID_FD);
    let fn_offset = get_i32_arg(&input, 1).unwrap_or(-1);
    let fn_ptr = get_i32_arg(&input, 2).unwrap_or(-1);
    let fn_len = get_i32_arg(&input, 3).unwrap_or(-1);

    let fn_json = match mem_read(instance, fn_ptr, fn_len) {
        Ok(b) => String::from_utf8_lossy(&b).to_string(),
        Err(e) => return Ok(vec![WasmValue::from_i32(e)]),
    };

    let rc = host_data.cchat_write_fn(fd, fn_offset, fn_json);
    Ok(vec![WasmValue::from_i32(rc)])
}

pub fn cchat_ctl(
    host_data: &mut DefaultHostApi,
    instance: &mut Instance,
    _frame: &mut CallingFrame,
    input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if input.len() != 4 {
        return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]);
    }

    let fd = get_i32_arg(&input, 0).unwrap_or(CCHAT_ERR_INVALID_FD);
    let cmd = get_i32_arg(&input, 1).unwrap_or(CCHAT_ERR_INVALID_CMD);
    let arg_ptr = get_i32_arg(&input, 2).unwrap_or(-1);
    let arg_len_ptr = get_i32_arg(&input, 3).unwrap_or(-1);

    match cmd {
        CTL_SET_PARAM => {
            let arg_len = match mem_read_u32(instance, arg_len_ptr) {
                Ok(v) => v as i32,
                Err(e) => return Ok(vec![WasmValue::from_i32(e)]),
            };
            let bytes = match mem_read(instance, arg_ptr, arg_len) {
                Ok(b) => b,
                Err(e) => return Ok(vec![WasmValue::from_i32(e)]),
            };
            let v: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(_) => return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]),
            };
            let key = v.get("key").and_then(|x| x.as_str()).unwrap_or("");
            let value = v.get("value").cloned().unwrap_or(serde_json::Value::Null);
            if key.is_empty() {
                return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]);
            }
            let rc = host_data.cchat_ctl_set_param(fd, key.to_string(), value);
            Ok(vec![WasmValue::from_i32(rc)])
        }
        CTL_GET_METRICS => {
            let payload = match host_data.cchat_ctl_get_metrics(fd) {
                Ok(b) => b,
                Err(e) => return Ok(vec![WasmValue::from_i32(e)]),
            };
            let wrote = mem_write_with_len(instance, arg_ptr, arg_len_ptr, &payload);
            Ok(vec![WasmValue::from_i32(wrote)])
        }
        _ => Ok(vec![WasmValue::from_i32(CCHAT_ERR_INVALID_CMD)]),
    }
}

pub fn cchat_send(
    host_data: &mut DefaultHostApi,
    _instance: &mut Instance,
    _frame: &mut CallingFrame,
    input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if input.len() != 2 {
        return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]);
    }

    let fd = get_i32_arg(&input, 0).unwrap_or(CCHAT_ERR_INVALID_FD);
    let flags = get_i32_arg(&input, 1).unwrap_or(0);
    match host_data.cchat_send(fd, flags) {
        Ok(resp_fd) => Ok(vec![WasmValue::from_i32(resp_fd)]),
        Err(e) => Ok(vec![WasmValue::from_i32(e)]),
    }
}

pub fn cchat_recv(
    host_data: &mut DefaultHostApi,
    instance: &mut Instance,
    _frame: &mut CallingFrame,
    input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if input.len() != 3 {
        return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]);
    }

    let response_fd = get_i32_arg(&input, 0).unwrap_or(CCHAT_ERR_INVALID_FD);
    let out_ptr = get_i32_arg(&input, 1).unwrap_or(-1);
    let out_len_ptr = get_i32_arg(&input, 2).unwrap_or(-1);

    let payload = match host_data.cchat_recv(response_fd) {
        Ok(b) => b,
        Err(e) => return Ok(vec![WasmValue::from_i32(e)]),
    };
    let wrote = mem_write_with_len(instance, out_ptr, out_len_ptr, &payload);
    Ok(vec![WasmValue::from_i32(wrote)])
}

pub fn cchat_close(
    host_data: &mut DefaultHostApi,
    _instance: &mut Instance,
    _frame: &mut CallingFrame,
    input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if input.len() != 1 {
        return Ok(vec![WasmValue::from_i32(CCHAT_ERR_INTERNAL)]);
    }
    let fd = get_i32_arg(&input, 0).unwrap_or(CCHAT_ERR_INVALID_FD);
    Ok(vec![WasmValue::from_i32(host_data.cchat_close(fd))])
}

pub fn spear_time_now_ms(
    host_data: &mut DefaultHostApi,
    _instance: &mut Instance,
    _frame: &mut CallingFrame,
    _input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    let v = host_data.time_now_ms() as i64;
    Ok(vec![WasmValue::from_i64(v)])
}

pub fn spear_wall_time_s(
    _host_data: &mut DefaultHostApi,
    _instance: &mut Instance,
    _frame: &mut CallingFrame,
    _input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    let v = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    Ok(vec![WasmValue::from_i64(v)])
}

pub fn spear_sleep_ms(
    _host_data: &mut DefaultHostApi,
    _instance: &mut Instance,
    _frame: &mut CallingFrame,
    input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    if input.len() != 1 {
        return Err(CoreError::Common(
            wasmedge_sdk::error::CoreCommonError::UserDefError,
        ));
    }
    let ms = input[0].to_i32() as u64;
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(vec![])
}

pub fn spear_random_i64(
    _host_data: &mut DefaultHostApi,
    _instance: &mut Instance,
    _frame: &mut CallingFrame,
    _input: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, CoreError> {
    let acc = rand::random::<i64>();
    Ok(vec![WasmValue::from_i64(acc)])
}

pub fn build_spear_import() -> Result<ImportObject<DefaultHostApi>, ExecutionError> {
    let api = DefaultHostApi::new(RuntimeConfig {
        runtime_type: RuntimeType::Wasm,
        settings: std::collections::HashMap::new(),
        global_environment: std::collections::HashMap::new(),
        spearlet_config: None,
        resource_pool: ResourcePoolConfig::default(),
    });
    let mut builder =
        ImportObjectBuilder::new("spear", api).map_err(|e| ExecutionError::RuntimeError {
            message: format!("create import builder error: {}", e),
        })?;

    builder
        .with_func::<(), i64>("time_now_ms", spear_time_now_ms)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add time_now_ms function error: {}", e),
        })?;
    builder
        .with_func::<(), i64>("wall_time_s", spear_wall_time_s)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add wall_time_s function error: {}", e),
        })?;
    builder
        .with_func::<(), i64>("random_i64", spear_random_i64)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add random_i64 function error: {}", e),
        })?;
    builder
        .with_func::<i32, ()>("sleep_ms", spear_sleep_ms)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add sleep_ms function error: {}", e),
        })?;

    builder
        .with_func::<(), i32>("cchat_create", cchat_create)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_create function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32, i32, i32, i32), i32>("cchat_write_msg", cchat_write_msg)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_write_msg function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32, i32, i32), i32>("cchat_write_fn", cchat_write_fn)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_write_fn function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32, i32, i32), i32>("cchat_ctl", cchat_ctl)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_ctl function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32), i32>("cchat_send", cchat_send)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_send function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32, i32), i32>("cchat_recv", cchat_recv)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_recv function error: {}", e),
        })?;
    builder
        .with_func::<i32, i32>("cchat_close", cchat_close)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_close function error: {}", e),
        })?;

    let import = builder.build();
    Ok(import)
}

pub fn build_spear_import_with_api(
    runtime_config: RuntimeConfig,
) -> Result<ImportObject<DefaultHostApi>, ExecutionError> {
    let api = DefaultHostApi::new(runtime_config);
    let mut builder =
        ImportObjectBuilder::new("spear", api).map_err(|e| ExecutionError::RuntimeError {
            message: format!("create import builder error: {}", e),
        })?;

    builder
        .with_func::<(), i64>("time_now_ms", spear_time_now_ms)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add time_now_ms function error: {}", e),
        })?;
    builder
        .with_func::<(), i64>("wall_time_s", spear_wall_time_s)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add wall_time_s function error: {}", e),
        })?;
    builder
        .with_func::<(), i64>("random_i64", spear_random_i64)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add random_i64 function error: {}", e),
        })?;
    builder
        .with_func::<i32, ()>("sleep_ms", spear_sleep_ms)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add sleep_ms function error: {}", e),
        })?;

    builder
        .with_func::<(), i32>("cchat_create", cchat_create)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_create function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32, i32, i32, i32), i32>("cchat_write_msg", cchat_write_msg)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_write_msg function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32, i32, i32), i32>("cchat_write_fn", cchat_write_fn)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_write_fn function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32, i32, i32), i32>("cchat_ctl", cchat_ctl)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_ctl function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32), i32>("cchat_send", cchat_send)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_send function error: {}", e),
        })?;
    builder
        .with_func::<(i32, i32, i32), i32>("cchat_recv", cchat_recv)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_recv function error: {}", e),
        })?;
    builder
        .with_func::<i32, i32>("cchat_close", cchat_close)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("add cchat_close function error: {}", e),
        })?;

    let import = builder.build();
    Ok(import)
}
