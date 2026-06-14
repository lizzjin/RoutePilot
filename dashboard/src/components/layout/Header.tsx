import { useAuthStore, useAppStore } from '@/stores'
import { BreadcrumbNav } from '@/components/shared/BreadcrumbNav'
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
import { Moon, Sun, Monitor, LogOut, User, Menu, Search } from 'lucide-react'
import { useState } from 'react'

interface HeaderProps {
  onMenuClick?: () => void
  isMobile?: boolean
}

export function Header({ onMenuClick, isMobile }: HeaderProps) {
  const currentUser = useAuthStore((s) => s.currentUser)
  const logout = useAuthStore((s) => s.logout)
  const theme = useAppStore((s) => s.theme)
  const setTheme = useAppStore((s) => s.setTheme)

  const themeIcons = { light: Sun, dark: Moon, system: Monitor }
  const ThemeIcon = themeIcons[theme]

  // Keyboard shortcut hint for command palette
  const [isMac] = useState(() => navigator.platform.includes('Mac'))

  return (
    <header className="flex h-14 items-center justify-between border-b border-border/50 bg-background/80 backdrop-blur-xl px-4 md:px-6">
      <div className="flex items-center gap-3 min-w-0">
        {isMobile && (
          <Button variant="ghost" size="icon" className="h-8 w-8 shrink-0" onClick={onMenuClick}>
            <Menu className="h-4 w-4" />
          </Button>
        )}
        <div className="min-w-0">
          <BreadcrumbNav />
        </div>
        {isMockMode && (
          <span className="shrink-0 rounded-full bg-yellow-100 px-2.5 py-0.5 text-xs font-medium text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300">
            演示数据
          </span>
        )}
      </div>

      <div className="flex items-center gap-1.5">
        {/* Command palette trigger */}
        <Button
          variant="outline"
          size="sm"
          className="hidden h-8 gap-2 text-xs text-muted-foreground md:flex"
          onClick={() => document.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }))}
        >
          <Search className="h-3.5 w-3.5" />
          <span>搜索...</span>
          <kbd className="pointer-events-none ml-1 select-none rounded border bg-muted px-1 text-[10px] font-medium">
            {isMac ? '⌘' : 'Ctrl+'}K
          </kbd>
        </Button>

        {/* Theme toggle */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="h-8 w-8">
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
            <Button variant="ghost" className="relative h-8 w-8 rounded-full">
              <Avatar className="h-8 w-8 ring-2 ring-primary/20">
                <AvatarFallback className="bg-primary/10 text-primary text-xs font-semibold">
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
              </div>
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem>
              <User className="mr-2 h-4 w-4" />
              个人资料
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={logout} className="text-destructive focus:text-destructive">
              <LogOut className="mr-2 h-4 w-4" />
              退出登录
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </header>
  )
}
