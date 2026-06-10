import type { LucideIcon } from 'lucide-react'
import type { LogSummary } from '@/types'
import { formatLatency } from '@/lib/utils'
import { MetricCard } from '@/components/shared/MetricCard'
import { Activity, BadgeDollarSign, Clock3, DatabaseZap, Gauge, Wrench } from 'lucide-react'
import { formatInteger, formatMoney, formatPercent } from './log-utils'

// ── Summary metric card ──────────────────────────────────────────

export function SummaryMetric({
  label,
  value,
  helper,
  icon: Icon,
}: {
  label: string
  value: string
  helper: string
  icon: LucideIcon
}) {
  return <MetricCard compact title={label} value={value} description={helper} icon={Icon} />
}

// ── Summary cards grid ───────────────────────────────────────────

export function LogsSummaryGrid({
  summary,
}: {
  summary?: LogSummary
}) {
  const totalRequests = summary?.totalRequests || 0
  const successRequests = summary?.successRequests || 0
  const successRate = totalRequests > 0 ? (successRequests / totalRequests) * 100 : 0
  const cacheTokens = (summary?.totalCacheWriteTokens || 0) + (summary?.totalCacheReadTokens || 0)
  const toolUseRequests = summary?.toolUseRequests || 0
  const toolUseSuccessRequests = summary?.toolUseSuccessRequests || 0
  const toolUseSuccessRate = toolUseRequests > 0 ? (toolUseSuccessRequests / toolUseRequests) * 100 : 0
  const latencySampleCount = summary?.latencySampleCount || 0
  const firstByteLatencySampleCount = summary?.firstByteLatencySampleCount || 0

  return (
    <div className="metric-grid grid grid-cols-2 gap-px overflow-hidden rounded-md border bg-border-soft sm:grid-cols-3 xl:grid-cols-6">
      <SummaryMetric
        label="可计费费用"
        value={formatMoney(summary?.totalBillableCost || 0, 4)}
        helper={`${formatInteger(summary?.billableRequests || 0)} 已核算 · ${formatInteger(summary?.estimateOnlyRequests || 0)} 仅预估 · 预估 ${formatMoney(summary?.totalCostEstimate || 0, 4)}`}
        icon={BadgeDollarSign}
      />
      <SummaryMetric
        label="成功率"
        value={formatPercent(successRate)}
        helper={`${formatInteger(successRequests)} 成功 / ${formatInteger(totalRequests)} 总计`}
        icon={Activity}
      />
      <SummaryMetric
        label="Token"
        value={formatInteger(summary?.totalTokens || 0)}
        helper={`TPM ${formatInteger(summary?.tpm || 0)} · RPM ${(summary?.rpm || 0).toFixed(2)}`}
        icon={Gauge}
      />
      <SummaryMetric
        label="Tool Use"
        value={toolUseRequests > 0 ? formatPercent(toolUseSuccessRate) : '—'}
        helper={`${formatInteger(toolUseSuccessRequests)} 成功 / ${formatInteger(toolUseRequests)} 工作流`}
        icon={Wrench}
      />
      <SummaryMetric
        label="P95 延迟"
        value={latencySampleCount > 0 ? formatLatency(summary?.latencyP95Ms || 0) : '—'}
        helper={firstByteLatencySampleCount > 0
          ? `TTFT P95 ${formatLatency(summary?.firstByteLatencyP95Ms || 0)} · ${formatInteger(firstByteLatencySampleCount)} 个流`
          : `${formatInteger(latencySampleCount)} 个完整请求 · 暂无 TTFT`}
        icon={Clock3}
      />
      <SummaryMetric
        label="缓存 Token"
        value={formatInteger(cacheTokens)}
        helper={`读 ${formatInteger(summary?.totalCacheReadTokens || 0)} / 写 ${formatInteger(summary?.totalCacheWriteTokens || 0)}`}
        icon={DatabaseZap}
      />
    </div>
  )
}
