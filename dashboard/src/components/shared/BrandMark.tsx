import type { SVGProps } from 'react'

/** RoutePilot's branching path mark, shared by the shell and sign-in page. */
export function BrandMark(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 32 32" fill="none" aria-hidden="true" {...props}>
      <path d="M7 25V17C7 12 12 12 16 12H25M16 12V7" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
      <circle cx="7" cy="25" r="3" fill="currentColor" />
      <circle cx="16" cy="7" r="3" fill="currentColor" />
      <path d="m22 8 4 4-4 4" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}
