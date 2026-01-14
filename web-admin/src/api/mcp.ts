import { fetchJson } from '@/api/client'

export type McpServer = {
  server_id: string
  display_name: string
  transport: number
  stdio?: {
    command: string
    args: string[]
    env: Record<string, string>
    cwd: string
  } | null
  http?: {
    url: string
    headers: Record<string, string>
    auth_ref: string
  } | null
  tool_namespace: string
  allowed_tools: string[]
  approval_policy?: {
    default_policy: string
    per_tool: Record<string, string>
  } | null
  budgets?: {
    tool_timeout_ms: number
    max_concurrency: number
    max_tool_output_bytes: number
  } | null
  updated_at_ms: number
}

export type ListMcpServersResponse = {
  success: boolean
  revision?: number
  servers?: McpServer[]
  message?: string
}

export function listMcpServers() {
  return fetchJson<ListMcpServersResponse>('/admin/api/mcp/servers')
}

export type UpsertMcpServerPayload = {
  server_id: string
  display_name?: string
  transport: 'stdio' | 'streamable_http'
  stdio?: {
    command: string
    args?: string[]
    env?: Record<string, string>
    cwd?: string
  }
  http?: {
    url: string
    headers?: Record<string, string>
    auth_ref?: string
  }
  tool_namespace?: string
  allowed_tools?: string[]
  budgets?: {
    tool_timeout_ms?: number
    max_concurrency?: number
    max_tool_output_bytes?: number
  }
  approval_policy?: {
    default_policy?: string
    per_tool?: Record<string, string>
  }
}

export type MutationResponse = {
  success: boolean
  revision?: number
  message?: string
}

export function upsertMcpServer(payload: UpsertMcpServerPayload) {
  return fetchJson<MutationResponse>('/admin/api/mcp/servers', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function deleteMcpServer(serverId: string) {
  return fetchJson<MutationResponse>(`/admin/api/mcp/servers/${encodeURIComponent(serverId)}`,
    { method: 'DELETE' },
  )
}

