//! HTTP handlers for SPEAR Metadata Server
//! SPEAR元数据服务器的HTTP处理器
//!
//! This module contains HTTP request handlers for different API endpoints
//! 此模块包含不同API端点的HTTP请求处理器

pub mod common;
pub mod node;
pub mod resource;
pub mod docs;
pub mod health;
pub mod task;
pub mod files;

// Re-export all public items from each module / 重新导出每个模块的所有公共项
pub use common::*;
pub use node::*;
pub use resource::*;
pub use docs::*;
pub use health::*;
pub use task::*;
pub use files::*;
