//! Constants and types used throughout the application
//! 应用程序中使用的常量和类型

/// Generic constant representing "no filter" for any filtering operation
/// 表示任何过滤操作中"无过滤器"的通用常量
pub const NO_FILTER: i32 = -1;

/// Enum representing different filter states for better type safety
/// 表示不同过滤器状态的枚举，提供更好的类型安全性
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterState {
    /// No filter applied / 不应用过滤器
    None,
    /// Filter with specific value / 使用特定值过滤
    Value(i32),
}

impl FilterState {
    /// Convert FilterState to i32 for protobuf compatibility
    /// 将FilterState转换为i32以兼容protobuf
    pub fn to_i32(self) -> i32 {
        match self {
            FilterState::None => NO_FILTER,
            FilterState::Value(v) => v,
        }
    }

    /// Create FilterState from i32 value
    /// 从i32值创建FilterState
    pub fn from_i32(value: i32) -> Self {
        if value == NO_FILTER {
            FilterState::None
        } else {
            FilterState::Value(value)
        }
    }

    /// Check if filter is active (has a value)
    /// 检查过滤器是否激活（有值）
    pub fn is_active(self) -> bool {
        matches!(self, FilterState::Value(_))
    }

    /// Check if filter is disabled (no filter)
    /// 检查过滤器是否禁用（无过滤器）
    pub fn is_none(self) -> bool {
        matches!(self, FilterState::None)
    }

    /// Get the filter value if it exists
    /// 获取过滤器值（如果存在）
    pub fn value(self) -> Option<i32> {
        match self {
            FilterState::None => None,
            FilterState::Value(v) => Some(v),
        }
    }
}