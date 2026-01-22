//! SMS module types and constants
//! SMS模块类型和常量
//!
//! This module contains common types and constants used throughout the SMS module.
//! 此模块包含SMS模块中使用的通用类型和常量。

use crate::proto::sms::{TaskPriority, TaskStatus};

/// Generic constant representing "no filter" for any filtering operation
/// 表示任何过滤操作中"无过滤器"的通用常量
pub const NO_FILTER: i32 = -1;

pub fn task_status_to_public_str(status: i32) -> &'static str {
    match TaskStatus::try_from(status).ok() {
        Some(TaskStatus::Registered) => "registered",
        Some(TaskStatus::Active) => "active",
        Some(TaskStatus::Inactive) => "inactive",
        _ => "unknown",
    }
}

pub fn parse_task_status_public_str(status: &str) -> Option<i32> {
    match status.trim().to_ascii_lowercase().as_str() {
        "unknown" => Some(TaskStatus::Unknown as i32),
        "registered" => Some(TaskStatus::Registered as i32),
        "active" => Some(TaskStatus::Active as i32),
        "inactive" => Some(TaskStatus::Inactive as i32),
        _ => None,
    }
}

pub fn task_priority_to_public_str(priority: i32) -> &'static str {
    match TaskPriority::try_from(priority).ok() {
        Some(TaskPriority::Low) => "low",
        Some(TaskPriority::Normal) => "normal",
        Some(TaskPriority::High) => "high",
        Some(TaskPriority::Urgent) => "urgent",
        _ => "unknown",
    }
}

pub fn parse_task_priority_public_str(priority: &str) -> i32 {
    match priority.trim().to_ascii_lowercase().as_str() {
        "low" => TaskPriority::Low as i32,
        "normal" => TaskPriority::Normal as i32,
        "high" => TaskPriority::High as i32,
        "urgent" => TaskPriority::Urgent as i32,
        _ => TaskPriority::Normal as i32,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_state_to_i32() {
        assert_eq!(FilterState::None.to_i32(), NO_FILTER);
        assert_eq!(FilterState::Value(42).to_i32(), 42);
    }

    #[test]
    fn test_filter_state_from_i32() {
        assert_eq!(FilterState::from_i32(NO_FILTER), FilterState::None);
        assert_eq!(FilterState::from_i32(42), FilterState::Value(42));
    }

    #[test]
    fn test_filter_state_is_active() {
        assert!(!FilterState::None.is_active());
        assert!(FilterState::Value(42).is_active());
    }

    #[test]
    fn test_filter_state_is_none() {
        assert!(FilterState::None.is_none());
        assert!(!FilterState::Value(42).is_none());
    }

    #[test]
    fn test_filter_state_value() {
        assert_eq!(FilterState::None.value(), None);
        assert_eq!(FilterState::Value(42).value(), Some(42));
    }

    #[test]
    fn test_task_status_to_public_str_maps_known_statuses() {
        assert_eq!(
            task_status_to_public_str(TaskStatus::Registered as i32),
            "registered"
        );
        assert_eq!(
            task_status_to_public_str(TaskStatus::Active as i32),
            "active"
        );
        assert_eq!(
            task_status_to_public_str(TaskStatus::Inactive as i32),
            "inactive"
        );
    }

    #[test]
    fn test_parse_task_status_public_str_registered() {
        assert_eq!(
            parse_task_status_public_str("registered"),
            Some(TaskStatus::Registered as i32)
        );
    }
}
