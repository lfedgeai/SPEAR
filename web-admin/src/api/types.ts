export type Stats = {
  total_count: number
  online_count: number
  offline_count: number
  recent_60s_count: number
}

export type NodeSummary = {
  uuid: string
  name?: string
  ip_address: string
  port: number
  status: string
  last_heartbeat: number
  registered_at: number
  metadata?: Record<string, string>
}

export type NodeResource = {
  cpu_usage_percent?: number
  memory_usage_percent?: number
  disk_usage_percent?: number
  total_memory_bytes?: number
  used_memory_bytes?: number
  available_memory_bytes?: number
}

export type NodeDetail = {
  found: boolean
  node?: NodeSummary
  resource?: NodeResource
}

export type TaskSummary = {
  task_id: string
  name: string
  description?: string
  status: string
  priority: string
  node_uuid: string
  endpoint: string
  version: string
  execution_kind?: string
  executable_type?: string
  executable_uri?: string
  executable_name?: string
  registered_at: number
  last_heartbeat: number
}

export type TaskDetail = {
  found: boolean
  task?: Record<string, unknown>
}

export type FileItem = {
  id: string
  name?: string | null
  len: number
  modified_at: number
}

