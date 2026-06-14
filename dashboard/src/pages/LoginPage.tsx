import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore } from '@/stores'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Loader2, ShieldCheck, Zap } from 'lucide-react'

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
    <div className="flex min-h-screen items-center justify-center bg-[#f4f8f6] px-5 py-8 sm:px-8">
      <Card className="grid w-full max-w-5xl overflow-hidden border-[#d8e4dd] shadow-[0_24px_80px_rgba(24,56,48,0.14)] md:grid-cols-[0.86fr_1fr]">
        <div className="relative hidden min-h-[560px] flex-col justify-between bg-[linear-gradient(145deg,#0f302b_0%,#143f35_52%,#284231_100%)] p-10 text-white md:flex">
          <div className="space-y-8">
            <div className="flex items-center gap-3">
              <div className="flex h-14 w-14 items-center justify-center rounded-xl bg-white/[0.12] ring-1 ring-white/15">
                <Zap className="h-7 w-7 text-[#9cf2df]" />
              </div>
              <div>
                <p className="text-xl font-semibold text-[#fff7e8]">ModelPort</p>
                <p className="text-sm text-[#a9d8cb]">Admin Console</p>
              </div>
            </div>

            <div className="space-y-4">
              <h1 className="max-w-sm text-4xl font-semibold leading-tight tracking-tight text-[#fff7e8]">
                小团队模型网关控制台
              </h1>
              <p className="max-w-sm text-base leading-7 text-[#d4eadf]">
                统一管理密钥、模型路由、额度与调用日志。
              </p>
            </div>
          </div>

          <div className="rounded-xl border border-white/15 bg-white/[0.07] p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-[#bdf4df]/[0.12] text-[#9cf2df]">
                <ShieldCheck className="h-5 w-5" />
              </div>
              <div>
                <p className="text-sm font-medium text-[#fff7e8]">安全登录入口</p>
                <p className="mt-1 text-xs text-[#b8d7cd]">管理员身份验证后进入控制台</p>
              </div>
            </div>
          </div>
        </div>

        <div className="flex min-h-[560px] items-center justify-center bg-white px-6 py-10 sm:px-10 lg:px-14">
          <div className="w-full max-w-md">
            <CardHeader className="px-0 pb-8 text-left">
              <div className="mb-6 flex items-center gap-4 md:hidden">
                <div className="flex h-14 w-14 items-center justify-center rounded-xl bg-[#e3f7ef]">
                  <Zap className="h-7 w-7 text-[#0f766e]" />
                </div>
                <div>
                  <p className="text-xl font-semibold text-[#14231f]">ModelPort</p>
                  <p className="text-sm text-[#66736d]">Admin Console</p>
                </div>
              </div>
              <CardTitle className="text-3xl font-semibold tracking-tight text-[#14231f]">登录管理后台</CardTitle>
              <CardDescription className="mt-2 text-base text-[#66736d]">
                使用管理员账号继续
              </CardDescription>
            </CardHeader>

            <CardContent className="px-0">
              <form onSubmit={handleSubmit} className="space-y-5">
                <div className="space-y-2">
                  <Label htmlFor="username" className="text-sm font-medium">用户名</Label>
                  <Input
                    id="username"
                    type="text"
                    placeholder="admin"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    disabled={loading}
                    autoComplete="username"
                    className="h-12 rounded-lg border-[#d7e3dc] bg-[#fbfdfc] px-4 text-base shadow-sm focus-visible:border-[#0f766e] focus-visible:ring-[#0f766e]"
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="password" className="text-sm font-medium">密码</Label>
                  <Input
                    id="password"
                    type="password"
                    placeholder="请输入密码"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    disabled={loading}
                    autoComplete="current-password"
                    className="h-12 rounded-lg border-[#d7e3dc] bg-[#fbfdfc] px-4 text-base shadow-sm focus-visible:border-[#0f766e] focus-visible:ring-[#0f766e]"
                  />
                  {error && (
                    <p className="rounded-lg border border-destructive/20 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                      {error}
                    </p>
                  )}
                </div>
                <Button
                  type="submit"
                  size="lg"
                  className="h-12 w-full rounded-lg bg-[#0f766e] text-base text-white shadow-[0_12px_24px_rgba(15,118,110,0.25)] hover:bg-[#0b625c]"
                  disabled={loading}
                >
                  {loading && <Loader2 className="h-4 w-4 animate-spin" />}
                  登录
                </Button>
                <p className="text-center text-sm text-muted-foreground">
                  管理员账号由服务端认证配置提供
                </p>
              </form>
            </CardContent>
          </div>
        </div>
      </Card>
    </div>
  )
}
