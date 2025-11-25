//! Kubernetes Runtime Implementation
//! Kubernetes 运行时实现
//!
//! This module provides Kubernetes-based execution runtime using Jobs and Pods.
//! 该模块提供基于 Kubernetes Jobs 和 Pods 的执行运行时。

use super::{
    ExecutionContext, RuntimeExecutionResponse, Runtime, RuntimeCapabilities, RuntimeConfig, RuntimeType,
};
use super::ResourcePoolConfig;
use crate::spearlet::execution::{
    ExecutionError, ExecutionResult,
    instance::{InstanceConfig, InstanceResourceLimits, TaskInstance},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;

/// Kubernetes runtime configuration / Kubernetes 运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesConfig {
    /// Kubernetes API server endpoint / Kubernetes API 服务器端点
    pub api_server: String,
    /// Namespace for job execution / 作业执行的命名空间
    pub namespace: String,
    /// Service account for job execution / 作业执行的服务账户
    pub service_account: Option<String>,
    /// Default container image / 默认容器镜像
    pub default_image: String,
    /// Image pull policy / 镜像拉取策略
    pub image_pull_policy: String,
    /// Image pull secrets / 镜像拉取密钥
    pub image_pull_secrets: Vec<String>,
    /// Resource configuration / 资源配置
    pub resource_config: KubernetesResourceConfig,
    /// Network configuration / 网络配置
    pub network_config: KubernetesNetworkConfig,
    /// Security configuration / 安全配置
    pub security_config: KubernetesSecurityConfig,
    /// Job configuration / 作业配置
    pub job_config: KubernetesJobConfig,
}

/// Kubernetes resource configuration / Kubernetes 资源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesResourceConfig {
    /// CPU request / CPU 请求
    pub cpu_request: String,
    /// CPU limit / CPU 限制
    pub cpu_limit: String,
    /// Memory request / 内存请求
    pub memory_request: String,
    /// Memory limit / 内存限制
    pub memory_limit: String,
    /// Ephemeral storage request / 临时存储请求
    pub ephemeral_storage_request: Option<String>,
    /// Ephemeral storage limit / 临时存储限制
    pub ephemeral_storage_limit: Option<String>,
}

/// Kubernetes network configuration / Kubernetes 网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesNetworkConfig {
    /// DNS policy / DNS 策略
    pub dns_policy: String,
    /// DNS config / DNS 配置
    pub dns_config: Option<KubernetesDnsConfig>,
    /// Host network / 主机网络
    pub host_network: bool,
    /// Host PID / 主机 PID
    pub host_pid: bool,
    /// Host IPC / 主机 IPC
    pub host_ipc: bool,
}

/// Kubernetes DNS configuration / Kubernetes DNS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesDnsConfig {
    /// Nameservers / 名称服务器
    pub nameservers: Vec<String>,
    /// Searches / 搜索域
    pub searches: Vec<String>,
    /// Options / 选项
    pub options: Vec<KubernetesDnsOption>,
}

/// Kubernetes DNS option / Kubernetes DNS 选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesDnsOption {
    /// Option name / 选项名称
    pub name: String,
    /// Option value / 选项值
    pub value: Option<String>,
}

/// Kubernetes security configuration / Kubernetes 安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesSecurityConfig {
    /// Run as non-root / 以非 root 用户运行
    pub run_as_non_root: bool,
    /// Run as user / 运行用户
    pub run_as_user: Option<i64>,
    /// Run as group / 运行组
    pub run_as_group: Option<i64>,
    /// FS group / 文件系统组
    pub fs_group: Option<i64>,
    /// Security context / 安全上下文
    pub security_context: HashMap<String, String>,
    /// Capabilities / 能力
    pub capabilities: KubernetesCapabilities,
}

/// Kubernetes capabilities / Kubernetes 能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesCapabilities {
    /// Capabilities to add / 要添加的能力
    pub add: Vec<String>,
    /// Capabilities to drop / 要删除的能力
    pub drop: Vec<String>,
}

