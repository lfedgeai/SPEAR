use rmcp::{
    model::CallToolRequestParam,
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;

use crate::proto::sms::{McpServerRecord, McpTransport};

fn resolve_env_value(raw: &str) -> Result<String, String> {
    let Some(inner) = raw.strip_prefix("${ENV:").and_then(|s| s.strip_suffix('}')) else {
        return Ok(raw.to_string());
    };

    let inner = inner.trim();
    if inner.is_empty() {
        return Err("invalid env reference: empty variable name".to_string());
    }

    let (var, default) = inner
        .split_once(":-")
        .map(|(a, b)| (a.trim(), Some(b.to_string())))
        .unwrap_or((inner, None));

    if var.is_empty() {
        return Err("invalid env reference: empty variable name".to_string());
    }

    match std::env::var(var) {
        Ok(v) => Ok(v),
        Err(_) => default.ok_or_else(|| format!("missing environment variable: {}", var)),
    }
}

fn resolve_stdio_env(env: &HashMap<String, String>) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::with_capacity(env.len());
    for (k, v) in env.iter() {
        let resolved =
            resolve_env_value(v).map_err(|e| format!("env {} resolve failed: {}", k, e))?;
        out.push((k.clone(), resolved));
    }
    Ok(out)
}

pub async fn list_tools(record: &McpServerRecord) -> Result<Vec<serde_json::Value>, String> {
    let transport = record.transport;
    if transport == McpTransport::Stdio as i32 {
        let stdio = record
            .stdio
            .as_ref()
            .ok_or_else(|| "missing stdio config".to_string())?;
        let resolved_env = resolve_stdio_env(&stdio.env)
            .map_err(|e| format!("mcp server {}: {}", record.server_id, e))?;
        let cmd = Command::new(&stdio.command).configure(|c| {
            for a in stdio.args.iter() {
                c.arg(a);
            }
            if !stdio.cwd.is_empty() {
                c.current_dir(&stdio.cwd);
            }
            for (k, v) in resolved_env.iter() {
                c.env(k, v);
            }
            c.stderr(Stdio::null());
        });

        let peer =
            ().serve(TokioChildProcess::new(cmd).map_err(|e| format!("spawn failed: {}", e))?)
                .await
                .map_err(|e| format!("connect failed: {}", e))?;

        let tools = peer
            .list_tools(Default::default())
            .await
            .map_err(|e| format!("list_tools failed: {}", e))?;

        let _ = peer.cancel().await;

        let v = serde_json::to_value(tools).map_err(|e| e.to_string())?;
        if let Some(arr) = v.as_array() {
            return Ok(arr.clone());
        }
        let arr = v
            .get("tools")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(arr)
    } else {
        Err("unsupported transport".to_string())
    }
}

pub async fn call_tool(
    record: &McpServerRecord,
    tool_name: &str,
    args_json: &str,
) -> Result<serde_json::Value, String> {
    let transport = record.transport;
    if transport == McpTransport::Stdio as i32 {
        let stdio = record
            .stdio
            .as_ref()
            .ok_or_else(|| "missing stdio config".to_string())?;
        let resolved_env = resolve_stdio_env(&stdio.env)
            .map_err(|e| format!("mcp server {}: {}", record.server_id, e))?;
        let cmd = Command::new(&stdio.command).configure(|c| {
            for a in stdio.args.iter() {
                c.arg(a);
            }
            if !stdio.cwd.is_empty() {
                c.current_dir(&stdio.cwd);
            }
            for (k, v) in resolved_env.iter() {
                c.env(k, v);
            }
            c.stderr(Stdio::null());
        });

        let peer =
            ().serve(TokioChildProcess::new(cmd).map_err(|e| format!("spawn failed: {}", e))?)
                .await
                .map_err(|e| format!("connect failed: {}", e))?;

        let args_val =
            serde_json::from_str::<serde_json::Value>(args_json).unwrap_or(serde_json::Value::Null);
        let args_obj = args_val.as_object().cloned();

        let out = peer
            .call_tool(CallToolRequestParam {
                name: tool_name.to_string().into(),
                arguments: args_obj,
            })
            .await
            .map_err(|e| format!("call_tool failed: {}", e))?;

        let _ = peer.cancel().await;
        serde_json::to_value(out).map_err(|e| e.to_string())
    } else {
        Err("unsupported transport".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_value_passthrough() {
        assert_eq!(resolve_env_value("abc").unwrap(), "abc");
    }

    #[test]
    fn test_resolve_env_value_required() {
        let name = format!("SPEAR_TEST_ENV_REQUIRED_{}", std::process::id());
        std::env::set_var(&name, "v1");
        assert_eq!(
            resolve_env_value(&format!("${{ENV:{}}}", name)).unwrap(),
            "v1"
        );
        std::env::remove_var(&name);
    }

    #[test]
    fn test_resolve_env_value_missing() {
        let name = format!("SPEAR_TEST_ENV_MISSING_{}", std::process::id());
        std::env::remove_var(&name);
        let err = resolve_env_value(&format!("${{ENV:{}}}", name)).unwrap_err();
        assert!(err.contains(&name));
    }

    #[test]
    fn test_resolve_env_value_default() {
        let name = format!("SPEAR_TEST_ENV_DEFAULT_{}", std::process::id());
        std::env::remove_var(&name);
        assert_eq!(
            resolve_env_value(&format!("${{ENV:{}:-fallback}}", name)).unwrap(),
            "fallback"
        );
    }
}
