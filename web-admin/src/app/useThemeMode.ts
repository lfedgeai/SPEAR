import { useEffect, useState } from 'react'

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

