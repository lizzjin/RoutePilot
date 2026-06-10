import { NavigationProtection } from '@/components/shared/NavigationProtection'
import { Outlet } from 'react-router-dom'
import { Header } from './Header'
import { Workspaces } from './Workspaces'
import { CommandPalette } from '@/components/shared/CommandPalette'
import { Toaster } from 'sonner'
import './workbench.css'

export function AppLayout() {
  return <NavigationProtection><div className="workbench-shell">
    <a href="#main-content" className="fixed left-3 top-3 z-[100] -translate-y-20 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground focus:translate-y-0">跳到主要内容</a>
    <Header />
    <Workspaces local />
    <main id="main-content" tabIndex={-1} className="workbench-main outline-none"><Outlet /></main>
    <CommandPalette />
    <Toaster position="top-right" toastOptions={{ className: 'text-sm', duration: 4000 }} richColors closeButton />
  </div></NavigationProtection>
}
