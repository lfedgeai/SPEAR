//! HTTP handlers for SPEAR Metadata Server
//! SPEAR元数据服务器的HTTP处理器
//!
//! This module contains HTTP request handlers for different API endpoints
//! 此模块包含不同API端点的HTTP请求处理器

pub mod common;
pub mod docs;
pub mod files;
pub mod health;
pub mod node;
pub mod placement;
pub mod resource;
pub mod task;

// Re-export all public items from each module / 重新导出每个模块的所有公共项
pub use common::*;
pub use docs::*;
pub use files::*;
pub use health::*;
pub use node::*;
pub use placement::*;
pub use resource::*;
pub use task::*;
