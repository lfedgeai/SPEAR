//! Spear-next: Next generation Spear components
//! Spear-next: 下一代Spear组件

// Shared modules / 共享模块
pub mod config;
pub mod proto;
pub mod network;
pub mod storage;
pub mod utils;

// Service-specific modules / 服务特定模块
pub mod sms;
pub mod spearlet;

// Legacy modules (to be migrated) / 遗留模块（待迁移）
pub mod constants;

// Re-exports / 重新导出
pub use config::*;
pub use network::*;
pub use storage::*;
pub use utils::*;