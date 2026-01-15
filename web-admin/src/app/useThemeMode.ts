import { useEffect, useState } from 'react'

/**
 * Persisted theme mode hook.
 * 持久化主题模式的 Hook。
 */
export function useThemeMode() {
  const [mode, setMode] = useState<'light' | 'dark'>(() => {
    const v = localStorage.getItem('ADMIN_THEME')
    return v === 'dark' ? 'dark' : 'light'
  })

  useEffect(() => {
    const root = document.documentElement
    if (mode === 'dark') root.classList.add('dark')
    else root.classList.remove('dark')
    localStorage.setItem('ADMIN_THEME', mode)
  }, [mode])

  return { mode, setMode }
}
