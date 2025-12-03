//! Network infrastructure shared between SMS and Spearlet
//! SMS和Spearlet共享的网络基础设施
//!
//! This module provides common networking components including:
//! - gRPC error handling utilities
//!
//! 此模块提供通用的网络组件，包括：
//! - gRPC错误处理工具

pub mod grpc;

pub use grpc::*;
