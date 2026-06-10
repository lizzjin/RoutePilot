import { useProtectedAction } from '@/components/shared/navigation-protection'
import { useAuthStore, useAppStore } from '@/stores'
import { BrandMark } from '@/components/shared/BrandMark'
import { Link } from 'react-router-dom'
import { Workspaces } from './Workspaces'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Button } from '@/components/ui/button'
import { isMockMode } from '@/lib/mock-mode'
import { ROLE_LABELS } from '@/lib/constants'
import { Moon, Sun, Monitor, LogOut, Search } from 'lucide-react'

export function Header() {
  const protect = useProtectedAction()
  const currentUser = useAuthStore((s) => s.currentUser)
  const logout = useAuthStore((s) => s.logout)
  const theme = useAppStore((s) => s.theme)
  const setTheme = useAppStore((s) => s.setTheme)

  const themeIcons = { light: Sun, dark: Moon, system: Monitor }
  const ThemeIcon = themeIcons[theme]

  const openCommandPalette = () => document.dispatchEvent(new CustomEvent('routepilot:open-command-palette'))

  return (
    <header className="workbench-header">
      <Link to="/dashboard" className="workbench-brand"><span><BrandMark className="h-7 w-7" /></span><strong>RoutePilot</strong><small>Beta · 单实例</small></Link>
      <Workspaces />
      <div className="workbench-session">
        {isMockMode && (
          <span className="shrink-0 rounded-full border border-warning-text/30 bg-warning-soft px-2 py-0.5 text-xs font-medium text-warning-text">
            演示数据
          </span>
        )}
      </div>

      <div className="flex items-center gap-1.5">
        <Button variant="ghost" size="icon" className="h-10 w-10" onClick={openCommandPalette} aria-label="打开快速导航">
          <Search className="h-4 w-4" />
        </Button>

        {/* Theme toggle */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="h-10 w-10" aria-label="切换界面主题">
              <ThemeIcon className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => setTheme('light')}>
              <Sun className="mr-2 h-4 w-4" />
              浅色
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setTheme('dark')}>
              <Moon className="mr-2 h-4 w-4" />
              深色
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setTheme('system')}>
              <Monitor className="mr-2 h-4 w-4" />
              跟随系统
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        {/* User menu */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" className="relative h-10 w-10 rounded-full" aria-label="打开账户菜单">
              <Avatar className="h-10 w-10 border border-border">
                <AvatarFallback className="bg-muted text-foreground text-xs font-semibold">
                  {currentUser?.username?.charAt(0).toUpperCase() || 'U'}
                </AvatarFallback>
              </Avatar>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent className="w-56" align="end" forceMount>
            <DropdownMenuLabel className="font-normal">
              <div className="flex flex-col space-y-1">
                <p className="text-sm font-medium leading-none">
                  {currentUser?.username || '用户'}
                </p>
                <p className="text-xs leading-none text-muted-foreground">
                  {currentUser?.email || ''}
                </p>
                <p className="pt-1 text-xs font-medium text-foreground">
                  {ROLE_LABELS[currentUser?.role || ''] || currentUser?.role || '未知角色'}
                </p>
              </div>
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => protect(() => { void logout() })} className="text-destructive-text focus:text-destructive-text">
              <LogOut className="mr-2 h-4 w-4" />
              退出登录
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </header>
  )
}
