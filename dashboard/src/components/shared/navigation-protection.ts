import { createContext, useContext, useEffect, useId, useRef } from 'react'
export type Protection = { title: string; description: string; onConfirmLeave?: () => boolean | Promise<boolean> }
export const ProtectionContext = createContext<{ register: (id: string, protection: Protection | null) => void; run: (action: () => void) => void }>({ register: () => {}, run: action => action() })
export function useNavigationProtection(active: boolean, protection: Protection) {
  const id = useId()
  const { register } = useContext(ProtectionContext)
  const { title, description, onConfirmLeave } = protection
  const leaveCallback = useRef(onConfirmLeave)
  useEffect(() => { leaveCallback.current = onConfirmLeave }, [onConfirmLeave])
  useEffect(() => {
    register(id, active ? { title, description, onConfirmLeave: () => leaveCallback.current?.() ?? true } : null)
    return () => register(id, null)
  }, [active, id, register, title, description])
}
export function useProtectedAction() { return useContext(ProtectionContext).run }
