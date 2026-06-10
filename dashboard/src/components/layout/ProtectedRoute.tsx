import { Navigate, useLocation } from 'react-router-dom'
import { useAuthStore } from '@/stores'

export function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  const isInitializing = useAuthStore((s) => s.isInitializing)
  const location = useLocation()

  if (isInitializing) {
    return <div className="flex min-h-screen items-center justify-center text-muted-foreground">加载中...</div>
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" state={{ from: location }} replace />
  }

  return <>{children}</>
}
