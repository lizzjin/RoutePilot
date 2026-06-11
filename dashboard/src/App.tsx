import { createBrowserRouter, RouterProvider, Navigate } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { lazy, Suspense, useEffect } from 'react'
import type { ReactNode } from 'react'
import { useAuthStore } from '@/stores'
import { AppLayout } from '@/components/layout/AppLayout'
import { ProtectedRoute } from '@/components/layout/ProtectedRoute'

const LoginPage = lazy(() => import('@/pages/LoginPage').then((module) => ({ default: module.LoginPage })))
const DashboardPage = lazy(() => import('@/pages/DashboardPage').then((module) => ({ default: module.DashboardPage })))
const ApiKeysPage = lazy(() => import('@/pages/ApiKeysPage').then((module) => ({ default: module.ApiKeysPage })))
const UsersPage = lazy(() => import('@/pages/UsersPage').then((module) => ({ default: module.UsersPage })))
const QuotasPage = lazy(() => import('@/pages/QuotasPage').then((module) => ({ default: module.QuotasPage })))
const ModelsPage = lazy(() => import('@/pages/ModelsPage').then((module) => ({ default: module.ModelsPage })))
const LogsPage = lazy(() => import('@/pages/LogsPage').then((module) => ({ default: module.LogsPage })))
const SettingsPage = lazy(() => import('@/pages/SettingsPage').then((module) => ({ default: module.SettingsPage })))
const NotFoundPage = lazy(() => import('@/pages/NotFoundPage').then((module) => ({ default: module.NotFoundPage })))

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5,
      retry: 1,
    },
  },
})

function PageLoader() {
  return <div className="flex h-64 items-center justify-center text-muted-foreground">加载中...</div>
}

function lazyPage(page: ReactNode) {
  return <Suspense fallback={<PageLoader />}>{page}</Suspense>
}

const router = createBrowserRouter([
  {
    path: '/login',
    element: lazyPage(<LoginPage />),
  },
  {
    path: '/',
    element: (
      <ProtectedRoute>
        <AppLayout />
      </ProtectedRoute>
    ),
    children: [
      { index: true, element: <Navigate to="/dashboard" replace /> },
      { path: 'dashboard', element: lazyPage(<DashboardPage />) },
      { path: 'api-keys', element: lazyPage(<ApiKeysPage />) },
      { path: 'users', element: lazyPage(<UsersPage />) },
      { path: 'quotas', element: lazyPage(<QuotasPage />) },
      { path: 'models', element: lazyPage(<ModelsPage />) },
      { path: 'logs', element: lazyPage(<LogsPage />) },
      { path: 'settings', element: lazyPage(<SettingsPage />) },
    ],
  },
  { path: '*', element: lazyPage(<NotFoundPage />) },
])

function App() {
  const initialize = useAuthStore((s) => s.initialize)

  useEffect(() => {
    initialize()
  }, [initialize])

  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  )
}

export default App
