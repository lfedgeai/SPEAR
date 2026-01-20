use crate::proto::sms::McpServerRecord;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::Value;
use std::collections::HashMap;

use crate::spearlet::param_keys::{chat as chat_keys, mcp as mcp_keys};

#[derive(Clone, Debug, Default)]
pub struct McpSessionParams {
    pub enabled: bool,
    pub server_ids: Vec<String>,
    pub task_tool_allowlist: Vec<String>,
    pub task_tool_denylist: Vec<String>,
    pub tool_allowlist: Vec<String>,
    pub tool_denylist: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpExecDecision {
    pub server_id: String,
    pub tool_name: String,
    pub timeout_ms: u64,
    pub max_tool_output_bytes: u64,
}

impl McpSessionParams {
    pub fn is_effectively_enabled(&self) -> bool {
        self.enabled && !self.server_ids.is_empty()
    }

    pub fn materialize_into(&self, params: &mut HashMap<String, Value>) {
        if !self.is_effectively_enabled() {
            return;
        }
        params.insert(mcp_keys::param::ENABLED.to_string(), Value::Bool(true));
        params.insert(
            mcp_keys::param::SERVER_IDS.to_string(),
            Value::Array(
                self.server_ids
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
        );
        if !self.task_tool_allowlist.is_empty() {
            params.insert(
                mcp_keys::param::TASK_TOOL_ALLOWLIST.to_string(),
                Value::Array(
                    self.task_tool_allowlist
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        if !self.task_tool_denylist.is_empty() {
            params.insert(
                mcp_keys::param::TASK_TOOL_DENYLIST.to_string(),
                Value::Array(
                    self.task_tool_denylist
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        if !self.tool_allowlist.is_empty() {
            params.insert(
                mcp_keys::param::TOOL_ALLOWLIST.to_string(),
                Value::Array(
                    self.tool_allowlist
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        if !self.tool_denylist.is_empty() {
            params.insert(
                mcp_keys::param::TOOL_DENYLIST.to_string(),
                Value::Array(
                    self.tool_denylist
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
    }
}

pub fn server_allowed_tools(server: &McpServerRecord) -> Vec<String> {
    let mut allowed = server.allowed_tools.clone();
    allowed.retain(|p| !p.is_empty());
    allowed
}

fn encode_tool_token(s: &str) -> String {
    URL_SAFE_NO_PAD.encode(s.as_bytes())
}

fn decode_tool_token(s: &str) -> Result<String, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(s.as_bytes())
        .map_err(|_| "invalid mcp tool name".to_string())?;
    String::from_utf8(bytes).map_err(|_| "invalid mcp tool name".to_string())
}

fn encode_openai_mcp_tool_name(server_id: &str, tool_name: &str) -> String {
    format!(
        "{}{}__{}",
        mcp_keys::tool::NAMESPACE_PREFIX_DBL_UNDERSCORE,
        encode_tool_token(server_id),
        encode_tool_token(tool_name)
    )
}

pub fn parse_namespaced_mcp_tool_name(namespaced: &str) -> Result<(String, String), String> {
    if let Some(rest) = namespaced.strip_prefix(mcp_keys::tool::NAMESPACE_PREFIX_DBL_UNDERSCORE) {
        let mut it = rest.splitn(2, "__");
        let sid_enc = it.next().unwrap_or("");
        let tool_enc = it.next().unwrap_or("");
        if sid_enc.is_empty() || tool_enc.is_empty() {
            return Err("invalid mcp tool name".to_string());
        }
        let server_id = decode_tool_token(sid_enc)?;
        let tool_name = decode_tool_token(tool_enc)?;
        if server_id.is_empty() || tool_name.is_empty() {
            return Err("invalid mcp tool name".to_string());
        }
        return Ok((server_id, tool_name));
    }

    let rest = namespaced
        .strip_prefix(mcp_keys::tool::NAMESPACE_PREFIX_DOT)
        .unwrap_or(namespaced);
    let (server_id, tool_name) = rest
        .split_once('.')
        .ok_or_else(|| "invalid mcp tool name".to_string())?;
    if server_id.is_empty() || tool_name.is_empty() {
        return Err("invalid mcp tool name".to_string());
    }
    Ok((server_id.to_string(), tool_name.to_string()))
}

pub fn allowed_by_policies(
    server_allowed: &[String],
    session: &McpSessionParams,
    tool_name: &str,
) -> Result<(), String> {
    if server_allowed.is_empty() || !match_any_pattern(server_allowed, tool_name) {
        return Err("mcp tool denied by server policy".to_string());
    }
    if !session.task_tool_allowlist.is_empty()
        && !match_any_pattern(&session.task_tool_allowlist, tool_name)
    {
        return Err("mcp tool denied by task allowlist".to_string());
    }
    if !session.task_tool_denylist.is_empty()
        && match_any_pattern(&session.task_tool_denylist, tool_name)
    {
        return Err("mcp tool denied by task denylist".to_string());
    }
    if !session.tool_allowlist.is_empty() && !match_any_pattern(&session.tool_allowlist, tool_name)
    {
        return Err("mcp tool denied by session allowlist".to_string());
    }
    if !session.tool_denylist.is_empty() && match_any_pattern(&session.tool_denylist, tool_name) {
        return Err("mcp tool denied by session denylist".to_string());
    }
    Ok(())
}

pub fn decide_mcp_exec(
    session: &McpSessionParams,
    params: &HashMap<String, Value>,
    server: &McpServerRecord,
    namespaced_tool_name: &str,
) -> Result<McpExecDecision, String> {
    let (server_id, tool_name) = parse_namespaced_mcp_tool_name(namespaced_tool_name)?;
    if server.server_id != server_id {
        return Err("unknown mcp server".to_string());
    }

    let allowed = server_allowed_tools(server);
    allowed_by_policies(&allowed, &session, &tool_name)?;

    let timeout_ms = server
        .budgets
        .as_ref()
        .map(|b| b.tool_timeout_ms)
        .unwrap_or(8000)
        .max(100)
        .min(120_000);
    let max_tool_output_bytes = params
        .get(chat_keys::MAX_TOOL_OUTPUT_BYTES)
        .and_then(|v| v.as_u64())
        .unwrap_or(64 * 1024)
        .min(10 * 1024 * 1024);

    Ok(McpExecDecision {
        server_id,
        tool_name,
        timeout_ms,
        max_tool_output_bytes,
    })
}

pub fn filter_and_namespace_openai_tools(
    server_id: &str,
    server_allowed: &[String],
    session: &McpSessionParams,
    tools: &[serde_json::Value],
) -> Vec<String> {
    if server_allowed.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for t in tools.iter() {
        let name = t.get("name").and_then(|x| x.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        if allowed_by_policies(server_allowed, session, name).is_err() {
            continue;
        }
        let ns_name = encode_openai_mcp_tool_name(server_id, name);
        let desc = t.get("description").and_then(|x| x.as_str()).unwrap_or("");
        let params = t
            .get("inputSchema")
            .cloned()
            .or_else(|| t.get("input_schema").cloned())
            .unwrap_or_else(|| serde_json::json!({"type":"object"}));
        let tool_def = serde_json::json!({
            "type": "function",
            "function": {
                "name": ns_name,
                "description": desc,
                "parameters": params
            }
        });
        if let Ok(s) = serde_json::to_string(&tool_def) {
            out.push(s);
        }
    }
    out
}

pub fn match_any_pattern(patterns: &[String], s: &str) -> bool {
    for p in patterns.iter() {
        if wildcard_match(p, s) {
            return true;
        }
    }
    false
}

pub fn wildcard_match(pattern: &str, s: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let mut pi = 0usize;
    let mut si = 0usize;
    let p = pattern.as_bytes();
    let b = s.as_bytes();
    let mut star: Option<usize> = None;
    let mut match_si: usize = 0;

    while si < b.len() {
        if pi < p.len() && (p[pi] == b[si] || p[pi] == b'?') {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            pi += 1;
            match_si = si;
        } else if let Some(st) = star {
            pi = st + 1;
            match_si += 1;
            si = match_si;
        } else {
            return false;
        }
    }

    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::sms::{McpBudgets, McpServerRecord, McpTransport};

    #[test]
    fn test_wildcard_match() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("read_*", "read_file"));
        assert!(!wildcard_match("read_*", "write_file"));
        assert!(wildcard_match("a?c", "abc"));
        assert!(!wildcard_match("a?c", "ac"));
        assert!(wildcard_match("ab*cd", "abcd"));
        assert!(wildcard_match("ab*cd", "ab__cd"));
        assert!(!wildcard_match("ab*cd", "ab__ce"));
    }

    #[test]
    fn test_parse_namespaced() {
        assert_eq!(
            parse_namespaced_mcp_tool_name("mcp.fs.read_file").unwrap(),
            ("fs".to_string(), "read_file".to_string())
        );
        assert_eq!(
            parse_namespaced_mcp_tool_name(&encode_openai_mcp_tool_name("fs", "read_file"))
                .unwrap(),
            ("fs".to_string(), "read_file".to_string())
        );
        assert!(parse_namespaced_mcp_tool_name("mcp.fs").is_err());
    }

    #[test]
    fn test_allowed_by_policies() {
        let server_allowed = vec!["read_*".to_string()];
        let session = McpSessionParams {
            enabled: true,
            server_ids: vec!["fs".to_string()],
            task_tool_allowlist: vec![],
            task_tool_denylist: vec![],
            tool_allowlist: vec!["read_*".to_string()],
            tool_denylist: vec!["read_secret".to_string()],
        };
        assert!(allowed_by_policies(&server_allowed, &session, "read_file").is_ok());
        assert!(allowed_by_policies(&server_allowed, &session, "write_file").is_err());
        assert!(allowed_by_policies(&server_allowed, &session, "read_secret").is_err());
    }

    #[test]
    fn test_filter_and_namespace_openai_tools() {
        let session = McpSessionParams {
            enabled: true,
            server_ids: vec!["fs".to_string()],
            task_tool_allowlist: vec![],
            task_tool_denylist: vec![],
            tool_allowlist: vec![],
            tool_denylist: vec!["delete_*".to_string()],
        };
        let server_allowed = vec!["*".to_string()];
        let tools = vec![
            serde_json::json!({"name":"read_file","description":"d"}),
            serde_json::json!({"name":"delete_file","description":"d"}),
        ];
        let out = filter_and_namespace_openai_tools("fs", &server_allowed, &session, &tools);
        assert_eq!(out.len(), 1);
        let v: serde_json::Value = serde_json::from_str(&out[0]).unwrap();
        assert_eq!(
            v["function"]["name"],
            encode_openai_mcp_tool_name("fs", "read_file")
        );
    }

    #[test]
    fn test_decide_exec_uses_budgets_and_limits() {
        let session = McpSessionParams {
            enabled: true,
            server_ids: vec!["fs".to_string()],
            tool_allowlist: vec!["read_*".to_string()],
            ..Default::default()
        };

        let mut params = std::collections::HashMap::new();
        params.insert(
            "max_tool_output_bytes".to_string(),
            serde_json::Value::Number(1024.into()),
        );

        let server = McpServerRecord {
            server_id: "fs".to_string(),
            display_name: "".to_string(),
            transport: McpTransport::Stdio as i32,
            stdio: None,
            http: None,
            tool_namespace: "".to_string(),
            allowed_tools: vec!["read_*".to_string()],
            approval_policy: None,
            budgets: Some(McpBudgets {
                tool_timeout_ms: 1234,
                max_concurrency: 0,
                max_tool_output_bytes: 0,
            }),
            updated_at_ms: 0,
        };

        let d = decide_mcp_exec(&session, &params, &server, "mcp.fs.read_file").unwrap();
        assert_eq!(d.timeout_ms, 1234);
        assert_eq!(d.max_tool_output_bytes, 1024);
        assert_eq!(d.tool_name, "read_file");
    }
}
