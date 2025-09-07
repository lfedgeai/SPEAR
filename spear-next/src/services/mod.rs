//! Service handlers for SPEAR Metadata Server / SPEAR元数据服务器的服务处理层
//!
//! This module contains all the service handlers that implement the business logic
//! for different API endpoints. Each handler is responsible for processing requests,
//! interacting with storage layers, and returning appropriate responses.
//!
//! 此模块包含实现不同API端点业务逻辑的所有服务处理器。每个处理器负责处理请求、
//! 与存储层交互并返回适当的响应。
//!
//! ## Architecture / 架构
//!
//! The handlers follow a layered architecture:
//! 处理器遵循分层架构：
//!
//! ```text
//! API Layer (gRPC/HTTP) -> Handlers -> Storage Layer
//! API层 (gRPC/HTTP) -> 处理器 -> 存储层
//! ```
//!
//! ## Available Handlers / 可用处理器
//!
//! - `node`: Node management operations / 节点管理操作
//! - `resource`: Resource monitoring and management / 资源监控和管理
//! - `task`: Task management and execution / 任务管理和执行

//!
//! ## Usage / 使用方法
//!
//! ```rust,no_run
//! use spear_next::services::node::NodeService;
//! use spear_next::services::resource::ResourceService;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create handlers with default memory backend
//! // 使用默认内存后端创建处理器
//! let node_service = NodeService::new();
//! let resource_service = ResourceService::new();
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod node;
pub mod resource;
pub mod service;
pub mod task;

#[cfg(test)]
pub mod test_utils;

// Re-export commonly used types for convenience
// 为方便使用重新导出常用类型
pub use config::*;
pub use error::*;
pub use node::{NodeService, NodeInfo, NodeStatus, ClusterStats};
pub use resource::{ResourceService, NodeResourceInfo};
pub use service::*;
pub use task::TaskService;