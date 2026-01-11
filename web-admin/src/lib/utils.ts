import { clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: Array<unknown>) {
  return twMerge(clsx(inputs))
}

