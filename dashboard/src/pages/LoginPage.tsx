import { useState, type ElementType } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore } from '@/stores'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import {
  Boxes,
  Cloud,
  Code2,
  Database,
  Eye,
  Globe,
  KeyRound,
  Layers3,
  Loader2,
  Network,
  Package,
  Plug,
  ShieldCheck,
  Shuffle,
  UserRound,
  Wrench,
  Zap,
} from 'lucide-react'

function ClientItem({
  icon: Icon,
  title,
}: {
  icon: ElementType
  title: string
}) {
  return (
    <div className="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs text-slate-300">
      <Icon className="h-3.5 w-3.5 shrink-0 text-cyan-200" />
      <span className="whitespace-nowrap">{title}</span>
    </div>
  )
}

function ProviderItem({
  label,
  icon: Icon,
  tone,
}: {
  label: string
  icon: ElementType
  tone: string
}) {
  return (
    <div className="flex items-center gap-2 rounded-md border border-white/8 bg-white/[0.045] px-2.5 py-1.5">
      <div className={`flex h-5 w-5 shrink-0 items-center justify-center rounded ${tone}`}>
        <Icon className="h-3 w-3" />
      </div>
      <span className="whitespace-nowrap text-xs text-slate-200">{label}</span>
    </div>
  )
}

function CapabilityCell({
  icon: Icon,
  label,
  tone = 'cyan',
}: {
  icon: ElementType
  label: string
  tone?: 'cyan' | 'blue' | 'amber' | 'emerald' | 'slate'
}) {
  const toneClass = {
    cyan: 'text-cyan-200',
    blue: 'text-blue-200',
    amber: 'text-amber-200',
    emerald: 'text-emerald-200',
    slate: 'text-slate-300',
  }[tone]

  return (
    <div className="flex min-w-0 flex-col items-center justify-center gap-1.5 border-r border-white/10 px-2.5 py-2.5 last:border-r-0">
      <Icon className={`h-3.5 w-3.5 ${toneClass}`} />
      <span className="max-w-full truncate text-center text-[11px] leading-4 text-slate-300">{label}</span>
    </div>
  )
}

