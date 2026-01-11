export type ApiError = {
  status: number
  message: string
}

export function getAdminToken() {
  return localStorage.getItem('ADMIN_TOKEN') || ''
}

export async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const baseUrl = (import.meta.env.VITE_SMS_ADMIN_BASE_URL as string | undefined) || ''
  const url = baseUrl ? `${baseUrl}${path}` : path

  const token = getAdminToken()
  const headers = new Headers(init?.headers)
  if (token) headers.set('Authorization', `Bearer ${token}`)

  const resp = await fetch(url, { ...init, headers })
  if (!resp.ok) {
    const text = await resp.text().catch(() => '')
    throw { status: resp.status, message: text || `HTTP ${resp.status}` } satisfies ApiError
  }
  return (await resp.json()) as T
}

