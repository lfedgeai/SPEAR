import { fetchJson } from '@/api/client'
import type { Stats } from '@/api/types'

/**
 * Get dashboard statistics.
 * 获取仪表盘统计数据。
 */
export function getStats() {
  return fetchJson<Stats>('/admin/api/stats')
}
