import { cn } from '@/lib/utils'

interface StatusBadgeProps {
  status: string
  className?: string
}

const statusConfig: Record<string, { label: string; className: string }> = {
  active: { label: '活跃', className: 'border-success-text/30 bg-success-soft text-success-text' },
  disabled: { label: '禁用', className: 'border-border bg-muted text-muted-foreground' },
  suspended: { label: '已暂停', className: 'border-destructive-text/30 bg-destructive-soft text-destructive-text' },
  success: { label: '成功', className: 'border-success-text/30 bg-success-soft text-success-text' },
  error: { label: '错误', className: 'border-destructive-text/30 bg-destructive-soft text-destructive-text' },
  timeout: { label: '超时', className: 'border-warning-text/30 bg-warning-soft text-warning-text' },
  healthy: { label: '健康', className: 'border-success-text/30 bg-success-soft text-success-text' },
  degraded: { label: '降级', className: 'border-warning-text/30 bg-warning-soft text-warning-text' },
  down: { label: '离线', className: 'border-destructive-text/30 bg-destructive-soft text-destructive-text' },
  inactive: { label: '未激活', className: 'border-border bg-muted text-muted-foreground' },
  revoked: { label: '已吊销', className: 'border-destructive-text/30 bg-destructive-soft text-destructive-text' },
  pending_second_approval: { label: '待第二人审批', className: 'border-warning-text/30 bg-warning-soft text-warning-text' },
  approved: { label: '已双人批准', className: 'border-info-text/30 bg-info-soft text-info-text' },
  applied: { label: '已应用', className: 'border-success-text/30 bg-success-soft text-success-text' },
}

export function StatusBadge({ status, className }: StatusBadgeProps) {
  const config = statusConfig[status] || { label: status, className: 'border-border bg-muted/55 text-muted-foreground' }

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-sm border px-2 py-0.5 text-xs font-medium leading-4 before:h-1.5 before:w-1.5 before:shrink-0 before:rounded-full before:bg-current before:opacity-70",
        config.className,
        className
      )}
    >
      {config.label}
    </span>
  )
}
