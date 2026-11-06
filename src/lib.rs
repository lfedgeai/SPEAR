//! Spear-next: Next generation Spear components
//! Spear-next: 下一代Spear组件

#![recursion_limit = "512"]

// Shared modules / 共享模块
pub mod ai_backend_types;
pub mod config;
pub mod proto;
pub mod storage;
pub mod utils;

// Service-specific modules / 服务特定模块
pub mod sms;
pub mod spearlet;

// Re-exports / 重新导出
pub use config::*;
pub use storage::*;
pub use utils::*;
