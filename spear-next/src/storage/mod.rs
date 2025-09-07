//! Storage module for spear-next
//! 
//! This module contains all storage-related abstractions and implementations.
//! It provides a unified interface for different storage backends.
//! 
//! spear-next的存储模块
//! 
//! 此模块包含所有与存储相关的抽象和实现。
//! 它为不同的存储后端提供统一的接口。

pub mod kv;

// Re-export commonly used types for convenience
// 为方便使用重新导出常用类型
pub use kv::{
    KvStore, KvKey, KvValue, KvStoreType, KvStoreConfig, KvPair, RangeOptions,
    MemoryKvStore, create_kv_store, serialization,
    KvStoreFactory, DefaultKvStoreFactory,
    set_kv_store_factory, get_kv_store_factory,
    create_kv_store_from_config, create_kv_store_from_env
};

#[cfg(feature = "sled")]
pub use kv::SledKvStore;