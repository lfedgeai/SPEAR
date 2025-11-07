# Runtime 创建 Task Instance 和获取 Communication Channel 流程

## 概述

在 spear-next 中，runtime 创建 task instance 并获取 communication channel 是一个多层次的过程，涉及任务执行管理器、运行时管理器、通信工厂等多个组件的协作。

## 核心组件

### 1. TaskExecutionManager（任务执行管理器）
- 负责整体的任务执行流程管理
- 管理 artifacts、tasks 和 instances 的生命周期
- 协调 runtime 和 communication 组件

### 2. RuntimeManager（运行时管理器）
- 管理不同类型的 runtime（Process、Kubernetes、WASM）
- 提供 runtime 实例的创建和管理接口

### 3. CommunicationFactory（通信工厂）
- 为不同的 runtime 类型创建相应的通信通道
- 支持 UnixSocket、TCP、gRPC 等多种通信方式
- 管理通信通道的生命周期和连接池

## 详细流程

### 阶段 1：请求处理和任务准备

1. **接收执行请求**
   ```rust
   // 在 TaskExecutionManager::submit_execution 中
   pub async fn submit_execution(
       &self,
       request: InvokeFunctionRequest,
   ) -> ExecutionResult<super::ExecutionResponse>
   ```

2. **创建或获取 Artifact**
   ```rust
   // 获取或创建 artifact
   let artifact = self.get_or_create_artifact(artifact_spec).await?;
   ```

3. **创建或获取 Task**
   ```rust
   // 获取或创建任务
   let task = self.get_or_create_task(&artifact).await?;
   ```

### 阶段 2：Instance 创建和启动

4. **获取或创建 Instance**
   ```rust
   async fn get_or_create_instance(&self, task: &Arc<Task>) -> ExecutionResult<Arc<TaskInstance>> {
       // 尝试找到可用实例
       if let Some(instance) = self.scheduler.select_instance(task).await? {
           return Ok(instance);
       }

       // 检查实例限制
       if task.instance_count() >= self.config.max_instances_per_task {
           return Err(ExecutionError::ResourceExhausted { ... });
       }

       // 创建新实例
       let instance_id = task.generate_instance_id();
       let runtime = self.runtime_manager.get_runtime(&task.spec.runtime_type)
           .ok_or_else(|| ExecutionError::RuntimeError { ... })?;

       let instance_config = task.create_instance_config();
       let instance = runtime.create_instance(&instance_config).await?;

       // 启动实例
       runtime.start_instance(&instance).await?;

       // 注册实例
       self.instances.insert(instance_id.clone(), instance.clone());
       task.add_instance(instance.clone())?;
       self.scheduler.add_instance(instance.clone()).await?;

       Ok(instance)
   }
   ```

### 阶段 3：Runtime 实例创建

5. **Runtime 创建 Instance**
   以 ProcessRuntime 为例：
   ```rust
   async fn create_instance(
       &self,
       config: &InstanceConfig,
   ) -> ExecutionResult<Arc<TaskInstance>> {
       let instance_id = format!("proc-{}", Uuid::new_v4());
       let instance = Arc::new(TaskInstance::new(instance_id, config.clone()));

       // 创建进程句柄
       let handle = ProcessHandle {
           pid: 0, // 将在启动时设置
           command: config.runtime_config.get("command")
               .and_then(|v| v.as_str())
               .unwrap_or(&self.config.default_executable)
               .to_string(),
           args: config.runtime_config.get("args")
               .and_then(|v| v.as_array())
               .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
               .unwrap_or_default(),
           working_directory: config.runtime_config.get("working_directory")
               .and_then(|v| v.as_str())
               .unwrap_or(&self.config.working_directory)
               .to_string(),
           environment: config.environment.clone(),
           start_time: SystemTime::now(),
           child: Arc::new(Mutex::new(None)),
       };

       // 设置运行时句柄
       instance.set_runtime_handle(handle);
       instance.set_status(InstanceStatus::Creating);

       Ok(instance)
   }
   ```

6. **启动 Instance**
   ```rust
   async fn start_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
       let handle = instance.get_runtime_handle::<ProcessHandle>()
           .ok_or_else(|| ExecutionError::RuntimeError { ... })?;

       let mut command = self.build_process_command(&instance.config);
       
       // 启动进程
       let child = command.spawn()
           .map_err(|e| ExecutionError::RuntimeError { ... })?;

       let pid = child.id().unwrap_or(0);
       
       // 更新句柄
       *handle.child.lock().await = Some(child);
       
       instance.set_status(InstanceStatus::Ready);
       Ok(())
   }
   ```

### 阶段 4：Communication Channel 获取

7. **通信通道的创建和管理**
   
   虽然当前代码中没有直接显示 communication channel 与 instance 的关联，但根据架构设计，通信通道的获取流程应该是：

   ```rust
   // 在 CommunicationFactory 中
   pub async fn get_or_create_channel(
       &mut self,
       instance_id: RuntimeInstanceId,
       custom_config: Option<ChannelConfig>,
   ) -> CommunicationResult<Arc<dyn CommunicationChannel>> {
       // 检查是否已有活跃通道
       if self.pool_enabled {
           if let Some(channel) = self.active_channels.get(&instance_id) {
               if channel.is_connected().await {
                   return Ok(channel.clone());
               } else {
                   self.active_channels.remove(&instance_id);
               }
           }
       }

       // 创建新通道
       let channel = self.create_channel_for_instance(instance_id.clone(), custom_config).await?;
       
       // 存储到池中
       if self.pool_enabled {
           self.active_channels.insert(instance_id, channel.clone());
       }
       
       Ok(channel)
   }
   ```

