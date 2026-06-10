import { useNavigationProtection } from './navigation-protection'
interface OneTimeSecretGuardProps {
  active: boolean
  title: string
  description: string
  onConfirmLeave?: () => boolean | Promise<boolean>
}
/** One shared blocker protects navigation, browser unload and account logout. */
export function OneTimeSecretGuard({ active, ...protection }: OneTimeSecretGuardProps) {
  useNavigationProtection(active, protection)
  return null
}