/// Kubernetes job configuration / Kubernetes 作业配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesJobConfig {
    /// Job completion mode / 作业完成模式
    pub completion_mode: String,
    /// Parallelism / 并行度
    pub parallelism: Option<i32>,
    /// Completions / 完成数
    pub completions: Option<i32>,
    /// Active deadline seconds / 活动截止时间（秒）
    pub active_deadline_seconds: Option<i64>,
    /// Backoff limit / 回退限制
    pub backoff_limit: Option<i32>,
    /// TTL seconds after finished / 完成后的 TTL 秒数
    pub ttl_seconds_after_finished: Option<i32>,
    /// Restart policy / 重启策略
    pub restart_policy: String,
}

impl Default for KubernetesConfig {
    fn default() -> Self {
        Self {
            api_server: "https://kubernetes.default.svc".to_string(),
            namespace: "default".to_string(),
            service_account: None,
            default_image: "alpine:latest".to_string(),
            image_pull_policy: "IfNotPresent".to_string(),
            image_pull_secrets: vec![],
            resource_config: KubernetesResourceConfig::default(),
            network_config: KubernetesNetworkConfig::default(),
            security_config: KubernetesSecurityConfig::default(),
            job_config: KubernetesJobConfig::default(),
        }
    }
}

impl Default for KubernetesResourceConfig {
    fn default() -> Self {
        Self {
            cpu_request: "100m".to_string(),
            cpu_limit: "500m".to_string(),
            memory_request: "128Mi".to_string(),
            memory_limit: "512Mi".to_string(),
            ephemeral_storage_request: Some("1Gi".to_string()),
            ephemeral_storage_limit: Some("2Gi".to_string()),
        }
    }
}

impl Default for KubernetesNetworkConfig {
    fn default() -> Self {
        Self {
            dns_policy: "ClusterFirst".to_string(),
            dns_config: None,
            host_network: false,
            host_pid: false,
            host_ipc: false,
        }
    }
}

impl Default for KubernetesSecurityConfig {
    fn default() -> Self {
        Self {
            run_as_non_root: true,
            run_as_user: Some(1000),
            run_as_group: Some(1000),
            fs_group: Some(1000),
            security_context: HashMap::new(),
            capabilities: KubernetesCapabilities::default(),
        }
    }
}

impl Default for KubernetesCapabilities {
    fn default() -> Self {
        Self {
            add: vec![],
            drop: vec!["ALL".to_string()],
        }
    }
}

impl Default for KubernetesJobConfig {
    fn default() -> Self {
        Self {
            completion_mode: "NonIndexed".to_string(),
            parallelism: Some(1),
            completions: Some(1),
            active_deadline_seconds: Some(3600), // 1 hour
            backoff_limit: Some(3),
            ttl_seconds_after_finished: Some(300), // 5 minutes
            restart_policy: "Never".to_string(),
        }
    }
}

/// Kubernetes job handle / Kubernetes 作业句柄
#[derive(Debug, Clone)]
pub struct KubernetesJobHandle {
    /// Job name / 作业名称
    pub job_name: String,
    /// Job namespace / 作业命名空间
    pub namespace: String,
    /// Job UID / 作业 UID
    pub job_uid: String,
    /// Pod name / Pod 名称
    pub pod_name: Option<String>,
    /// Job status / 作业状态
    pub status: String,
    /// Created at / 创建时间
    pub created_at: std::time::SystemTime,
}

/// Kubernetes runtime implementation / Kubernetes 运行时实现
pub struct KubernetesRuntime {
    /// Kubernetes configuration / Kubernetes 配置
    config: KubernetesConfig,
    /// Runtime configuration / 运行时配置
    runtime_config: RuntimeConfig,
}

impl KubernetesRuntime {
    /// Create a new Kubernetes runtime / 创建新的 Kubernetes 运行时
    pub fn new(runtime_config: &RuntimeConfig) -> ExecutionResult<Self> {
        let config = if let Some(settings) = runtime_config.settings.get("kubernetes") {
            serde_json::from_value(settings.clone()).map_err(|e| ExecutionError::InvalidConfiguration {
                message: format!("Invalid Kubernetes configuration: {}", e),
            })?
        } else {
            KubernetesConfig::default()
        };

        Ok(Self {
            config,
            runtime_config: runtime_config.clone(),
        })
    }

