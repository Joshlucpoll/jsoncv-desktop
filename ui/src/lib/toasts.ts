import { useCallback, useState } from 'react'

export type ToastAction = { label: string; run: () => void }
/** `key` replaces an earlier toast with the same key; `timeout` (ms) auto-dismisses it. */
export type ToastInput = { key?: string; kind?: 'ok' | 'warn' | 'error'; text: string; actions?: ToastAction[]; timeout?: number }
export type Toast = ToastInput & { id: number }

let seq = 0

export function useToasts() {
  const [list, setList] = useState<Toast[]>([])
  const push = useCallback((t: ToastInput) => {
    const id = ++seq
    setList((l) => [...l.filter((x) => !t.key || x.key !== t.key), { ...t, id }])
    return id
  }, [])
  const dismiss = useCallback((key: string | number) => setList((l) => l.filter((x) => x.id !== key && x.key !== key)), [])
  return { list, push, dismiss }
}
