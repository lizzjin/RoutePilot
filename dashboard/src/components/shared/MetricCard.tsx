import { cn } from '@/lib/utils'
import { Card, CardContent } from '@/components/ui/card'
import { Skeleton } from './Skeleton'
import { Sparkline } from './Sparkline'
import { AnimatedNumber } from './AnimatedNumber'
import { type LucideIcon } from 'lucide-react'

interface MetricCardProps {
  title: string
  value: string | number
  description?: string
  icon?: LucideIcon
  trend?: { value: number; label: string }
  sparkline?: number[]
  loading?: boolean
  className?: string
  compact?: boolean
}

export function MetricCard({
  title,
  value,
  description,
  icon: Icon,
  trend,
  sparkline,
  loading,
  className,
  compact = false,
}: MetricCardProps) {
  if (loading) {
    return (
      <Card className={cn('border-0 rounded-none shadow-none', className)}>
        <CardContent className={compact ? 'p-4' : 'p-4 sm:p-5'}>
          <div className="flex items-start gap-4">
            <Skeleton className="h-10 w-10 shrink-0 rounded-lg" />
            <div className="min-w-0 flex-1 space-y-2">
              <Skeleton className="h-4 w-24" />
              <Skeleton className="h-8 w-28" />
              <Skeleton className="h-3 w-36" />
            </div>
          </div>
        </CardContent>
      </Card>
    )
  }

  const numericValue = typeof value === 'number' ? value : undefined

  return (
    <Card
      className={cn(
        'border-0 rounded-none shadow-none',
        className,
  compact = false,
      )}
    >
      <CardContent className={compact ? 'p-4' : 'p-4 sm:p-5'}>
        <div className="relative flex items-start gap-2.5 sm:gap-4">
          {Icon && (
            <div className={cn('flex shrink-0 items-center justify-center text-muted-foreground', compact ? 'absolute right-0 top-0 h-4 w-4' : 'h-9 w-9 sm:h-10 sm:w-10')}>
              <Icon className="h-4 w-4 sm:h-5 sm:w-5" />
            </div>
          )}

          <div className="min-w-0 flex-1">
            <div className="flex items-end justify-between gap-3">
              <div className="min-w-0">
                <p className={cn('text-xs font-medium text-muted-foreground sm:text-sm', compact && 'pr-6')}>{title}</p>
                <div className="numeric mt-2 break-words text-2xl font-semibold leading-tight tracking-[-0.025em] sm:mt-3 sm:text-2xl">
                  {numericValue !== undefined ? (
                    <AnimatedNumber value={numericValue} />
                  ) : (
                    value
                  )}
                </div>
              </div>
              {sparkline && sparkline.length > 0 && (
                <Sparkline data={sparkline} width={52} height={28} className="hidden shrink-0 opacity-70 2xl:block" />
              )}
            </div>

            {(description || trend) && (
              <p className="mt-2 flex items-start gap-1.5 text-xs leading-relaxed text-muted-foreground">
                {trend && (
                  <span
                    className={cn(
                      'inline-flex items-center rounded-full px-1.5 py-0.5 text-xs font-semibold',
                      trend.value >= 0
                        ? 'bg-success-soft text-success-text'
                        : 'bg-destructive-soft text-destructive-text',
                    )}
                  >
                    {trend.value >= 0 ? '↑' : '↓'} {Math.abs(trend.value)}%
                  </span>
                )}
                <span className="min-w-0 break-words">{trend?.label || description}</span>
              </p>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
