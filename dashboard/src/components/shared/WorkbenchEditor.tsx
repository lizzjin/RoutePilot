import { createContext, useContext, useEffect, useId, useRef, useState, type HTMLAttributes, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { ArrowLeft } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from './ConfirmDialog'
import { useNavigationProtection } from './navigation-protection'
const EditorContext = createContext({ close: () => {}, titleId: '', descriptionId: '' })
export function Root({ open, onOpenChange, children, targetId = 'workbench-editor' }: { open?: boolean; onOpenChange?: (open: boolean) => void; children: ReactNode; targetId?: string }) {
  return open ? <ActiveEditor close={() => onOpenChange?.(false)} targetId={targetId}>{children}</ActiveEditor> : null
}
function ActiveEditor({ children, close, targetId }: { children: ReactNode; close: () => void; targetId: string }) {
  const [dirty, setDirty] = useState(false)
  const [confirm, setConfirm] = useState(false)
  const id = useId()
  const ref = useRef<HTMLDivElement>(null)
  useNavigationProtection(dirty, { title: '有未保存的修改，确认离开？', description: '离开会丢弃当前编辑内容；已提交的操作不受影响。' })
  useEffect(() => { const previous = document.activeElement as HTMLElement | null; ref.current?.focus(); return () => { if (previous?.isConnected) previous.focus() } }, [])
  const target = document.getElementById(targetId)
  const content = <EditorContext.Provider value={{ close: () => dirty ? setConfirm(true) : close(), titleId: id, descriptionId: id + '-description' }}><div ref={ref} tabIndex={-1} onChangeCapture={() => setDirty(true)} onClickCapture={event => { if (event.target instanceof Element && event.target.closest('[role=switch], [role=checkbox], [role=option]')) setDirty(true) }} className="workbench-editor outline-none">{children}</div><ConfirmDialog open={confirm} title="放弃未保存的修改？" description="关闭编辑区域会丢弃当前修改。" confirmLabel="放弃修改" cancelLabel="继续编辑" onCancel={() => setConfirm(false)} onConfirm={close} /></EditorContext.Provider>
  return target ? createPortal(content, target) : content
}
export function Content({ children }: { children: ReactNode; className?: string }) {
  const { close, titleId, descriptionId } = useContext(EditorContext)
  return <section className="workbench-editor-content" aria-labelledby={titleId} aria-describedby={descriptionId}><Button variant="outline" size="sm" onClick={close} className="mb-5"><ArrowLeft className="mr-2 h-4 w-4" />返回对象详情</Button>{children}</section>
}
export function Title(props: HTMLAttributes<HTMLHeadingElement>) { const { titleId } = useContext(EditorContext); return <h2 {...props} id={titleId} className="text-xl font-semibold" /> }
export function Description(props: HTMLAttributes<HTMLParagraphElement>) { const { descriptionId } = useContext(EditorContext); return <p {...props} id={descriptionId} className="mt-2 text-sm text-muted-foreground" /> }
export function Header(props: HTMLAttributes<HTMLDivElement>) { return <div {...props} className="mb-6" /> }
export function Footer(props: HTMLAttributes<HTMLDivElement>) { return <div {...props} className="workbench-editor-footer" /> }
