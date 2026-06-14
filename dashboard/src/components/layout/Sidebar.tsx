import { NavLink, useLocation } from 'react-router-dom'
import { useAppStore } from '@/stores'
import { NAV_ITEMS } from '@/lib/constants'
import { cn } from '@/lib/utils'
import {
  LayoutDashboard,
  KeyRound,
  Users,
  Gauge,
  Boxes,
  ScrollText,
  Settings,
  ChevronLeft,
  ChevronRight,
  Zap,
} from 'lucide-react'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger, TooltipProvider } from '@/components/ui/tooltip'

const iconMap: Record<string, React.ElementType> = {
  LayoutDashboard,
  KeyRound,
  Users,
  Gauge,
  Boxes,
  ScrollText,
  Settings,
}

interface SidebarProps {
  onNavigate?: () => void
}

export function Sidebar({ onNavigate }: SidebarProps) {
  const collapsed = useAppStore((s) => s.sidebarCollapsed)
  const toggle = useAppStore((s) => s.toggleSidebar)
  const location = useLocation()

  return (
    <TooltipProvider delayDuration={0}>
      <aside
        className={cn(
          'flex h-screen flex-col border-r border-border/50 bg-sidebar/80 backdrop-blur-xl text-sidebar-foreground transition-all duration-200',
          collapsed ? 'w-14' : 'w-56',
        )}
      >
        {/* Logo */}
        <div className="flex h-14 items-center gap-2.5 border-b border-border/50 px-3">
          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Zap className="h-4 w-4" />
          </div>
          {!collapsed && (
            <span className="text-lg font-bold tracking-tight">ModelPort</span>
          )}
        </div>

        {/* Navigation */}
        <ScrollArea className="flex-1 py-3">
          <nav className="flex flex-col gap-1 px-2">
            {NAV_ITEMS.map((item) => {
              const Icon = iconMap[item.icon]
              const isActive =
                location.pathname === item.path ||
                (item.path !== '/dashboard' && location.pathname.startsWith(item.path))

              const link = (
                <NavLink
                  key={item.path}
                  to={item.path}
                  onClick={onNavigate}
                  className={cn(
                    'group flex items-center gap-3 rounded-md px-2.5 py-2 text-sm font-medium transition-all duration-150',
                    isActive
                      ? 'bg-sidebar-accent text-sidebar-primary shadow-sm'
                      : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground',
                  )}
                >
                  {Icon && (
                    <Icon
                      className={cn(
                        'h-4 w-4 shrink-0 transition-colors',
                        isActive ? 'text-sidebar-primary' : 'text-sidebar-foreground/50 group-hover:text-sidebar-foreground/80',
                      )}
                    />
                  )}
                  {!collapsed && <span>{item.label}</span>}
                </NavLink>
              )

              if (collapsed) {
                return (
                  <Tooltip key={item.path}>
                    <TooltipTrigger asChild>{link}</TooltipTrigger>
                    <TooltipContent side="right" sideOffset={8}>
                      {item.label}
                    </TooltipContent>
                  </Tooltip>
                )
              }

              return link
            })}
          </nav>
        </ScrollArea>

        <Separator className="opacity-50" />

        {/* Status indicator + collapse toggle */}
        <div className="flex items-center justify-between p-2">
          {!collapsed && (
            <div className="flex items-center gap-2 px-1 text-xs text-muted-foreground">
              <span className="relative flex h-2 w-2">
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-green-400 opacity-75" />
                <span className="relative inline-flex h-2 w-2 rounded-full bg-green-500" />
              </span>
              运行中
            </div>
          )}
          <Button variant="ghost" size="icon" onClick={toggle} className="h-8 w-8 shrink-0">
            {collapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
          </Button>
        </div>
      </aside>
    </TooltipProvider>
  )
}
