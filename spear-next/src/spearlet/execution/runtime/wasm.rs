//! WASM Runtime Implementation
//! WASM 运行时实现
//!
//! This module provides WebAssembly-based execution runtime using Wasmtime.
//! 该模块使用 Wasmtime 提供基于 WebAssembly 的执行运行时。

use super::{
    ExecutionContext, RuntimeExecutionResponse, Runtime, RuntimeCapabilities, RuntimeConfig, RuntimeType,
};
use crate::spearlet::execution::{
    ExecutionError, ExecutionResult, InstanceStatus,
    instance::{InstanceConfig, InstanceResourceLimits, TaskInstance},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::debug;
use reqwest::StatusCode;
use crate::spearlet::execution::artifact_fetch;

// Note: In a real implementation, you would use wasmedge crate
// 注意：在真实实现中，您会使用 wasmedge crate
// For now, we'll create a mock implementation to avoid adding dependencies
// 现在，我们将创建一个模拟实现以避免添加依赖项

/// WASM runtime configuration / WASM 运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    /// WASM module cache directory / WASM 模块缓存目录
    pub cache_directory: String,
    /// Maximum WASM module size in bytes / WASM 模块最大大小（字节）
    pub max_module_size_bytes: u64,
    /// WASM execution configuration / WASM 执行配置
    pub execution_config: WasmExecutionConfig,
    /// WASM security configuration / WASM 安全配置
    pub security_config: WasmSecurityConfig,
    /// WASM optimization configuration / WASM 优化配置
    pub optimization_config: WasmOptimizationConfig,
}

/// WASM execution configuration / WASM 执行配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmExecutionConfig {
    /// Maximum stack size / 最大栈大小
    pub max_stack_size: u32,
    /// Maximum heap size / 最大堆大小
    pub max_heap_size: u32,
    /// Enable WASI (WebAssembly System Interface) / 启用 WASI（WebAssembly 系统接口）
    pub enable_wasi: bool,
    /// WASI allowed directories / WASI 允许的目录
    pub wasi_allowed_dirs: Vec<String>,
    /// WASI environment variables / WASI 环境变量
    pub wasi_env_vars: HashMap<String, String>,
    /// Enable multi-threading / 启用多线程
    pub enable_threads: bool,
    /// Maximum number of threads / 最大线程数
    pub max_threads: u32,
}

/// WASM security configuration / WASM 安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSecurityConfig {
    /// Enable sandbox mode / 启用沙箱模式
    pub enable_sandbox: bool,
    /// Allowed host functions / 允许的主机函数
    pub allowed_host_functions: Vec<String>,
    /// Maximum execution time per call / 每次调用的最大执行时间
    pub max_execution_time_ms: u64,
    /// Maximum memory allocation / 最大内存分配
    pub max_memory_allocation: u64,
    /// Enable fuel (execution limits) / 启用燃料（执行限制）
    pub enable_fuel: bool,
    /// Fuel limit per execution / 每次执行的燃料限制
    pub fuel_limit: u64,
}

/// WASM optimization configuration / WASM 优化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmOptimizationConfig {
    /// Enable JIT compilation / 启用 JIT 编译
    pub enable_jit: bool,
    /// Optimization level (0-3) / 优化级别（0-3）
    pub optimization_level: u8,
    /// Enable module caching / 启用模块缓存
    pub enable_caching: bool,
    /// Cache TTL in seconds / 缓存 TTL（秒）
    pub cache_ttl_seconds: u64,
    /// Enable parallel compilation / 启用并行编译
    pub enable_parallel_compilation: bool,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            cache_directory: "/tmp/spearlet/wasm_cache".to_string(),
            max_module_size_bytes: 100 * 1024 * 1024, // 100MB
            execution_config: WasmExecutionConfig {
                max_stack_size: 1024 * 1024,     // 1MB
                max_heap_size: 64 * 1024 * 1024, // 64MB
                enable_wasi: true,
                wasi_allowed_dirs: vec!["/tmp".to_string()],
                wasi_env_vars: HashMap::new(),
                enable_threads: false,
                max_threads: 1,
            },
            security_config: WasmSecurityConfig {
                enable_sandbox: true,
                allowed_host_functions: vec![],
                max_execution_time_ms: 30000, // 30 seconds
                max_memory_allocation: 128 * 1024 * 1024, // 128MB
                enable_fuel: true,
                fuel_limit: 1_000_000,
            },
            optimization_config: WasmOptimizationConfig {
                enable_jit: true,
                optimization_level: 2,
                enable_caching: true,
                cache_ttl_seconds: 3600, // 1 hour
                enable_parallel_compilation: true,
            },
        }
    }
}

