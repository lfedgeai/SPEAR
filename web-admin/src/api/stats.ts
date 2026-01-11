import { fetchJson } from '@/api/client'
import type { Stats } from '@/api/types'

export function getStats() {
  return fetchJson<Stats>('/admin/api/stats')
}

