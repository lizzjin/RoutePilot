import { useCallback, useState, type ReactNode } from 'react'
import { useBeforeUnload, useBlocker } from 'react-router-dom'
import { ConfirmDialog } from './ConfirmDialog'
import { ProtectionContext, type Protection } from './navigation-protection'
export function NavigationProtection({ children }: { children: ReactNode }) {
  const [protections, setProtections] = useState<Record<string, Protection>>({})
  const [action, setAction] = useState<(() => void) | null>(null)
  const [pending, setPending] = useState(false)
  const active = Object.values(protections)
  const blocker = useBlocker(active.length > 0)
  const register = useCallback((id: string, protection: Protection | null) => setProtections(current => {
    if (!protection && !(id in current)) return current
    const next = { ...current }
    if (protection) next[id] = protection; else delete next[id]
    return next
  }), [])
  useBeforeUnload(useCallback(event => { if (active.length) { event.preventDefault(); event.returnValue = true } }, [active.length]))
  const run = (next: () => void) => { if (active.length) setAction(() => next); else next() }
  const stay = () => { setAction(null); if (blocker.state === 'blocked') blocker.reset() }
  const leave = async () => {
    if (pending) return
    setPending(true)
    try {
      for (const protection of active) if (protection.onConfirmLeave && !await protection.onConfirmLeave()) return
      if (action) { action(); setAction(null) }
      else if (blocker.state === 'blocked') blocker.proceed()
    } catch {
      // A failed credential handoff must keep the protected page open.
      return
    } finally { setPending(false) }
  }
  return <ProtectionContext.Provider value={{ register, run }}>{children}<ConfirmDialog open={!!action || blocker.state === 'blocked'} title={active[0]?.title || '确认离开？'} description={active.map(item => item.description).join(' ')} confirmLabel="确认离开" cancelLabel="留在此页" destructive pending={pending} onCancel={stay} onConfirm={() => { void leave() }} /></ProtectionContext.Provider>
}