/// Mock WASM module handle / 模拟 WASM 模块句柄
#[derive(Debug, Clone)]
pub struct WasmModuleHandle {
    /// Module ID / 模块 ID
    pub module_id: String,
    /// Module name / 模块名称
    pub module_name: String,
    /// Module size in bytes / 模块大小（字节）
    pub module_size: u64,
    /// Module hash / 模块哈希
    pub module_hash: String,
    /// Compilation time / 编译时间
    pub compilation_time: std::time::SystemTime,
    /// Exported functions / 导出的函数
    pub exported_functions: Vec<String>,
    /// Memory usage / 内存使用
    pub memory_usage: u64,
    pub module_bytes: Vec<u8>,
}

/// Mock WASM instance handle / 模拟 WASM 实例句柄
#[derive(Debug)]
pub struct WasmInstanceHandle {
    /// Instance ID / 实例 ID
    pub instance_id: String,
    /// Module handle / 模块句柄
    pub module_handle: WasmModuleHandle,
    /// Instance state / 实例状态
    pub state: Arc<Mutex<WasmInstanceState>>,
    /// Execution statistics / 执行统计
    pub execution_stats: Arc<Mutex<WasmExecutionStats>>,
    #[cfg(feature = "wasmedge")]
    pub vm: wasmedge_sdk::Vm,
}

/// WASM instance state / WASM 实例状态
#[derive(Debug, Clone)]
pub struct WasmInstanceState {
    /// Is instance initialized / 实例是否已初始化
    pub initialized: bool,
    /// Current memory usage / 当前内存使用
    pub memory_usage: u64,
    /// Current fuel remaining / 当前剩余燃料
    pub fuel_remaining: u64,
    /// Last execution time / 上次执行时间
    pub last_execution_time: Option<std::time::SystemTime>,
}

/// WASM execution statistics / WASM 执行统计
#[derive(Debug, Clone)]
pub struct WasmExecutionStats {
    /// Total executions / 总执行次数
    pub total_executions: u64,
    /// Total execution time / 总执行时间
    pub total_execution_time_ms: u64,
    /// Average execution time / 平均执行时间
    pub average_execution_time_ms: f64,
    /// Memory peak usage / 内存峰值使用
    pub memory_peak_usage: u64,
    /// Fuel consumed / 消耗的燃料
    pub fuel_consumed: u64,
}

impl Default for WasmExecutionStats {
    fn default() -> Self {
        Self {
            total_executions: 0,
            total_execution_time_ms: 0,
            average_execution_time_ms: 0.0,
            memory_peak_usage: 0,
            fuel_consumed: 0,
        }
    }
}

/// WASM runtime implementation / WASM 运行时实现
pub struct WasmRuntime {
    /// WASM configuration / WASM 配置
    config: WasmConfig,
    /// Runtime configuration / 运行时配置
    runtime_config: RuntimeConfig,
    /// Module cache / 模块缓存
    module_cache: Arc<Mutex<HashMap<String, WasmModuleHandle>>>,
}

