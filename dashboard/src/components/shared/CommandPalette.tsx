import { useEffect, useState } from 'react'
import { Command } from 'cmdk'
import { useNavigate } from 'react-router-dom'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { NAV_ITEMS } from '@/lib/constants'
import { Search, LayoutDashboard, KeyRound, Users, Gauge, Boxes, ScrollText, Settings } from 'lucide-react'

const iconMap: Record<string, React.ElementType> = {
  LayoutDashboard,
  KeyRound,
  Users,
  Gauge,
  Boxes,
  ScrollText,
  Settings,
}

export function CommandPalette() {
  const [open, setOpen] = useState(false)
  const navigate = useNavigate()

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        setOpen((prev) => !prev)
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [])

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="overflow-hidden p-0 gap-0 max-w-lg">
        <Command className="rounded-lg border shadow-md" shouldFilter>
          <div className="flex items-center border-b px-3">
            <Search className="mr-2 h-4 w-4 shrink-0 opacity-50" />
            <Command.Input
              placeholder="搜索页面、模型、设置..."
              className="flex h-11 w-full rounded-md bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50"
            />
          </div>
          <Command.List className="max-h-[300px] overflow-y-auto p-2">
            <Command.Empty className="py-6 text-center text-sm text-muted-foreground">
              未找到结果
            </Command.Empty>
            <Command.Group heading="页面导航">
              {NAV_ITEMS.map((item) => {
                const Icon = iconMap[item.icon]
                return (
                  <Command.Item
                    key={item.path}
                    value={item.label}
                    onSelect={() => {
                      navigate(item.path)
                      setOpen(false)
                    }}
                    className="relative flex cursor-pointer select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none aria-selected:bg-accent aria-selected:text-accent-foreground"
                  >
                    {Icon && <Icon className="h-4 w-4" />}
                    {item.label}
                  </Command.Item>
                )
              })}
            </Command.Group>
          </Command.List>
        </Command>
      </DialogContent>
    </Dialog>
  )
}
