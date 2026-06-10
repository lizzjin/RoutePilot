import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { gatewayProcessStatus } from './sidebar-status'
export function GatewayStatus() {
  const { data, isError } = useQuery({ queryKey: ['gateway-liveness'], queryFn: () => api.get<{ status: string }>('/livez'), refetchInterval: 30000, staleTime: 10000, retry: 1 })
  const status = gatewayProcessStatus(data?.status, isError)
  return <span className="ml-auto text-xs text-muted-foreground" title={status.title}><span className={`mr-2 inline-block h-1.5 w-1.5 rounded-full ${status.kind === 'online' ? 'bg-success' : 'bg-warning'}`} />{status.label}</span>
}
