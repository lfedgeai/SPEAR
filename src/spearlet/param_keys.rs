pub mod chat {
    pub const MODEL: &str = "model";
    pub const BACKEND: &str = "backend";
    pub const TIMEOUT_MS: &str = "timeout_ms";
    pub const RESPONSE_FORMAT: &str = "response_format";
    pub const MESSAGES: &str = "messages";
    pub const TOOLS: &str = "tools";
    pub const TOOL_ARENA_PTR: &str = "tool_arena_ptr";
    pub const TOOL_ARENA_LEN: &str = "tool_arena_len";
    pub const MAX_TOOL_OUTPUT_BYTES: &str = "max_tool_output_bytes";
    pub const MAX_TOTAL_TOOL_CALLS: &str = "max_total_tool_calls";
    pub const MAX_TOOL_CALLS: &str = "max_tool_calls";
    pub const MAX_ITERATIONS: &str = "max_iterations";

    pub fn is_structural_param_key(key: &str) -> bool {
        matches!(
            key,
            MODEL
                | BACKEND
                | TIMEOUT_MS
                | MESSAGES
                | TOOLS
                | TOOL_ARENA_PTR
                | TOOL_ARENA_LEN
                | MAX_TOOL_OUTPUT_BYTES
                | MAX_TOTAL_TOOL_CALLS
                | MAX_TOOL_CALLS
                | MAX_ITERATIONS
        )
    }
}

pub mod mcp {
    pub mod param {
        pub const PREFIX: &str = "mcp.";
        pub const TASK_PREFIX: &str = "mcp.task_";
        pub const ENABLED: &str = "mcp.enabled";
        pub const SERVER_IDS: &str = "mcp.server_ids";
        pub const TOOL_ALLOWLIST: &str = "mcp.tool_allowlist";
        pub const TOOL_DENYLIST: &str = "mcp.tool_denylist";
        pub const TASK_TOOL_ALLOWLIST: &str = "mcp.task_tool_allowlist";
        pub const TASK_TOOL_DENYLIST: &str = "mcp.task_tool_denylist";
    }

    pub mod task_config {
        pub const ENABLED: &str = "mcp.enabled";
        pub const DEFAULT_SERVER_IDS: &str = "mcp.default_server_ids";
        pub const ALLOWED_SERVER_IDS: &str = "mcp.allowed_server_ids";
        pub const TOOL_ALLOWLIST: &str = "mcp.tool_allowlist";
        pub const TOOL_DENYLIST: &str = "mcp.tool_denylist";
    }

    pub mod tool {
        pub const NAMESPACE_PREFIX_DOT: &str = "mcp.";
        pub const NAMESPACE_PREFIX_DBL_UNDERSCORE: &str = "mcp__";
    }
}

pub mod rtasr {
    pub const TRANSPORT: &str = "transport";
    pub const WS_URL: &str = "ws_url";
    pub const CLIENT_SECRET: &str = "client_secret";
    pub const MODEL: &str = "model";
    pub const MAX_SEND_QUEUE_BYTES: &str = "max_send_queue_bytes";
    pub const MAX_RECV_QUEUE_BYTES: &str = "max_recv_queue_bytes";
}