function GatewayTopology() {
  return (
    <div className="relative mx-auto h-[292px] w-full max-w-[660px]">
      <svg
        className="pointer-events-none absolute inset-0 h-full w-full"
        viewBox="0 0 730 330"
        fill="none"
        aria-hidden="true"
      >
        <path d="M178 112 C246 112 276 146 331 146" stroke="rgba(34,211,238,0.34)" />
        <path d="M178 154 C252 154 278 164 331 164" stroke="rgba(125,211,252,0.28)" />
        <path d="M178 196 C246 196 276 182 331 182" stroke="rgba(192,132,252,0.26)" />
        <path d="M399 146 C458 146 485 112 552 112" stroke="rgba(251,191,36,0.38)" />
        <path d="M399 164 C462 164 486 154 552 154" stroke="rgba(34,211,238,0.34)" />
        <path d="M399 182 C458 182 485 196 552 196" stroke="rgba(129,140,248,0.30)" />
        <path d="M365 66 L365 122" stroke="rgba(251,191,36,0.22)" />
        <path d="M365 208 L365 260" stroke="rgba(34,211,238,0.22)" />
        <circle cx="244" cy="133" r="3" fill="#67e8f9" />
        <circle cx="258" cy="133" r="3" fill="#93c5fd" />
        <circle cx="470" cy="134" r="3" fill="#fde047" />
        <circle cx="486" cy="134" r="3" fill="#67e8f9" />
        <circle cx="486" cy="198" r="3" fill="#c4b5fd" />
        <circle cx="470" cy="198" r="3" fill="#67e8f9" />
      </svg>

      <div className="absolute left-0 top-1/2 w-[152px] -translate-y-1/2 rounded-lg border border-cyan-100/15 bg-[#151d28]/90 p-2.5 shadow-[0_18px_50px_rgba(0,0,0,0.22)]">
        <div className="mb-1.5 flex items-center gap-2 px-2 text-xs font-semibold text-cyan-100">
          <Network className="h-3.5 w-3.5" />
          客户端入口
        </div>
        <div className="space-y-0.5">
          <ClientItem icon={Plug} title="IDE / Plugins" />
          <ClientItem icon={Globe} title="Web / Mobile" />
          <ClientItem icon={Package} title="第三方应用" />
          <ClientItem icon={Code2} title="自定义客户端" />
        </div>
      </div>

      <div className="absolute left-1/2 top-1/2 flex h-28 w-28 -translate-x-1/2 -translate-y-1/2 rotate-45 items-center justify-center rounded-2xl border border-cyan-200/60 bg-[#10212c] shadow-[0_0_42px_rgba(34,211,238,0.24)]">
        <div className="-rotate-45 text-center">
          <div className="mx-auto flex h-9 w-9 items-center justify-center rounded-lg bg-cyan-300/10 text-cyan-100">
            <Zap className="h-5 w-5" />
          </div>
          <p className="mt-2 text-sm font-semibold text-white">ModelPort</p>
          <p className="mt-0.5 text-[11px] text-cyan-200">网关</p>
        </div>
      </div>

      <div className="absolute left-1/2 top-5 -translate-x-1/2 rounded-md border border-amber-200/20 bg-amber-200/10 px-2.5 py-1.5 text-[11px] text-amber-100">
        策略校验
      </div>
      <div className="absolute bottom-5 left-1/2 -translate-x-1/2 rounded-md border border-white/10 bg-[#151a24]/95 px-3 py-1.5 text-[11px] text-slate-300 shadow-[0_12px_28px_rgba(0,0,0,0.2)]">
        路由 · 负载 · 限流
      </div>

      <div className="absolute right-0 top-1/2 w-[152px] -translate-y-1/2 rounded-lg border border-cyan-100/15 bg-[#151d28]/90 p-2.5 shadow-[0_18px_50px_rgba(0,0,0,0.22)]">
        <div className="mb-2 flex items-center gap-2 px-1 text-xs font-semibold text-cyan-100">
          <Cloud className="h-3.5 w-3.5" />
          上游渠道
        </div>
        <div className="space-y-1.5">
          <ProviderItem label="OpenAI" icon={Zap} tone="bg-emerald-300/10 text-emerald-200" />
          <ProviderItem label="Anthropic" icon={Boxes} tone="bg-violet-300/10 text-violet-200" />
          <ProviderItem label="Gemini" icon={Globe} tone="bg-blue-300/10 text-blue-200" />
          <ProviderItem label="本地模型" icon={Database} tone="bg-cyan-300/10 text-cyan-200" />
          <ProviderItem label="自定义渠道" icon={Layers3} tone="bg-amber-300/10 text-amber-200" />
        </div>
      </div>
    </div>
  )
}

