import { GatewayStatus } from './GatewayStatus'
import { NavLink, useLocation } from 'react-router-dom'
import { useAuthStore } from '@/stores'
import { navItemsForRole } from '@/lib/constants'

const WORKSPACES = [
  { name: '运行', paths: ['/dashboard', '/logs', '/operations', '/enterprise'] },
  { name: '配置', paths: ['/models', '/settings'] },
  { name: '访问与治理', paths: ['/api-keys', '/users', '/quotas', '/governance'] },
  { name: '接入指南', paths: ['/guide'] },
]
export function Workspaces({ local = false }: { local?: boolean }) {
  const role = useAuthStore(state => state.currentUser?.role)
  const { pathname } = useLocation()
  const items = navItemsForRole(role)
  const groups = WORKSPACES.map(group => ({ ...group, items: group.paths.flatMap(path => items.filter(item => item.path === path)) })).filter(group => group.items.length)
  const active = groups.find(group => group.paths.includes(pathname))
  return <nav className={local ? 'workbench-local-nav' : 'workbench-workspaces'} aria-label={local ? '工作区页面' : '工作区'}>
    {local ? active?.items.map(item => <NavLink key={item.path} to={item.path}>{item.path === '/dashboard' ? '运行概览' : item.path === '/models' ? '模型与渠道' : item.label}</NavLink>)
      : groups.map(group => <NavLink key={group.name} to={group.items[0].path} className={active?.name === group.name ? 'active' : ''}>{group.name}</NavLink>)}
    {local && <GatewayStatus />}
  </nav>
}
