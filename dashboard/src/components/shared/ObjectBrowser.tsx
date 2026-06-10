import { useRef, useState, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { useProtectedAction } from './navigation-protection'
export function ObjectBrowser<T extends { id: string }>({ items, label, summary, renderDetail, onSelect }: {
  items: T[]; label: (item: T) => ReactNode; summary: (item: T) => ReactNode; renderDetail: (item: T) => ReactNode; onSelect?: () => void;
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const chosen = items.find(item => item.id === selectedId)
  const selected = chosen || items[0]
  const lastButton = useRef<HTMLButtonElement | null>(null)
  const protect = useProtectedAction()
  return <div className={`workbench-split ${chosen ? 'has-selection' : ''}`}>
    <div className="workbench-directory" aria-label="对象目录">{items.map(item => <button key={item.id} type="button" aria-pressed={selected?.id === item.id} onClick={event => { lastButton.current = event.currentTarget; protect(() => { onSelect?.(); setSelectedId(item.id) }) }}><span>{label(item)}<small>{summary(item)}</small></span><span aria-hidden>›</span></button>)}</div>
    <div className="workbench-detail"><Button className="workbench-return" variant="outline" size="sm" onClick={() => protect(() => { onSelect?.(); setSelectedId(null); requestAnimationFrame(() => lastButton.current?.focus()) })}>返回对象目录</Button><div className="workbench-object-facts">{selected ? renderDetail(selected) : <p>选择对象后查看详情。</p>}</div><div id="workbench-editor" /></div>
  </div>
}