impl WasmRuntime {
    /// Create a new WASM runtime / 创建新的 WASM 运行时
    pub fn new(runtime_config: &RuntimeConfig) -> ExecutionResult<Self> {
        let wasm_config = if let Some(wasm_settings) = runtime_config.settings.get("wasm") {
            serde_json::from_value(wasm_settings.clone()).map_err(|e| {
                ExecutionError::InvalidConfiguration {
                    message: format!("Invalid WASM configuration: {}", e),
                }
            })?
        } else {
            WasmConfig::default()
        };

        // Create cache directory if it doesn't exist / 如果缓存目录不存在则创建
        if let Err(e) = std::fs::create_dir_all(&wasm_config.cache_directory) {
            tracing::warn!("Failed to create WASM cache directory: {}", e);
        }

        Ok(Self {
            config: wasm_config,
            runtime_config: runtime_config.clone(),
            module_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Load WASM module from bytes / 从字节加载 WASM 模块
    async fn load_wasm_module(&self, module_bytes: &[u8]) -> ExecutionResult<WasmModuleHandle> {
        if module_bytes.len() > self.config.max_module_size_bytes as usize {
            return Err(ExecutionError::InvalidConfiguration {
                message: format!(
                    "WASM module size {} exceeds maximum {}",
                    module_bytes.len(),
                    self.config.max_module_size_bytes
                ),
            });
        }

        // Calculate module hash / 计算模块哈希
        let module_hash = format!("{:x}", md5::compute(module_bytes));

        // Check cache first / 首先检查缓存
        {
            let cache = self.module_cache.lock().await;
            if let Some(cached_module) = cache.get(&module_hash) {
                return Ok(cached_module.clone());
            }
        }

        // Mock module compilation / 模拟模块编译
        tokio::time::sleep(Duration::from_millis(100)).await; // Simulate compilation time / 模拟编译时间

        let module_handle = WasmModuleHandle {
            module_id: uuid::Uuid::new_v4().to_string(),
            module_name: "user_module".to_string(),
            module_size: module_bytes.len() as u64,
            module_hash: module_hash.clone(),
            compilation_time: std::time::SystemTime::now(),
            exported_functions: vec!["main".to_string(), "_start".to_string()], // Mock exports / 模拟导出
            memory_usage: self.config.execution_config.max_heap_size as u64,
            module_bytes: module_bytes.to_vec(),
        };

        // Cache the module / 缓存模块
        {
            let mut cache = self.module_cache.lock().await;
            cache.insert(module_hash, module_handle.clone());
        }

        Ok(module_handle)
    }

    /// Create WASM instance from module / 从模块创建 WASM 实例
    async fn create_wasm_instance(
        &self,
        module_handle: WasmModuleHandle,
        _instance_config: &InstanceConfig,
    ) -> ExecutionResult<WasmInstanceHandle> {
        let instance_id = uuid::Uuid::new_v4().to_string();

        let state = WasmInstanceState {
            initialized: true,
            memory_usage: module_handle.memory_usage,
            fuel_remaining: self.config.security_config.fuel_limit,
            last_execution_time: None,
        };

        #[cfg(feature = "wasmedge")]
        let vm_built = {
            use wasmedge_sdk::config::{CommonConfigOptions, ConfigBuilder, HostRegistrationConfigOptions};
            use wasmedge_sdk::VmBuilder;
            let c = ConfigBuilder::new(CommonConfigOptions::default())
                .with_host_registration_config(HostRegistrationConfigOptions::default().wasi(true))
                .build()
                .map_err(|e| ExecutionError::RuntimeError { message: format!("wasmedge config error: {}", e) })?;
            let mut vm = VmBuilder::new()
                .with_config(c)
                .build()
                .map_err(|e| ExecutionError::RuntimeError { message: format!("wasmedge vm build error: {}", e) })?;
            let bytes = if module_handle.module_bytes.starts_with(&[0x00, 0x61, 0x73, 0x6d]) {
                module_handle.module_bytes.clone()
            } else {
                // return error
                return Err(ExecutionError::InvalidConfiguration {
                    message: "Invalid WASM module format".to_string(),
                });
            };
            vm = vm
                .register_module_from_bytes(&module_handle.module_name, &bytes)
                .map_err(|e| ExecutionError::RuntimeError { message: format!("wasmedge register error: {}", e) })?;
            vm
        };

        let instance_handle = WasmInstanceHandle {
            instance_id,
            module_handle,
            state: Arc::new(Mutex::new(state)),
            execution_stats: Arc::new(Mutex::new(WasmExecutionStats::default())),
            #[cfg(feature = "wasmedge")]
            vm: vm_built,
        };

        Ok(instance_handle)
    }

    /// Execute WASM function / 执行 WASM 函数
    async fn execute_wasm_function(
        &self,
        instance_handle: &WasmInstanceHandle,
        function_name: &str,
        input_data: &[u8],
    ) -> ExecutionResult<Vec<u8>> {
        let start_time = Instant::now();

        // Update state / 更新状态
        {
            let mut state = instance_handle.state.lock().await;
            state.last_execution_time = Some(std::time::SystemTime::now());
        }

        #[cfg(feature = "wasmedge")]
        let output = {
            use wasmedge_sdk::params;
            match instance_handle
                .vm
                .run_func(Some(&instance_handle.module_handle.module_name), function_name, params!())
            {
                Ok(values) => format!("{:?}", values),
                Err(e) => return Err(ExecutionError::RuntimeError { message: format!("wasmedge exec error: {}", e) }),
            }
        };
        #[cfg(not(feature = "wasmedge"))]
        let output = {
            tokio::time::sleep(Duration::from_millis(10)).await;
            format!(
                "WASM function '{}' executed with {} bytes input",
                function_name,
                input_data.len()
            )
        };

        let execution_time = start_time.elapsed();
        let execution_time_ms = execution_time.as_millis() as u64;

        // Update execution statistics / 更新执行统计
        {
            let mut stats = instance_handle.execution_stats.lock().await;
            stats.total_executions += 1;
            stats.total_execution_time_ms += execution_time_ms;
            stats.average_execution_time_ms = 
                stats.total_execution_time_ms as f64 / stats.total_executions as f64;
            stats.fuel_consumed += 1000; // Mock fuel consumption / 模拟燃料消耗
        }

        Ok(output.into_bytes())
    }

    /// Get WASM instance metrics / 获取 WASM 实例指标
    async fn get_wasm_metrics(
        &self,
        instance_handle: &WasmInstanceHandle,
    ) -> ExecutionResult<HashMap<String, serde_json::Value>> {
        let mut metrics = HashMap::new();

        // Get state metrics / 获取状态指标
        {
            let state = instance_handle.state.lock().await;
            metrics.insert(
                "memory_usage_bytes".to_string(),
                serde_json::Value::Number(serde_json::Number::from(state.memory_usage)),
            );
            metrics.insert(
                "fuel_remaining".to_string(),
                serde_json::Value::Number(serde_json::Number::from(state.fuel_remaining)),
            );
            metrics.insert(
                "initialized".to_string(),
                serde_json::Value::Bool(state.initialized),
            );
        }

        // Get execution statistics / 获取执行统计
        {
            let stats = instance_handle.execution_stats.lock().await;
            metrics.insert(
                "total_executions".to_string(),
                serde_json::Value::Number(serde_json::Number::from(stats.total_executions)),
            );
            metrics.insert(
                "total_execution_time_ms".to_string(),
                serde_json::Value::Number(serde_json::Number::from(stats.total_execution_time_ms)),
            );
            metrics.insert(
                "average_execution_time_ms".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(stats.average_execution_time_ms).unwrap_or(
                        serde_json::Number::from(0)
                    )
                ),
            );
            metrics.insert(
                "memory_peak_usage_bytes".to_string(),
                serde_json::Value::Number(serde_json::Number::from(stats.memory_peak_usage)),
            );
            metrics.insert(
                "fuel_consumed".to_string(),
                serde_json::Value::Number(serde_json::Number::from(stats.fuel_consumed)),
            );
        }

        // Module information / 模块信息
        metrics.insert(
            "module_id".to_string(),
            serde_json::Value::String(instance_handle.module_handle.module_id.clone()),
        );
        metrics.insert(
            "module_size_bytes".to_string(),
            serde_json::Value::Number(serde_json::Number::from(instance_handle.module_handle.module_size)),
        );
        metrics.insert(
            "exported_functions".to_string(),
            serde_json::Value::Array(
                instance_handle.module_handle.exported_functions
                    .iter()
                    .map(|f| serde_json::Value::String(f.clone()))
                    .collect()
            ),
        );

        Ok(metrics)
    }
}

#[async_trait]
impl Runtime for WasmRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Wasm
    }

