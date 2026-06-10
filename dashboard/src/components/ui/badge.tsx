import * as React from "react"
import { cn } from "@/lib/utils"

const Badge = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & {
    variant?: 'default' | 'secondary' | 'destructive' | 'outline' | 'success' | 'warning'
  }
>(({ className, variant = 'default', ...props }, ref) => {
  const variants: Record<string, string> = {
    default: "border-border bg-muted text-foreground",
    secondary: "border-border/70 bg-secondary/80 text-secondary-foreground",
    destructive: "border-destructive-text/30 bg-destructive-soft text-destructive-text",
    outline: "border-border/85 bg-background/70 text-foreground/80",
    success: "border-success-text/30 bg-success-soft text-success-text",
    warning: "border-warning-text/30 bg-warning-soft text-warning-text",
  }

  return (
    <div
      ref={ref}
      className={cn(
        "inline-flex min-w-0 max-w-full items-center whitespace-normal break-words rounded-sm border px-2 py-0.5 text-xs font-medium leading-4 transition-colors focus:outline-none focus:ring-[3px] focus-visible:ring-ring",
        variants[variant],
        className
      )}
      {...props}
    />
  )
})
Badge.displayName = "Badge"

export { Badge }
