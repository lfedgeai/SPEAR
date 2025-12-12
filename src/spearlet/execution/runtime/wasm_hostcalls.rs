#![cfg(feature = "wasmedge")]
use crate::spearlet::execution::host_api::{DefaultHostApi, SpearHostApi};
use crate::spearlet::execution::runtime::{ResourcePoolConfig, RuntimeConfig};
use crate::spearlet::execution::ExecutionError;
use crate::spearlet::execution::RuntimeType;
use std::time::{SystemTime, UNIX_EPOCH};
use wasmedge_sdk::{
    error::CoreError, CallingFrame, ImportObject, ImportObjectBuilder, Instance, WasmValue,
};

// Helper function to extract DefaultHostApi from host data
fn _unused() {}

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

    let import = builder.build();
    Ok(import)
}