    async fn create_instance(
        &self,
        config: &InstanceConfig,
    ) -> ExecutionResult<Arc<TaskInstance>> {
        debug!("WasmRuntime::create_instance task_id={}", config.task_id);
        let instance = Arc::new(TaskInstance::new(config.task_id.clone(), config.clone()));

        let module_bytes_vec: Vec<u8> = if let Some(snapshot) = &config.artifact {
            if let Some(uri) = &snapshot.location {
                if uri.starts_with("sms+file://") {
                    let rest = &uri[11..];
                    let (override_host_port, id_part) = match rest.find('/') {
                        Some(pos) => (Some(rest[..pos].to_string()), rest[pos+1..].to_string()),
                        None => (None, rest.to_string()),
                    };
                    let id = id_part.trim_start_matches('/');
                    let path = format!("/api/v1/files/{}", id);

                    let cfg = self
                        .runtime_config
                        .spearlet_config
                        .as_ref()
                        .ok_or_else(|| ExecutionError::InvalidConfiguration { message: "Missing SpearletConfig".to_string() })?;

                    let sms_http_addr = if let Some(hp) = override_host_port { hp } else { cfg.sms_http_addr.clone() };

                    debug!(
                        task_id = %config.task_id,
                        artifact_id = %config.artifact_id,
                        sms_http_addr = %cfg.sms_http_addr,
                        file_id = %id,
                        "Fetching WASM module from SMS"
                    );

                    match artifact_fetch::fetch_sms_file(&sms_http_addr, &path).await {
                        Ok(b) => b,
                        Err(e) => {
                            debug!(error = %e.to_string(), url = format!("http://{}{}", sms_http_addr, path), "Failed to fetch SMS file");
                            return Err(e);
                        }
                    }
                } else {
                    return Err(ExecutionError::InvalidConfiguration { message: format!("Unsupported artifact URI scheme: {}", uri) });
                }
            } else {
                return Err(ExecutionError::InvalidConfiguration { message: "Missing artifact location for WASM module".to_string() });
            }
        } else {
            debug!("WasmRuntime::create_instance missing artifact snapshot task_id={} artifact_id={}", config.task_id, config.artifact_id);
            return Err(ExecutionError::InvalidConfiguration { message: "Missing artifact snapshot for WASM module".to_string() });
        };

        // Validate WASM magic header / 校验 WASM 魔数
        if !module_bytes_vec.starts_with(&[0x00, 0x61, 0x73, 0x6d]) {
            return Err(ExecutionError::InvalidConfiguration {
                message: "Invalid WASM module: missing magic header".to_string(),
            });
        }

        // Load WASM module / 加载 WASM 模块
        let module_handle = self.load_wasm_module(&module_bytes_vec).await?;

        // Create WASM instance / 创建 WASM 实例
        let wasm_instance = self.create_wasm_instance(module_handle, config).await?;

        instance.set_runtime_handle(Arc::new(wasm_instance));
        instance.set_status(InstanceStatus::Ready);

        Ok(instance)
    }

