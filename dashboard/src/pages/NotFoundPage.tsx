import { Link, useNavigate } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { useAuthStore } from '@/stores'
import { ArrowLeft, FileQuestion, LayoutDashboard, LogIn } from 'lucide-react'

export function NotFoundPage() {
  const navigate = useNavigate()
  const isAuthenticated = useAuthStore((state) => state.isAuthenticated)

  return (
    <main className="mx-auto grid min-h-dvh max-w-4xl content-center gap-6 px-6 sm:grid-cols-[180px_1fr]">
      <div className="relative">
        <p className="text-[6rem] font-semibold leading-none tracking-tight text-foreground">
          404
        </p>
      </div>
      <div className="hidden">
        <FileQuestion className="h-10 w-10 text-muted-foreground" />
      </div>
      <div className="space-y-2">
        <h1 className="text-xl font-semibold">页面不存在</h1>
        <p className="text-muted-foreground">地址可能已变更。你可以返回上一页，或回到可用入口继续操作。</p>
      </div>
      <div className="flex flex-wrap items-center gap-3 sm:col-start-2">
        <Button variant="outline" onClick={() => navigate(-1)}>
          <ArrowLeft className="h-4 w-4" />
          返回上一页
        </Button>
        <Button asChild>
          <Link to={isAuthenticated ? '/dashboard' : '/login'}>
            {isAuthenticated ? <LayoutDashboard className="h-4 w-4" /> : <LogIn className="h-4 w-4" />}
            {isAuthenticated ? '返回运行工作区' : '前往登录'}
          </Link>
        </Button>
      </div>
    </main>
  )
}
