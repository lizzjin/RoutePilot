import { useEffect, useState } from 'react'
import { useLogs } from '@/hooks'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { Clock3, Radio } from 'lucide-react'
import type { LogFilters, RequestLog } from '@/types'
import { formatInteger } from './logs/log-utils'
import { LogsSummaryGrid } from './logs/LogsSummary'
import { LogsFilters } from './logs/LogsFilters'
import { LogsTable } from './logs/LogsTable'
import { LogsDrawer } from './logs/LogsDrawer'

const PAGE_SIZE = 200

export function LogsPage() {
  const [filters, setFilters] = useState<LogFilters>({})
  const [liveMode, setLiveMode] = useState(false)
  const [selectedLog, setSelectedLog] = useState<RequestLog | null>(null)

  const { data, isLoading, refetch } = useLogs(filters, 1, PAGE_SIZE)
  const logs = data?.logs || []
  const total = data?.total || 0
  const summary = data?.summary

  // Live mode: refetch every 3 seconds
  useEffect(() => {
    if (!liveMode) return
    const id = setInterval(() => refetch(), 3000)
    return () => clearInterval(id)
  }, [liveMode, refetch])

  const handleFiltersChange = (next: LogFilters) => {
    setFilters(next)
    setSelectedLog(null)
  }

  if (isLoading) {
    return (
      <div className="flex h-64 items-center justify-center text-muted-foreground">
        加载中...
      </div>
    )
  }

  return (
    <div className="space-y-5">
      {/* Page header */}
      <section>
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0 space-y-1">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="outline" className="gap-1.5 border-emerald-200 bg-emerald-50 text-emerald-700">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                实时聚合
              </Badge>
              <Badge variant="outline" className="border-slate-200 bg-slate-50 text-slate-700">
                {formatInteger(total)} 条结果
              </Badge>
            </div>
            <h2 className="text-2xl font-bold tracking-tight">请求日志</h2>
            <p className="max-w-3xl text-sm text-muted-foreground">
              路由、身份、缓存、计费和延迟的请求明细。
            </p>
          </div>

          <div className="flex items-center gap-3">
            {/* Live mode toggle */}
            <Button
              variant={liveMode ? 'default' : 'outline'}
              size="sm"
              className={cn(
                'gap-1.5 transition-all',
                liveMode && 'bg-emerald-600 hover:bg-emerald-700 text-white',
              )}
              onClick={() => setLiveMode((prev) => !prev)}
            >
              <Radio className={cn('h-3.5 w-3.5', liveMode && 'animate-pulse')} />
              实时
            </Button>

            <div className="flex items-center gap-2 rounded-lg border bg-background px-3 py-2 text-sm shadow-sm">
              <Clock3 className="h-4 w-4 text-muted-foreground" />
              <span className="text-muted-foreground">每页</span>
              <span className="font-mono font-semibold">{PAGE_SIZE}</span>
            </div>
          </div>
        </div>
      </section>

      {/* Summary cards */}
      <LogsSummaryGrid summary={summary} />

      {/* Filters */}
      <LogsFilters
        filters={filters}
        onFiltersChange={handleFiltersChange}
        logs={logs}
      />

      {/* Table */}
      <LogsTable
        logs={logs}
        total={total}
        isLoading={isLoading}
        onSelectLog={setSelectedLog}
      />

      {/* Detail drawer */}
      <LogsDrawer
        log={selectedLog}
        onClose={() => setSelectedLog(null)}
      />
    </div>
  )
}
