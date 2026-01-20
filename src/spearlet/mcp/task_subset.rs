use std::collections::{HashMap, HashSet};

use crate::spearlet::mcp::policy::McpSessionParams;
use crate::spearlet::param_keys::mcp as mcp_keys;

#[derive(Clone, Debug, Default)]
pub struct McpTaskPolicy {
    pub enabled: bool,
    pub default_server_ids: Vec<String>,
    pub allowed_server_ids: Vec<String>,
    pub task_tool_allowlist: Vec<String>,
    pub task_tool_denylist: Vec<String>,
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "on" => Some(true),
        "false" | "0" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

fn parse_string_list_str(s: &str) -> Vec<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Ok(arr) = serde_json::from_str::<Vec<String>>(trimmed) {
        return arr
            .into_iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
    }
    trimmed
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn get_str<'a>(map: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    map.get(key).map(|s| s.as_str())
}

pub fn parse_task_config(map: &HashMap<String, String>) -> McpTaskPolicy {
    let enabled = get_str(map, mcp_keys::task_config::ENABLED)
        .and_then(parse_bool)
        .unwrap_or(false);
    let default_server_ids = get_str(map, mcp_keys::task_config::DEFAULT_SERVER_IDS)
        .map(parse_string_list_str)
        .unwrap_or_default();
    let allowed_server_ids = get_str(map, mcp_keys::task_config::ALLOWED_SERVER_IDS)
        .map(parse_string_list_str)
        .unwrap_or_else(|| default_server_ids.clone());

    let task_tool_allowlist = get_str(map, mcp_keys::task_config::TOOL_ALLOWLIST)
        .map(parse_string_list_str)
        .unwrap_or_default();
    let task_tool_denylist = get_str(map, mcp_keys::task_config::TOOL_DENYLIST)
        .map(parse_string_list_str)
        .unwrap_or_default();

    McpTaskPolicy {
        enabled,
        default_server_ids,
        allowed_server_ids,
        task_tool_allowlist,
        task_tool_denylist,
    }
}

pub fn task_default_session_params(task: &McpTaskPolicy) -> McpSessionParams {
    if !task.enabled {
        return McpSessionParams::default();
    }

    let allowed_set: HashSet<&str> = task.allowed_server_ids.iter().map(|s| s.as_str()).collect();
    let mut seen = HashSet::<String>::new();
    let mut server_ids = Vec::new();
    for sid in task.default_server_ids.iter() {
        let sid = sid.trim();
        if sid.is_empty() {
            continue;
        }
        if !allowed_set.contains(sid) {
            continue;
        }
        if seen.insert(sid.to_string()) {
            server_ids.push(sid.to_string());
        }
    }
    if server_ids.is_empty() {
        return McpSessionParams::default();
    }

    McpSessionParams {
        enabled: true,
        server_ids,
        task_tool_allowlist: task.task_tool_allowlist.clone(),
        task_tool_denylist: task.task_tool_denylist.clone(),
        ..Default::default()
    }
}

pub fn validate_requested_server_ids(
    task: &McpTaskPolicy,
    requested: &[String],
) -> Result<(), String> {
    let allowed: HashSet<&str> = task.allowed_server_ids.iter().map(|s| s.as_str()).collect();
    for sid in requested.iter() {
        if sid.is_empty() {
            continue;
        }
        if !allowed.contains(sid.as_str()) {
            return Err(format!("mcp server not allowed: {}", sid));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_task_config_json_lists() {
        let mut m = HashMap::new();
        m.insert(
            mcp_keys::task_config::ENABLED.to_string(),
            "true".to_string(),
        );
        m.insert(
            mcp_keys::task_config::DEFAULT_SERVER_IDS.to_string(),
            "[\"gitlab\",\"fs\"]".to_string(),
        );
        let p = parse_task_config(&m);
        assert!(p.enabled);
        assert_eq!(
            p.default_server_ids,
            vec!["gitlab".to_string(), "fs".to_string()]
        );
        assert_eq!(p.allowed_server_ids, p.default_server_ids);
    }

    #[test]
    fn test_task_default_intersects_allowed() {
        let task = McpTaskPolicy {
            enabled: true,
            default_server_ids: vec!["b".to_string(), "c".to_string(), "b".to_string()],
            allowed_server_ids: vec!["a".to_string(), "b".to_string()],
            ..Default::default()
        };
        let eff = task_default_session_params(&task);
        assert_eq!(eff.server_ids, vec!["b".to_string()]);
    }

    #[test]
    fn test_task_default_includes_task_tool_lists() {
        let task = McpTaskPolicy {
            enabled: true,
            default_server_ids: vec!["gitlab".to_string()],
            allowed_server_ids: vec!["gitlab".to_string()],
            task_tool_allowlist: vec!["read_*".to_string()],
            task_tool_denylist: vec!["delete_*".to_string()],
        };
        let eff = task_default_session_params(&task);
        assert!(eff.enabled);
        assert_eq!(eff.server_ids, vec!["gitlab".to_string()]);
        assert_eq!(eff.task_tool_allowlist, vec!["read_*".to_string()]);
        assert_eq!(eff.task_tool_denylist, vec!["delete_*".to_string()]);
    }
}
