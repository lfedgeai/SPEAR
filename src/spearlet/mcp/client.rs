use rmcp::{
    model::CallToolRequestParam,
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::process::Command;
use std::process::Stdio;

use crate::proto::sms::{McpServerRecord, McpTransport};

pub async fn list_tools(record: &McpServerRecord) -> Result<Vec<serde_json::Value>, String> {
    let transport = record.transport;
    if transport == McpTransport::Stdio as i32 {
        let stdio = record
            .stdio
            .as_ref()
            .ok_or_else(|| "missing stdio config".to_string())?;
        let cmd = Command::new(&stdio.command).configure(|c| {
            for a in stdio.args.iter() {
                c.arg(a);
            }
            if !stdio.cwd.is_empty() {
                c.current_dir(&stdio.cwd);
            }
            for (k, v) in stdio.env.iter() {
                c.env(k, v);
            }
            c.stderr(Stdio::null());
        });

        let peer = ().serve(
            TokioChildProcess::new(cmd)
                .map_err(|e| format!("spawn failed: {}", e))?,
        )
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
        let cmd = Command::new(&stdio.command).configure(|c| {
            for a in stdio.args.iter() {
                c.arg(a);
            }
            if !stdio.cwd.is_empty() {
                c.current_dir(&stdio.cwd);
            }
            for (k, v) in stdio.env.iter() {
                c.env(k, v);
            }
            c.stderr(Stdio::null());
        });

        let peer = ().serve(
            TokioChildProcess::new(cmd)
                .map_err(|e| format!("spawn failed: {}", e))?,
        )
        .await
        .map_err(|e| format!("connect failed: {}", e))?;

        let args_val = serde_json::from_str::<serde_json::Value>(args_json)
            .unwrap_or(serde_json::Value::Null);
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
