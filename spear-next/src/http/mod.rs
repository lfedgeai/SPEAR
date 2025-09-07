//! HTTP module for SPEAR Metadata Server
//! SPEAR元数据服务器的HTTP模块
//!
//! This module contains all HTTP-related functionality including:
//! 此模块包含所有HTTP相关功能，包括：
//! - HTTP gateway for gRPC service / gRPC服务的HTTP网关
//! - REST API routes / REST API路由
//! - HTTP request/response handlers / HTTP请求/响应处理器
//! - OpenAPI documentation / OpenAPI文档

pub mod gateway;
pub mod routes;
pub mod handlers;

// Re-export commonly used items / 重新导出常用项
pub use gateway::{GatewayState, create_gateway_router};
pub use routes::create_routes;