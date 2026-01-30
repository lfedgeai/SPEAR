import { fetchJson } from '@/api/client'

export type McpServer = {
  /** Unique server id / 唯一 server id */
  server_id: string
  /** Display name / 展示名称 */
  display_name: string
  /** Transport kind (numeric enum from backend) / 传输类型（后端数值枚举） */
  transport: number
  stdio?: {
    /** Command / 命令 */
    command: string
    /** Command args / 命令参数 */
    args: string[]
    /** Environment variables / 环境变量 */
    env: Record<string, string>
    /** Working directory / 工作目录 */
    cwd: string
  } | null
  http?: {
    /** Server URL / 服务 URL */
    url: string
    /** Extra headers / 额外请求头 */
    headers: Record<string, string>
    /** Auth reference id / 鉴权引用 id */
    auth_ref: string
  } | null
  /** Tool namespace / 工具命名空间 */
  tool_namespace: string
  /** Allowed tool patterns / 允许的工具模式 */
  allowed_tools: string[]
  approval_policy?: {
    /** Default policy / 默认策略 */
    default_policy: string
    /** Per-tool override policy / 按工具覆盖策略 */
    per_tool: Record<string, string>
  } | null
  budgets?: {
    /** Tool timeout / 工具超时 */
    tool_timeout_ms: number
    /** Max concurrency / 最大并发 */
    max_concurrency: number
    /** Max tool output bytes / 工具输出字节上限 */
    max_tool_output_bytes: number
  } | null
  /** Updated time (epoch ms) / 更新时间（毫秒） */
  updated_at_ms: number
}

export type ListMcpServersResponse = {
  /** Whether request succeeded / 请求是否成功 */
  success: boolean
  /** Registry revision (optional) / 注册表版本（可选） */
  revision?: number
  /** Servers list (optional) / server 列表（可选） */
  servers?: McpServer[]
  /** Human readable message (optional) / 人类可读信息（可选） */
  message?: string
}

export type GetMcpServerResponse = {
  success: boolean
  message?: string
  found?: boolean
  server?: McpServer
}

/**
 * List MCP servers.
 * 列出 MCP servers。
 */
export function listMcpServers() {
  return fetchJson<ListMcpServersResponse>('/admin/api/mcp/servers')
}

export function getMcpServer(serverId: string) {
  return fetchJson<GetMcpServerResponse>(
    `/admin/api/mcp/servers/${encodeURIComponent(serverId)}`,
  )
}

export type UpsertMcpServerPayload = {
  /** Unique server id / 唯一 server id */
  server_id: string
  /** Display name (optional) / 展示名称（可选） */
  display_name?: string
  /** Transport type / 传输类型 */
  transport: 'stdio' | 'streamable_http'
  stdio?: {
    /** Command / 命令 */
    command: string
    /** Args (optional) / 参数（可选） */
    args?: string[]
    /** Env (optional) / 环境变量（可选） */
    env?: Record<string, string>
    /** Working directory (optional) / 工作目录（可选） */
    cwd?: string
  }
  http?: {
    /** URL / URL */
    url: string
    /** Headers (optional) / 请求头（可选） */
    headers?: Record<string, string>
    /** Auth ref (optional) / 鉴权引用（可选） */
    auth_ref?: string
  }
  /** Tool namespace (optional) / 工具命名空间（可选） */
  tool_namespace?: string
  /** Allowed tools (optional) / 允许工具（可选） */
  allowed_tools?: string[]
  budgets?: {
    /** Tool timeout (optional) / 工具超时（可选） */
    tool_timeout_ms?: number
    /** Max concurrency (optional) / 最大并发（可选） */
    max_concurrency?: number
    /** Max tool output bytes (optional) / 工具输出字节上限（可选） */
    max_tool_output_bytes?: number
  }
  approval_policy?: {
    /** Default policy (optional) / 默认策略（可选） */
    default_policy?: string
    /** Per-tool policy (optional) / 按工具策略（可选） */
    per_tool?: Record<string, string>
  }
}

export type MutationResponse = {
  /** Whether request succeeded / 请求是否成功 */
  success: boolean
  /** Registry revision (optional) / 注册表版本（可选） */
  revision?: number
  /** Human readable message (optional) / 人类可读信息（可选） */
  message?: string
}

/**
 * Create or update an MCP server.
 * 创建或更新 MCP server。
 */
export function upsertMcpServer(payload: UpsertMcpServerPayload) {
  return fetchJson<MutationResponse>('/admin/api/mcp/servers', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * Delete an MCP server by server id.
 * 通过 server id 删除 MCP server。
 */
export function deleteMcpServer(serverId: string) {
  return fetchJson<MutationResponse>(`/admin/api/mcp/servers/${encodeURIComponent(serverId)}`,
    { method: 'DELETE' },
  )
}