export function LoginPage() {
  const [username, setUsername] = useState('admin')
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const login = useAuthStore((s) => s.login)
  const navigate = useNavigate()

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!username.trim() || !password) {
      setError('请输入用户名和密码')
      return
    }

    setLoading(true)
    setError('')

    try {
      await login(username.trim(), password)
      navigate('/dashboard')
    } catch {
      setError('登录失败，请检查用户名或密码')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-[#f6f8f9] bg-[linear-gradient(to_right,rgba(15,23,42,0.028)_1px,transparent_1px),linear-gradient(to_bottom,rgba(15,23,42,0.022)_1px,transparent_1px)] [background-size:56px_56px] px-5 py-8 sm:px-8">
      <Card className="grid h-auto min-h-[610px] w-full max-w-[1160px] overflow-hidden border-slate-200 bg-white shadow-[0_28px_86px_rgba(15,23,42,0.13)] md:grid-cols-[1.34fr_0.88fr]">
        <div className="relative hidden min-h-[610px] flex-col justify-between overflow-hidden bg-[#0d141d] p-8 text-white lg:p-9 md:flex">
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_48%_45%,rgba(34,211,238,0.11),transparent_33%),linear-gradient(to_right,rgba(148,163,184,0.07)_1px,transparent_1px),linear-gradient(to_bottom,rgba(148,163,184,0.055)_1px,transparent_1px)] [background-size:auto,38px_38px,38px_38px]" />
          <div className="absolute inset-x-9 top-28 h-px bg-cyan-200/15" />
          <div className="absolute bottom-32 left-9 right-9 h-px bg-amber-200/12" />

          <div className="relative">
            <div className="flex items-start justify-between gap-6">
              <div className="flex items-center gap-3">
                <div className="flex h-12 w-12 items-center justify-center rounded-lg border border-white/10 bg-white/[0.06] text-cyan-100">
                  <Zap className="h-6 w-6" />
                </div>
                <div>
                  <p className="text-lg font-semibold text-white">ModelPort</p>
                  <p className="text-sm text-slate-400">Admin Console</p>
                </div>
              </div>
              <div className="rounded-md border border-emerald-300/15 bg-emerald-300/10 px-2.5 py-1.5 text-xs font-medium text-emerald-100">
                本地部署
              </div>
            </div>
          </div>

          <div className="relative">
            <GatewayTopology />
          </div>

          <div className="relative space-y-3">
            <div className="grid w-full grid-cols-6 overflow-hidden rounded-lg border border-white/10 bg-white/[0.045] shadow-[inset_0_1px_0_rgba(255,255,255,0.07)]">
              <CapabilityCell icon={Code2} label="Anthropic API" />
              <CapabilityCell icon={Boxes} label="OpenAI API" tone="blue" />
              <CapabilityCell icon={Wrench} label="Tool Use" tone="cyan" />
              <CapabilityCell icon={Eye} label="Trace" tone="emerald" />
              <CapabilityCell icon={Shuffle} label="Fallback" tone="amber" />
              <CapabilityCell icon={Database} label="Local" tone="cyan" />
            </div>
            <div className="flex flex-wrap items-center justify-between gap-2 border-t border-white/10 pt-3 text-xs leading-5">
              <span className="font-medium text-slate-200">本地模型路由网关</span>
              <span className="text-slate-400">鉴权 · 协议适配 · Tool Use · Fallback · 日志</span>
            </div>
          </div>
        </div>

        <div className="flex min-h-[610px] items-center justify-center bg-[#fdfefe] px-7 py-10 sm:px-10">
          <div className="w-full max-w-[340px]">
            <CardHeader className="items-center px-0 pb-8 text-center">
              <div className="mb-3 flex h-8 w-8 items-center justify-center rounded-lg text-cyan-600">
                <Zap className="h-5 w-5" />
              </div>
              <div className="space-y-1.5">
                <p className="text-xl font-semibold text-slate-950">ModelPort</p>
                <p className="text-sm text-slate-500">Admin Console</p>
              </div>
            </CardHeader>

            <CardContent className="px-0">
              <form onSubmit={handleSubmit} className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="username" className="text-xs font-medium text-slate-700">用户名</Label>
                  <div className="relative">
                    <UserRound className="absolute left-3.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400" />
                    <Input
                      id="username"
                      type="text"
                      placeholder="请输入用户名"
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                      disabled={loading}
                      autoComplete="username"
                      className="h-11 rounded-md border-slate-200 bg-white pl-9 pr-4 text-sm shadow-sm focus-visible:border-cyan-600 focus-visible:ring-cyan-600"
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="password" className="text-xs font-medium text-slate-700">密码</Label>
                  <div className="relative">
                    <KeyRound className="absolute left-3.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400" />
                    <Input
                      id="password"
                      type="password"
                      placeholder="请输入密码"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      disabled={loading}
                      autoComplete="current-password"
                      className="h-11 rounded-md border-slate-200 bg-white pl-9 pr-4 text-sm shadow-sm focus-visible:border-cyan-600 focus-visible:ring-cyan-600"
                    />
                  </div>
                  {error && (
                    <p className="rounded-lg border border-destructive/20 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                      {error}
                    </p>
                  )}
                </div>
                <Button
                  type="submit"
                  size="lg"
                  className="mt-2 h-11 w-full rounded-md bg-[#0891b2] text-sm font-medium text-white shadow-[0_16px_34px_rgba(8,145,178,0.24)] hover:bg-[#0e7490]"
                  disabled={loading}
                >
                  {loading && <Loader2 className="h-4 w-4 animate-spin" />}
                  登录
                </Button>
                <div className="flex items-center gap-3 pt-3 text-xs text-slate-500">
                  <div className="h-px flex-1 bg-slate-200" />
                  <ShieldCheck className="h-4 w-4 text-slate-400" />
                  <div className="h-px flex-1 bg-slate-200" />
                </div>
                <p className="text-center text-xs text-slate-500">本地部署 · 仅管理员访问 · 安全可靠</p>
              </form>
            </CardContent>
          </div>
        </div>
      </Card>
    </div>
  )
}
