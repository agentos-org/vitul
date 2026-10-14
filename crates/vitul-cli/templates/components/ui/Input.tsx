import type { InputHTMLAttributes } from 'react'

export function TextInput({
  className = '',
  ...props
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={`min-h-11 min-w-0 flex-1 rounded-xl border border-[var(--border)] bg-[var(--inset)] px-4 text-sm text-[var(--text)] outline-none placeholder:text-[var(--muted)] focus:border-cyan-200/50 focus:ring-2 focus:ring-cyan-200/10 ${className}`}
      {...props}
    />
  )
}
