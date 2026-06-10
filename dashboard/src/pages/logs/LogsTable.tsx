import { useRef } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { RequestLog } from '@/types'
import { PaginationBar } from '@/components/shared/PaginationBar'
import { formatLatency } from '@/lib/utils'
import { formatInteger, parseLogDate } from './log-utils'

export function LogsTable({ logs, total, page, pageSize, totalPages, start, end, pageSizeOptions, onPageChange, onPageSizeChange, onSelectLog, selectedId }: {
  logs: RequestLog[]; total: number; page: number; pageSize: number; totalPages: number; start: number; end: number;
  pageSizeOptions: number[]; isLoading: boolean; onPageChange: (page: number) => void; onPageSizeChange: (size: number) => void;
  onSelectLog: (log: RequestLog) => void; selectedId?: string;
}) {
  const parent = useRef<HTMLDivElement>(null)
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({ count: logs.length, getScrollElement: () => parent.current, estimateSize: () => 112, overscan: 6, getItemKey: index => logs[index].id })
  return <section className="border bg-card overflow-hidden" aria-label="请求列表">
    <div className="flex items-center justify-between border-b px-4 py-3"><h2 className="text-sm font-semibold">请求队列</h2><span className="text-xs text-muted-foreground">{formatInteger(total)} 条 · 结果 / 路由 / 耗时</span></div>
    {logs.length === 0 ? <div className="px-5 py-20 text-center"><p className="font-medium">没有匹配的请求日志</p><p className="mt-2 text-sm text-muted-foreground">调整筛选条件，或先发送一条模型请求。</p></div>
    : <div ref={parent} data-testid="logs-scroll-viewport" className="overflow-auto" style={{ height: 'min(65vh, 740px)', minHeight: 340 }}>
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>{virtualizer.getVirtualItems().map(item => {
        const log = logs[item.index]
        return <div key={item.key} data-index={item.index} ref={virtualizer.measureElement} style={{ position: 'absolute', top: 0, left: 0, width: '100%', transform: `translateY(${item.start}px)` }}>
          <button data-log-id={log.id} type="button" className={`request-queue-row ${selectedId === log.id ? 'selected' : ''}`} aria-pressed={selectedId === log.id} aria-label={`查看 ${log.resolvedModel || log.model} 请求详情`} onClick={() => onSelectLog(log)}>
            <span className={`rounded px-1.5 py-1 text-xs ${log.status === 'success' ? 'bg-success-soft text-success-text' : 'bg-destructive-soft text-destructive-text'}`}>{log.statusCode}</span>
            <span className="min-w-0 flex-1"><strong className="block break-words font-mono text-xs">{log.resolvedModel || log.model}</strong><span className="mt-1 block text-xs text-muted-foreground">{log.provider} · {log.username}</span><span className="mt-1 block text-[10px] text-muted-foreground">{parseLogDate(log.timestamp)?.toLocaleString('zh-CN', { hour12: false }) || log.timestamp} · {log.stream}</span>{log.errorMessage && <span className="mt-1 block break-words text-xs text-destructive-text">{log.errorMessage}</span>}</span>
            <span className="shrink-0 text-right text-xs tabular-nums">{formatLatency(log.latencyMs)}<small className="mt-1 block text-[10px] text-muted-foreground">{log.toolUseRequested ? 'Tool Use' : log.status}</small></span>
          </button>
        </div>
      })}</div>
    </div>}
    <div className="border-t p-3"><PaginationBar total={total} page={page} pageSize={pageSize} totalPages={totalPages} start={start} end={end} totalLabel="条请求" pageSizeOptions={pageSizeOptions} onPageChange={onPageChange} onPageSizeChange={onPageSizeChange} /></div>
  </section>
}
