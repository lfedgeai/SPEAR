//! SPEARlet module - SPEAR core agent component
//! SPEARlet模块 - SPEAR核心代理组件
//!
//! This module provides the core functionality for SPEARlet nodes:
//! 此模块为SPEARlet节点提供核心功能：
//!
//! - Core agent functionality similar to kubelet / 类似kubelet的核心代理功能
//! - Object metadata management / 对象元数据管理
//! - gRPC and HTTP API handlers / gRPC和HTTP API处理器
//!
//! ## Architecture / 架构
//!
//! The Spearlet follows a microservice architecture with the following components:
//! Spearlet遵循微服务架构，包含以下组件：
//!
//! ```text
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │   HTTP Gateway  │    │   gRPC Server   │    │ Object Service  │
//! │   HTTP网关      │    │   gRPC服务器    │    │   对象服务      │
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//!          │                       │                       │
//!          └───────────────────────┼───────────────────────┘
//!                                  │
//!                         ┌─────────────────┐
//!                         │   Storage Layer │
//!                         │   存储层        │
//!                         └─────────────────┘
//! ```

pub mod config;
pub mod grpc_server;
pub mod http_gateway;
pub mod object_service;
pub mod registration;

#[cfg(test)]
mod config_test;
#[cfg(test)]
mod grpc_server_test;
#[cfg(test)]
mod object_service_test;
#[cfg(test)]
mod http_gateway_test;
#[cfg(test)]
mod registration_test;

// Re-export commonly used types / 重新导出常用类型
pub use config::{AppConfig, CliArgs, SpearletConfig};
pub use grpc_server::{GrpcServer, HealthService};
pub use http_gateway::HttpGateway;
pub use object_service::ObjectServiceImpl;
pub use registration::{RegistrationService, RegistrationState};