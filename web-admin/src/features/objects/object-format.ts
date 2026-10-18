/**
 * Object domain formatting helpers.
 * 对象域格式化辅助函数。
 */

import type { ObjectState } from '@/features/objects/object-types'

export function formatObjectSize(bytes?: number) {
  if (!Number.isFinite(bytes) || !bytes || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let index = 0
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024
    index += 1
  }
  return `${value.toFixed(index === 0 ? 0 : 1)} ${units[index]}`
}

export function formatObjectTs(ts?: number) {
  if (!ts) return '-'
  return new Date(ts * 1000).toLocaleString()
}

export function formatObjectState(state: ObjectState) {
  if (state === 'deleting') return 'Deleting'
  return 'Ready'
}