    /// Build kubectl command arguments / 构建 kubectl 命令参数
    fn build_kubectl_args(&self, subcommand: &str, args: Vec<String>) -> Vec<String> {
        let mut kubectl_args = vec![
            subcommand.to_string(),
            "--namespace".to_string(),
            self.config.namespace.clone(),
        ];

        // Add API server if not default
        if self.config.api_server != "https://kubernetes.default.svc" {
            kubectl_args.extend_from_slice(&[
                "--server".to_string(),
                self.config.api_server.clone(),
            ]);
        }

        kubectl_args.extend(args);
        kubectl_args
    }

    /// Execute kubectl command / 执行 kubectl 命令
    async fn execute_kubectl_command(&self, args: Vec<String>) -> ExecutionResult<String> {
        let _start_time = Instant::now();
        let timeout_duration = Duration::from_millis(30000); // 30 seconds timeout

        let output = timeout(timeout_duration, async {
            Command::new("kubectl")
                .args(&args)
                .output()
                .await
        })
        .await
        .map_err(|_| ExecutionError::ExecutionTimeout {
            timeout_ms: 30000,
        })?
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("Failed to execute kubectl: {}", e),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ExecutionError::RuntimeError {
                message: format!("kubectl command failed: {}", stderr),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    /// Generate job manifest YAML / 生成作业清单 YAML
    fn generate_job_manifest(&self, instance_config: &InstanceConfig, job_name: &str, execution_context: &ExecutionContext) -> String {
        let image = instance_config.runtime_config.get("image")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.config.default_image);

        let mut env_vars = Vec::new();
        for (key, value) in &self.runtime_config.global_environment {
            env_vars.push(format!("        - name: {}\n          value: \"{}\"", key, value));
        }
        for (key, value) in &execution_context.headers {
            env_vars.push(format!("        - name: HEADER_{}\n          value: \"{}\"", key.to_uppercase(), value));
        }

        let env_section = if env_vars.is_empty() {
            String::new()
        } else {
            format!("      env:\n{}", env_vars.join("\n"))
        };

        format!(r#"apiVersion: batch/v1
kind: Job
metadata:
  name: {}
  namespace: {}
  labels:
    app: spear-execution
    execution-id: {}
spec:
  completionMode: {}
  parallelism: {}
  completions: {}
  activeDeadlineSeconds: {}
  backoffLimit: {}
  ttlSecondsAfterFinished: {}
  template:
    metadata:
      labels:
        app: spear-execution
        execution-id: {}
    spec:
      restartPolicy: {}
      serviceAccountName: {}
      securityContext:
        runAsNonRoot: {}
        runAsUser: {}
        runAsGroup: {}
        fsGroup: {}
      dnsPolicy: {}
      hostNetwork: {}
      hostPID: {}
      hostIPC: {}
      containers:
      - name: executor
        image: {}
        imagePullPolicy: {}
        command: ["/bin/sh", "-c"]
        args: ["echo 'Execution started'; sleep 10; echo 'Execution completed'"]
{}
        resources:
          requests:
            cpu: {}
            memory: {}
            ephemeral-storage: {}
          limits:
            cpu: {}
            memory: {}
            ephemeral-storage: {}
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
"#,
            job_name,
            self.config.namespace,
            execution_context.execution_id,
            self.config.job_config.completion_mode,
            self.config.job_config.parallelism.unwrap_or(1),
            self.config.job_config.completions.unwrap_or(1),
            self.config.job_config.active_deadline_seconds.unwrap_or(3600),
            self.config.job_config.backoff_limit.unwrap_or(3),
            self.config.job_config.ttl_seconds_after_finished.unwrap_or(300),
            execution_context.execution_id,
            self.config.job_config.restart_policy,
            self.config.service_account.as_ref().unwrap_or(&"default".to_string()),
            self.config.security_config.run_as_non_root,
            self.config.security_config.run_as_user.unwrap_or(1000),
            self.config.security_config.run_as_group.unwrap_or(1000),
            self.config.security_config.fs_group.unwrap_or(1000),
            self.config.network_config.dns_policy,
            self.config.network_config.host_network,
            self.config.network_config.host_pid,
            self.config.network_config.host_ipc,
            image,
            self.config.image_pull_policy,
            env_section,
            self.config.resource_config.cpu_request,
            self.config.resource_config.memory_request,
            self.config.resource_config.ephemeral_storage_request.as_ref().unwrap_or(&"1Gi".to_string()),
            self.config.resource_config.cpu_limit,
            self.config.resource_config.memory_limit,
            self.config.resource_config.ephemeral_storage_limit.as_ref().unwrap_or(&"2Gi".to_string()),
        )
    }

    /// Get job status / 获取作业状态
    async fn get_job_status(&self, job_name: &str) -> ExecutionResult<String> {
        let args = self.build_kubectl_args("get", vec![
            "job".to_string(),
            job_name.to_string(),
            "-o".to_string(),
            "jsonpath={.status.conditions[?(@.type==\"Complete\")].status}".to_string(),
        ]);

        let output = self.execute_kubectl_command(args).await?;
        Ok(output.trim().to_string())
    }

    /// Get job logs / 获取作业日志
    async fn get_job_logs(&self, job_name: &str) -> ExecutionResult<String> {
        let args = self.build_kubectl_args("logs", vec![
            format!("job/{}", job_name),
            "--tail".to_string(),
            "1000".to_string(),
        ]);

        let output = self.execute_kubectl_command(args).await?;
        Ok(output)
    }

    /// Delete job / 删除作业
    async fn delete_job(&self, job_name: &str) -> ExecutionResult<()> {
        let args = self.build_kubectl_args("delete", vec![
            "job".to_string(),
            job_name.to_string(),
            "--ignore-not-found".to_string(),
        ]);

        self.execute_kubectl_command(args).await?;
        Ok(())
    }
}

#[async_trait]
impl Runtime for KubernetesRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Kubernetes
    }

    async fn create_instance(
        &self,
        config: &InstanceConfig,
    ) -> ExecutionResult<Arc<TaskInstance>> {
        // For Kubernetes, we don't pre-create instances, they are created on-demand
        // 对于 Kubernetes，我们不预先创建实例，而是按需创建
        let instance = TaskInstance::new(
            "default-task".to_string(),
            config.clone(),
        );
        Ok(Arc::new(instance))
    }

    async fn start_instance(&self, _instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        // Kubernetes jobs are started when executed, not pre-started
        // Kubernetes 作业在执行时启动，而不是预先启动
        Ok(())
    }

    async fn stop_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        // Stop any running jobs for this instance
        // 停止此实例的任何正在运行的作业
        if let Some(handle) = instance.get_runtime_handle::<KubernetesJobHandle>() {
            self.delete_job(&handle.job_name).await?;
        }
        Ok(())
    }

    async fn execute(
        &self,
        instance: &Arc<TaskInstance>,
        context: ExecutionContext,
    ) -> ExecutionResult<RuntimeExecutionResponse> {
        let start_time = Instant::now();
        let job_name = format!("spear-job-{}", context.execution_id);

        // Generate and apply job manifest
        // 生成并应用作业清单
        let manifest = self.generate_job_manifest(&instance.config, &job_name, &context);
        
        // Write manifest to temporary file and apply it
        // 将清单写入临时文件并应用
        let temp_file = format!("/tmp/{}.yaml", job_name);
        tokio::fs::write(&temp_file, manifest).await.map_err(|e| ExecutionError::RuntimeError {
            message: format!("Failed to write job manifest: {}", e),
        })?;

        let apply_args = self.build_kubectl_args("apply", vec![
            "-f".to_string(),
            temp_file.clone(),
        ]);

        self.execute_kubectl_command(apply_args).await?;

        // Clean up temporary file
        // 清理临时文件
        let _ = tokio::fs::remove_file(&temp_file).await;

        // Wait for job completion
        // 等待作业完成
        let timeout_duration = Duration::from_millis(context.timeout_ms);
        let completion_result = timeout(timeout_duration, async {
            loop {
                let status = self.get_job_status(&job_name).await?;
                if status == "True" {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Ok::<(), ExecutionError>(())
        }).await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match completion_result {
            Ok(_) => {
                // Job completed successfully, get logs
                // 作业成功完成，获取日志
                let logs = self.get_job_logs(&job_name).await.unwrap_or_default();
                
                // Clean up job
                // 清理作业
                let _ = self.delete_job(&job_name).await;

                Ok(RuntimeExecutionResponse::new_sync(
                    context.execution_id,
                    logs.into_bytes(),
                    duration_ms,
                ))
            }
            Err(_) => {
                // Job timed out
                // 作业超时
                let _ = self.delete_job(&job_name).await;
                
                Ok(RuntimeExecutionResponse::new_failed(
                    context.execution_id,
                    super::ExecutionMode::Sync,
                    super::RuntimeExecutionError::ExecutionTimeout { 
                        timeout_ms: context.timeout_ms
                    },
                    duration_ms,
                ))
            }
        }
    }

    async fn health_check(&self, _instance: &Arc<TaskInstance>) -> ExecutionResult<bool> {
        // Check if kubectl is available and cluster is accessible
        // 检查 kubectl 是否可用且集群是否可访问
        let args = vec!["cluster-info".to_string()];
        match self.execute_kubectl_command(args).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn get_metrics(
        &self,
        _instance: &Arc<TaskInstance>,
    ) -> ExecutionResult<HashMap<String, serde_json::Value>> {
        // Get cluster metrics
        // 获取集群指标
        let mut metrics = HashMap::new();
        metrics.insert("runtime_type".to_string(), serde_json::Value::String("kubernetes".to_string()));
        metrics.insert("namespace".to_string(), serde_json::Value::String(self.config.namespace.clone()));
        Ok(metrics)
    }

    async fn scale_instance(
        &self,
        _instance: &Arc<TaskInstance>,
        _new_limits: &InstanceResourceLimits,
    ) -> ExecutionResult<()> {
        // Kubernetes jobs don't support scaling after creation
        // Kubernetes 作业在创建后不支持扩缩容
        Err(ExecutionError::RuntimeError {
            message: "Kubernetes jobs do not support scaling after creation".to_string(),
        })
    }

    async fn cleanup_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        // Clean up any remaining jobs
        // 清理任何剩余的作业
        if let Some(handle) = instance.get_runtime_handle::<KubernetesJobHandle>() {
            self.delete_job(&handle.job_name).await?;
        }
        Ok(())
    }

    fn validate_config(&self, config: &InstanceConfig) -> ExecutionResult<()> {
        // Validate Kubernetes-specific configuration
        // 验证 Kubernetes 特定配置
        let has_image = config.runtime_config.get("image")
            .and_then(|v| v.as_str())
            .is_some();
            
        if !has_image && self.config.default_image.is_empty() {
            return Err(ExecutionError::InvalidConfiguration {
                message: "No container image specified".to_string(),
            });
        }

        if self.config.namespace.is_empty() {
            return Err(ExecutionError::InvalidConfiguration {
                message: "Kubernetes namespace cannot be empty".to_string(),
            });
        }

        Ok(())
    }

    fn get_capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            supports_scaling: false, // Jobs don't support scaling
            supports_health_checks: true,
            supports_metrics: true,
            supports_hot_reload: false,
            supports_persistent_storage: true,
            supports_network_isolation: true,
            max_concurrent_instances: self.runtime_config.resource_pool.max_concurrent_instances,
            supported_protocols: vec!["http".to_string(), "grpc".to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::instance::{InstanceConfig, InstanceResourceLimits, NetworkConfig};

    #[test]
    fn test_kubernetes_config_default() {
        let config = KubernetesConfig::default();
        assert_eq!(config.namespace, "default");
        assert_eq!(config.default_image, "alpine:latest");
        assert_eq!(config.image_pull_policy, "IfNotPresent");
    }

    #[test]
    fn test_kubernetes_runtime_creation() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Kubernetes,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            resource_pool: ResourcePoolConfig::default(),
        };

        let runtime = KubernetesRuntime::new(&runtime_config);
        assert!(runtime.is_ok());
        
        let runtime = runtime.unwrap();
        assert_eq!(runtime.runtime_type(), RuntimeType::Kubernetes);
    }

    #[test]
    fn test_validate_config() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Kubernetes,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            resource_pool: ResourcePoolConfig::default(),
        };

        let runtime = KubernetesRuntime::new(&runtime_config).unwrap();

        // Valid config
        let mut runtime_config_map = HashMap::new();
        runtime_config_map.insert("image".to_string(), serde_json::Value::String("nginx:latest".to_string()));
        
        let valid_config = InstanceConfig {
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: runtime_config_map,
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 10,
            request_timeout_ms: 30000,
        };
        assert!(runtime.validate_config(&valid_config).is_ok());

        // Invalid config - no image
        let invalid_config = InstanceConfig {
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: HashMap::new(),
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 10,
            request_timeout_ms: 30000,
        };
        
        // This should be valid because we have a default image
        assert!(runtime.validate_config(&invalid_config).is_ok());
    }

    #[test]
    fn test_job_manifest_generation() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Kubernetes,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            resource_pool: ResourcePoolConfig::default(),
        };

        let runtime = KubernetesRuntime::new(&runtime_config).unwrap();
        
        let mut runtime_config_map = HashMap::new();
        runtime_config_map.insert("image".to_string(), serde_json::Value::String("nginx:latest".to_string()));
        
        let instance_config = InstanceConfig {
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: runtime_config_map,
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 10,
            request_timeout_ms: 30000,
        };

        let execution_context = ExecutionContext {
            execution_id: "test-execution-123".to_string(),
            payload: vec![],
            headers: HashMap::new(),
            timeout_ms: 30000,
            context_data: HashMap::new(),
        };

        let manifest = runtime.generate_job_manifest(&instance_config, "test-job", &execution_context);
        
        assert!(manifest.contains("kind: Job"));
        assert!(manifest.contains("test-job"));
        assert!(manifest.contains("nginx:latest"));
        assert!(manifest.contains("test-execution-123"));
    }

    #[test]
    fn test_kubernetes_job_handle() {
        let handle = KubernetesJobHandle {
            job_name: "test-job".to_string(),
            namespace: "default".to_string(),
            job_uid: "12345".to_string(),
            pod_name: Some("test-pod".to_string()),
            status: "Running".to_string(),
            created_at: std::time::SystemTime::now(),
        };

        assert_eq!(handle.job_name, "test-job");
        assert_eq!(handle.namespace, "default");
        assert_eq!(handle.job_uid, "12345");
        assert_eq!(handle.pod_name, Some("test-pod".to_string()));
        assert_eq!(handle.status, "Running");
    }

    #[test]
    fn test_kubernetes_capabilities() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Kubernetes,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            resource_pool: ResourcePoolConfig::default(),
        };

        let runtime = KubernetesRuntime::new(&runtime_config).unwrap();
        let capabilities = runtime.get_capabilities();

        assert!(!capabilities.supports_scaling);
        assert!(capabilities.supports_health_checks);
        assert!(capabilities.supports_metrics);
        assert!(!capabilities.supports_hot_reload);
        assert!(capabilities.supports_persistent_storage);
        assert!(capabilities.supports_network_isolation);
        assert!(capabilities.supported_protocols.contains(&"http".to_string()));
        assert!(capabilities.supported_protocols.contains(&"grpc".to_string()));
    }

    #[test]
    fn test_build_kubectl_args() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Kubernetes,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            resource_pool: ResourcePoolConfig::default(),
        };

        let runtime = KubernetesRuntime::new(&runtime_config).unwrap();
        let args = runtime.build_kubectl_args("apply", vec!["-f".to_string(), "manifest.yaml".to_string()]);

        assert!(args.contains(&"apply".to_string()));
        assert!(args.contains(&"-f".to_string()));
        assert!(args.contains(&"manifest.yaml".to_string()));
        assert!(args.contains(&"--namespace".to_string()));
        assert!(args.contains(&"default".to_string()));
    }

    #[test]
    fn test_kubernetes_config_with_custom_values() {
        let mut settings = HashMap::new();
        let k8s_config = KubernetesConfig {
            namespace: "custom-namespace".to_string(),
            default_image: "custom:image".to_string(),
            ..KubernetesConfig::default()
        };
        settings.insert("kubernetes".to_string(), serde_json::to_value(k8s_config).unwrap());

        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Kubernetes,
            settings,
            global_environment: HashMap::new(),
            resource_pool: ResourcePoolConfig::default(),
        };

        let runtime = KubernetesRuntime::new(&runtime_config).unwrap();
        assert_eq!(runtime.config.namespace, "custom-namespace");
        assert_eq!(runtime.config.default_image, "custom:image");
    }

    #[test]
    fn test_runtime_type() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Kubernetes,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            resource_pool: ResourcePoolConfig::default(),
        };

        let runtime = KubernetesRuntime::new(&runtime_config).unwrap();
        assert_eq!(runtime.runtime_type(), RuntimeType::Kubernetes);
    }
}