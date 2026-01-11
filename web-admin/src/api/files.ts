import { fetchJson, getAdminToken } from '@/api/client'
import type { FileItem } from '@/api/types'

export type ListFilesParams = {
  q?: string
  limit?: number
  offset?: number
}

export async function listFiles(params?: ListFilesParams) {
  const url = new URL('/admin/api/files', window.location.origin)
  if (params?.q) url.searchParams.set('q', params.q)
  if (params?.limit) url.searchParams.set('limit', String(params.limit))
  if (params?.offset) url.searchParams.set('offset', String(params.offset))
  return fetchJson<{ files: FileItem[]; total_count?: number }>(
    url.pathname + url.search,
  )
}

export function getFileMeta(id: string) {
  return fetchJson<Record<string, unknown>>(
    `/admin/api/files/${encodeURIComponent(id)}/meta`,
  )
}

export async function uploadFile(file: File) {
  const presign = await fetchJson<{ upload_url: string; method?: string }>(
    '/admin/api/files/presign-upload',
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        content_type: file.type || 'application/octet-stream',
        max_size_bytes: file.size || undefined,
      }),
    },
  )

  const token = getAdminToken()
  const headers: Record<string, string> = {}
  if (token) headers.Authorization = `Bearer ${token}`
  if (file.type) headers['content-type'] = file.type
  headers['x-file-name'] = file.name || 'blob'

  const url = presign.upload_url.startsWith('/admin')
    ? presign.upload_url
    : '/admin/api/files'
  const method = presign.method || 'POST'
  const resp = await fetch(url, { method, headers, body: file })
  if (!resp.ok) throw new Error(`Upload failed: HTTP ${resp.status}`)
  const j = (await resp.json()) as { success: boolean; id?: string; message?: string }
  if (!j.success) throw new Error(j.message || 'Upload failed')
  return j
}

export async function deleteFile(id: string) {
  const token = getAdminToken()
  const headers: Record<string, string> = {}
  if (token) headers.Authorization = `Bearer ${token}`
  const resp = await fetch(`/admin/api/files/${encodeURIComponent(id)}`, {
    method: 'DELETE',
    headers,
  })
  if (!resp.ok) throw new Error(`Delete failed: HTTP ${resp.status}`)
  const j = (await resp.json()) as { success: boolean; message?: string }
  if (!j.success) throw new Error(j.message || 'Delete failed')
  return j
}
