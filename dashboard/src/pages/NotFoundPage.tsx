import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { FileQuestion } from 'lucide-react'

export function NotFoundPage() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-6 animate-fade-in">
      <div className="relative">
        <div className="pointer-events-none absolute inset-0 -z-10 mx-auto h-[300px] w-[300px] rounded-full bg-primary/10 blur-[80px]" />
        <p className="bg-gradient-to-br from-primary to-primary/50 bg-clip-text text-[8rem] font-black leading-none tracking-tighter text-transparent">
          404
        </p>
      </div>
      <div className="rounded-full bg-muted p-4">
        <FileQuestion className="h-10 w-10 text-muted-foreground" />
      </div>
      <div className="space-y-2 text-center">
        <h2 className="text-xl font-semibold">页面不存在</h2>
        <p className="text-muted-foreground">你访问的页面已被移除或从未存在</p>
      </div>
      <div className="flex items-center gap-3">
        <Button asChild>
          <Link to="/dashboard">返回仪表盘</Link>
        </Button>
        <Button variant="outline" asChild>
          <Link to="/">返回首页</Link>
        </Button>
      </div>
    </div>
  )
}
