import { clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: Array<unknown>) {
  return twMerge(clsx(inputs))
}

export function normalizeEndpointName(input: string, options?: { maxLen?: number }) {
  const maxLen = options?.maxLen ?? 64
  const s = input
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^[-_]+|[-_]+$/g, '')

  return s.slice(0, maxLen).replace(/^[-_]+|[-_]+$/g, '')
}

export function isValidEndpointName(input: string, options?: { maxLen?: number }) {
  const maxLen = options?.maxLen ?? 64
  const s = input.trim()
  if (!s) return false
  if (s.length > maxLen) return false
  return /^[A-Za-z0-9_-]+$/.test(s)
}
