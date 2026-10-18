/**
 * Object API adapter for Web Admin.
 * Web Admin 的对象 API 适配层。
 */

import { fetchJson, getAdminToken } from '@/api/client'
import type {
  ListObjectsParams,
  ListObjectsResult,
  ObjectDetail,
} from '@/features/objects/object-types'

function toEpochSeconds(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) {
    if (value > 1_000_000_000_000) return Math.floor(value / 1000)
    return Math.floor(value)
  }
  if (typeof value === 'string') {
    const parsed = Date.parse(value)
    if (!Number.isNaN(parsed)) return Math.floor(parsed / 1000)
  }
  return undefined
}

function asStringRecord(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return undefined
  return value as Record<string, unknown>
}

function fromMeta(key: string, meta: Record<string, unknown>): ObjectDetail {
  const metadata =
    asStringRecord(meta.metadata) ??
    asStringRecord(meta.custom_metadata) ??
    asStringRecord(meta.tags)

  return {
    key,
    name:
      typeof meta.name === 'string'
        ? meta.name
        : typeof meta.file_name === 'string'
          ? meta.file_name
          : undefined,
    size:
      typeof meta.size === 'number'
        ? meta.size
        : typeof meta.len === 'number'
          ? meta.len
          : 0,
    checksum:
      typeof meta.checksum === 'string'
        ? meta.checksum
        : typeof meta.checksum_sha256 === 'string'
          ? meta.checksum_sha256
          : undefined,
    refCount: typeof meta.ref_count === 'number' ? meta.ref_count : 1,
    pinned: typeof meta.pinned === 'boolean' ? meta.pinned : false,
    state: meta.state === 'deleting' ? 'deleting' : 'ready',
    createdAt:
      toEpochSeconds(meta.created_at) ??
      toEpochSeconds(meta.registered_at) ??
      toEpochSeconds(meta.uploaded_at),
    updatedAt:
      toEpochSeconds(meta.updated_at) ??
      toEpochSeconds(meta.modified_at) ??
      toEpochSeconds(meta.last_modified),
    metadata,
    raw: meta,
  }
}

function parsePageToken(token?: string) {
  if (!token) return undefined
  const parsed = Number(token)
  return Number.isFinite(parsed) && parsed >= 0 ? String(parsed) : undefined
}

export async function listObjects(params?: ListObjectsParams): Promise<ListObjectsResult> {
  const url = new URL('/admin/api/objects', window.location.origin)
  if (params?.prefix) url.searchParams.set('prefix', params.prefix)
  if (params?.limit) url.searchParams.set('limit', String(params.limit))
  const pageToken = parsePageToken(params?.pageToken)
  if (pageToken) url.searchParams.set('page_token', pageToken)
  const res = await fetchJson<{
    items: Array<Record<string, unknown>>
    next_page_token?: string
    total_count?: number
  }>(url.pathname + url.search)
  const items = (res.items ?? []).map((item) => fromMeta(String(item.key ?? ''), item))
  return {
    items,
    totalCount: res.total_count,
    nextPageToken: res.next_page_token,
  }
}

export async function headObject(key: string): Promise<ObjectDetail> {
  const meta = await fetchJson<Record<string, unknown>>(
    `/admin/api/objects/${encodeURIComponent(key)}`,
  )
  return fromMeta(key, meta)
}

export async function putObject(file: File) {
  const token = getAdminToken()
  const headers: Record<string, string> = {}
  if (token) headers.Authorization = `Bearer ${token}`
  if (file.type) headers['content-type'] = file.type
  headers['x-file-name'] = file.name || 'blob'

  const resp = await fetch('/admin/api/objects', {
    method: 'POST',
    headers,
    body: file,
  })
  if (!resp.ok) throw new Error(`Upload failed: HTTP ${resp.status}`)
  const payload = (await resp.json()) as { success?: boolean; message?: string }
  if (payload.success === false) throw new Error(payload.message || 'Upload failed')
  return payload
}

export async function deleteObject(key: string) {
  const token = getAdminToken()
  const headers: Record<string, string> = {}
  if (token) headers.Authorization = `Bearer ${token}`
  const resp = await fetch(`/admin/api/objects/${encodeURIComponent(key)}`, {
    method: 'DELETE',
    headers,
  })
  if (!resp.ok) throw new Error(`Delete failed: HTTP ${resp.status}`)
  const payload = (await resp.json()) as { success?: boolean; message?: string }
  if (payload.success === false) throw new Error(payload.message || 'Delete failed')
  return payload
}

/**
 * Post an object lifecycle action and return the updated summary fields.
 * 发送对象生命周期操作并返回更新后的摘要字段。
 */
async function postObjectAction(actionPath: string) {
  const token = getAdminToken()
  const headers: Record<string, string> = {}
  if (token) headers.Authorization = `Bearer ${token}`
  const resp = await fetch(actionPath, {
    method: 'POST',
    headers,
  })
  if (!resp.ok) throw new Error(`Object action failed: HTTP ${resp.status}`)
  const payload = (await resp.json()) as {
    success?: boolean
    message?: string
    key?: string
    ref_count?: number
    pinned?: boolean
    state?: string
  }
  if (payload.success === false) throw new Error(payload.message || 'Object action failed')
  return payload
}

export async function retainObject(key: string) {
  return postObjectAction(`/admin/api/objects/${encodeURIComponent(key)}/retain`)
}

export async function releaseObject(key: string) {
  return postObjectAction(`/admin/api/objects/${encodeURIComponent(key)}/release`)
}

export async function pinObject(key: string) {
  return postObjectAction(`/admin/api/objects/${encodeURIComponent(key)}/pin`)
}

export async function unpinObject(key: string) {
  return postObjectAction(`/admin/api/objects/${encodeURIComponent(key)}/unpin`)
}

export function getObjectDownloadUrl(key: string) {
  return `/admin/api/objects/${encodeURIComponent(key)}/content`
}

export async function getObject(key: string) {
  const token = getAdminToken()
  const headers: Record<string, string> = {}
  if (token) headers.Authorization = `Bearer ${token}`
  const resp = await fetch(getObjectDownloadUrl(key), { headers })
  if (!resp.ok) throw new Error(`Failed to download object: HTTP ${resp.status}`)
  return resp.blob()
}

export async function getObjectStorageStatus() {
  return fetchJson<{
    backend: string
    healthy: boolean
    message?: string
  }>('/admin/api/object-storage/status')
}

/**
 * Build the canonical object URI used by task payloads.
 * 构造任务载荷使用的标准对象 URI。
 */
export function toObjectUri(key: string) {
  return `object://${key}`
}