8. **根据 Runtime 类型选择通信策略**
   ```rust
   fn init_default_strategies(&mut self) {
       // Process runtime 策略
       let process_strategy = CommunicationStrategy::new(
           RuntimeType::Process,
           "unix".to_string(),
           vec!["tcp".to_string()],
           ChannelConfig {
               channel_type: "unix".to_string(),
               address: "/tmp/spear-process.sock".to_string(),
               ..Default::default()
           },
       );
       self.strategies.insert(RuntimeType::Process, process_strategy);
       
       // Kubernetes runtime 策略
       let k8s_strategy = CommunicationStrategy::new(
           RuntimeType::Kubernetes,
           "grpc".to_string(),
           vec!["tcp".to_string()],
           ChannelConfig {
               channel_type: "grpc".to_string(),
               address: "http://127.0.0.1:50051".to_string(),
               ..Default::default()
           },
       );
       self.strategies.insert(RuntimeType::Kubernetes, k8s_strategy);
       
       // WASM runtime 策略（进程内通信）
       let wasm_strategy = CommunicationStrategy::new(
           RuntimeType::Wasm,
           "memory".to_string(),
           vec![],
           ChannelConfig {
               channel_type: "memory".to_string(),
               address: "in-process".to_string(),
               ..Default::default()
           },
       );
       self.strategies.insert(RuntimeType::Wasm, wasm_strategy);
   }
   ```

### 阶段 5：执行和通信

9. **执行请求**
   ```rust
   async fn execute_request(
       &self,
       artifact_spec: ProtoArtifactSpec,
       execution_context: ExecutionContext,
   ) -> ExecutionResult<super::ExecutionResponse> {
       // 获取实例
       let instance = self.get_or_create_instance(&task).await?;

       // 在实例上执行
       let runtime = self.runtime_manager.get_runtime(&task.spec.runtime_type)?;
       let runtime_response = runtime.execute(&instance, execution_context).await?;
       
       // 转换响应
       Ok(super::ExecutionResponse { ... })
   }
   ```

10. **Runtime 执行**
    ```rust
    async fn execute(
        &self,
        instance: &Arc<TaskInstance>,
        _context: ExecutionContext,
    ) -> ExecutionResult<RuntimeExecutionResponse> {
        let handle = instance.get_runtime_handle::<ProcessHandle>()?;

        // 通过通信通道与进程交互
        // 这里会使用相应的 CommunicationChannel 实现
        let child_guard = handle.child.lock().await;
        if let Some(_child) = child_guard.as_ref() {
            // 实际的通信逻辑
            Ok(RuntimeExecutionResponse::new_sync(...))
        } else {
            Err(ExecutionError::RuntimeError { ... })
        }
    }
    ```

## 通信通道类型

### 1. UnixSocketChannel
- 用于本地进程间通信
- 高性能，低延迟
- 适用于 Process Runtime

### 2. TcpChannel
- 用于网络通信
- 支持远程实例
- 适用于分布式部署

### 3. GrpcChannel
- 用于高级 RPC 通信
- 支持流式传输
- 适用于 Kubernetes Runtime

## 关键数据结构

### TaskInstance
```rust
pub struct TaskInstance {
    pub id: InstanceId,
    pub task_id: TaskId,
    pub config: InstanceConfig,
    pub status: Arc<parking_lot::RwLock<InstanceStatus>>,
    pub health_status: Arc<parking_lot::RwLock<HealthStatus>>,
    pub metrics: Arc<parking_lot::RwLock<InstanceMetrics>>,
    pub runtime_handle: Arc<parking_lot::RwLock<Option<Box<dyn std::any::Any + Send + Sync>>>>,
    // ...
}
```

### RuntimeInstanceId
```rust
pub struct RuntimeInstanceId {
    pub runtime_type: RuntimeType,
    pub instance_id: String,
    pub namespace: Option<String>,
}
```

### CommunicationChannel Trait
```rust
pub trait CommunicationChannel: Send + Sync {
    async fn connect(&mut self) -> CommunicationResult<()>;
    async fn disconnect(&mut self) -> CommunicationResult<()>;
    async fn send_message(&self, message: &[u8]) -> CommunicationResult<()>;
    async fn receive_message(&self) -> CommunicationResult<Vec<u8>>;
    async fn is_connected(&self) -> bool;
    async fn close(&mut self) -> CommunicationResult<()>;
    async fn get_stats(&self) -> CommunicationResult<ChannelStats>;
}
```

## 总结

整个流程可以概括为：

1. **请求接收** → TaskExecutionManager 接收执行请求
2. **资源准备** → 创建或获取 Artifact 和 Task
3. **实例管理** → 通过 RuntimeManager 创建和启动 TaskInstance
4. **通信建立** → CommunicationFactory 根据 Runtime 类型创建相应的通信通道
5. **执行交互** → Runtime 通过通信通道与实例进行交互
6. **结果返回** → 将执行结果返回给调用方

这个设计实现了运行时类型的抽象化、通信方式的可插拔性，以及实例生命周期的完整管理。