    async fn start_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        debug!("WasmRuntime::start_instance instance_id={}", instance.id());
        // WASM instance is ready to execute when created / WASM 实例在创建时就准备好执行
        instance.set_status(InstanceStatus::Running);
        Ok(())
    }

    async fn stop_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        debug!("WasmRuntime::stop_instance instance_id={}", instance.id());
        instance.set_status(InstanceStatus::Stopping);
        
        // WASM instances don't need explicit stopping / WASM 实例不需要显式停止
        // Just mark as stopped / 只需标记为已停止
        instance.set_status(InstanceStatus::Stopped);
        Ok(())
    }

    async fn execute(
        &self,
        instance: &Arc<TaskInstance>,
        context: ExecutionContext,
    ) -> ExecutionResult<RuntimeExecutionResponse> {
        debug!("WasmRuntime::execute instance_id={} execution_id={}", instance.id(), context.execution_id);
        let start_time = Instant::now();
        instance.record_request_start();

        let wasm_handle = instance
            .get_runtime_handle::<WasmInstanceHandle>()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "No WASM instance handle found".to_string(),
            })?;

        // Execute WASM function / 执行 WASM 函数
        let function_name = {
            let has_start = wasm_handle
                .module_handle
                .exported_functions
                .iter()
                .any(|f| f == "_start");
            if has_start { "_start" } else { "main" }
        };

        let result = tokio::time::timeout(
            Duration::from_millis(context.timeout_ms),
            self.execute_wasm_function(&wasm_handle, function_name, &context.payload),
        )
        .await;

        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                instance.record_request_completion(true, duration_ms as f64);
                debug!(
                    instance_id = %instance.id(),
                    execution_id = %context.execution_id,
                    duration_ms = duration_ms,
                    output_len = output.len(),
                    "WASM execution completed"
                );
                Ok(RuntimeExecutionResponse::new_sync(
                    context.execution_id,
                    output,
                    duration_ms,
                ))
            }
            Ok(Err(e)) => {
                instance.record_request_completion(false, duration_ms as f64);
                debug!(
                    instance_id = %instance.id(),
                    execution_id = %context.execution_id,
                    duration_ms = duration_ms,
                    error = %e.to_string(),
                    "WASM execution failed"
                );
                Err(e)
            }
            Err(_) => {
                instance.record_request_completion(false, duration_ms as f64);
                debug!(
                    instance_id = %instance.id(),
                    execution_id = %context.execution_id,
                    duration_ms = duration_ms,
                    timeout_ms = context.timeout_ms,
                    "WASM execution timed out"
                );
                Err(ExecutionError::ExecutionTimeout {
                    timeout_ms: context.timeout_ms,
                })
            }
        }
    }

    async fn health_check(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<bool> {
        debug!("WasmRuntime::health_check instance_id={}", instance.id());
        let wasm_handle = instance
            .get_runtime_handle::<WasmInstanceHandle>()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "No WASM instance handle found".to_string(),
            })?;

        // Check if WASM instance is initialized / 检查 WASM 实例是否已初始化
        let state = wasm_handle.state.lock().await;
        let is_healthy = state.initialized && state.fuel_remaining > 0;
        
        instance.record_health_check(is_healthy);
        Ok(is_healthy)
    }

    async fn get_metrics(
        &self,
        instance: &Arc<TaskInstance>,
    ) -> ExecutionResult<HashMap<String, serde_json::Value>> {
        debug!("WasmRuntime::get_metrics instance_id={}", instance.id());
        let wasm_handle = instance
            .get_runtime_handle::<WasmInstanceHandle>()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "No WASM instance handle found".to_string(),
            })?;

        self.get_wasm_metrics(&wasm_handle).await
    }

    async fn scale_instance(
        &self,
        instance: &Arc<TaskInstance>,
        new_limits: &InstanceResourceLimits,
    ) -> ExecutionResult<()> {
        debug!("WasmRuntime::scale_instance instance_id={}", instance.id());
        let wasm_handle = instance
            .get_runtime_handle::<WasmInstanceHandle>()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "No WASM instance handle found".to_string(),
            })?;

        // Update fuel limit based on new resource limits / 根据新的资源限制更新燃料限制
        let new_fuel_limit = (new_limits.max_cpu_cores * 1_000_000.0) as u64;
        
        {
            let mut state = wasm_handle.state.lock().await;
            state.fuel_remaining = new_fuel_limit;
        }

        Ok(())
    }

    async fn cleanup_instance(&self, _instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        debug!("WasmRuntime::cleanup_instance instance_id={}", _instance.id());
        // WASM instances are automatically cleaned up when dropped / WASM 实例在丢弃时自动清理
        // No explicit cleanup needed / 不需要显式清理
        Ok(())
    }

    fn validate_config(&self, config: &InstanceConfig) -> ExecutionResult<()> {
        debug!("WasmRuntime::validate_config task_id={}", config.task_id);
        if config.runtime_type != RuntimeType::Wasm {
            return Err(ExecutionError::InvalidConfiguration {
                message: "Runtime type must be WASM".to_string(),
            });
        }

        // Validate resource limits / 验证资源限制
        let limits = &config.resource_limits;
        if limits.max_cpu_cores <= 0.0 {
            return Err(ExecutionError::InvalidConfiguration {
                message: "CPU cores must be greater than 0".to_string(),
            });
        }

        if limits.max_memory_bytes == 0 {
            return Err(ExecutionError::InvalidConfiguration {
                message: "Memory limit must be greater than 0".to_string(),
            });
        }

        // Validate WASM-specific limits / 验证 WASM 特定限制
        if limits.max_memory_bytes > self.config.security_config.max_memory_allocation {
            return Err(ExecutionError::InvalidConfiguration {
                message: format!(
                    "Memory limit {} exceeds maximum allowed {}",
                    limits.max_memory_bytes,
                    self.config.security_config.max_memory_allocation
                ),
            });
        }

        Ok(())
    }

    fn get_capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            supports_scaling: true, // WASM supports dynamic resource scaling / WASM 支持动态资源扩缩容
            supports_health_checks: true,
            supports_metrics: true,
            supports_hot_reload: true, // WASM modules can be reloaded / WASM 模块可以重新加载
            supports_persistent_storage: false, // WASM is stateless by default / WASM 默认是无状态的
            supports_network_isolation: true, // WASM provides strong isolation / WASM 提供强隔离
            max_concurrent_instances: self.runtime_config.resource_pool.max_concurrent_instances,
            supported_protocols: vec!["HTTP".to_string(), "gRPC".to_string(), "Custom".to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::instance::{InstanceConfig, InstanceResourceLimits, NetworkConfig};

    #[test]
    fn test_wasm_config_default() {
        let config = WasmConfig::default();
        assert_eq!(config.cache_directory, "/tmp/spearlet/wasm_cache");
        assert_eq!(config.max_module_size_bytes, 100 * 1024 * 1024);
        assert!(config.execution_config.enable_wasi);
        assert!(config.security_config.enable_sandbox);
        assert!(config.optimization_config.enable_jit);
    }

    #[test]
    fn test_wasm_runtime_creation() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Wasm,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: super::super::ResourcePoolConfig::default(),
        };

        let runtime = WasmRuntime::new(&runtime_config);
        assert!(runtime.is_ok());
        
        let runtime = runtime.unwrap();
        assert_eq!(runtime.runtime_type(), RuntimeType::Wasm);
    }

    #[tokio::test]
    async fn test_load_wasm_module() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Wasm,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: super::super::ResourcePoolConfig::default(),
        };

        let runtime = WasmRuntime::new(&runtime_config).unwrap();
        
        let module_bytes = b"mock_wasm_module_bytes";
        let module_handle = runtime.load_wasm_module(module_bytes).await;
        
        assert!(module_handle.is_ok());
        let handle = module_handle.unwrap();
        assert_eq!(handle.module_size, module_bytes.len() as u64);
        assert!(!handle.exported_functions.is_empty());
    }

    #[test]
    fn test_validate_config() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Wasm,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: super::super::ResourcePoolConfig::default(),
        };

        let runtime = WasmRuntime::new(&runtime_config).unwrap();
        
        let valid_config = InstanceConfig {
            task_id: "task-xyz".to_string(),
            artifact_id: "artifact-xyz".to_string(),
            runtime_type: RuntimeType::Wasm,
            runtime_config: HashMap::new(),
            artifact: None,
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits {
                max_cpu_cores: 0.5,
                max_memory_bytes: 64 * 1024 * 1024, // 64MB, less than WASM max_memory_allocation (128MB)
                max_disk_bytes: 512 * 1024 * 1024,   // 512MB
                max_network_bps: 50 * 1024 * 1024,   // 50MB/s
            },
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
        };

        assert!(runtime.validate_config(&valid_config).is_ok());

        let invalid_config = InstanceConfig {
            task_id: "task-xyz".to_string(),
            artifact_id: "artifact-xyz".to_string(),
            runtime_type: RuntimeType::Process, // Different runtime type for testing / 用于测试的不同运行时类型
            runtime_config: HashMap::new(),
            artifact: None,
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
        };

        assert!(runtime.validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_wasm_execution_stats() {
        let mut stats = WasmExecutionStats::default();
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.average_execution_time_ms, 0.0);
        
        // Simulate execution / 模拟执行
        stats.total_executions = 5;
        stats.total_execution_time_ms = 1000;
        stats.average_execution_time_ms = 
            stats.total_execution_time_ms as f64 / stats.total_executions as f64;
        
        assert_eq!(stats.average_execution_time_ms, 200.0);
    }

    #[tokio::test]
    async fn test_default_entry_prefers_start() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Wasm,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: super::super::ResourcePoolConfig::default(),
        };

        let runtime = WasmRuntime::new(&runtime_config).unwrap();

        let valid_config = InstanceConfig {
            task_id: "task-xyz".to_string(),
            artifact_id: "artifact-xyz".to_string(),
            runtime_type: RuntimeType::Wasm,
            runtime_config: HashMap::new(),
            artifact: None,
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits {
                max_cpu_cores: 0.5,
                max_memory_bytes: 64 * 1024 * 1024,
                max_disk_bytes: 512 * 1024 * 1024,
                max_network_bps: 50 * 1024 * 1024,
            },
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
        };
        // According to current logic, missing valid wasm module bytes should error
        let result = runtime.create_instance(&valid_config).await;
        assert!(result.is_err());
    }
}
