import type { InputHTMLAttributes } from 'react'

export function TextInput({
  className = '',
  ...props
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={`min-h-11 min-w-0 flex-1 rounded-xl border border-white/10 bg-black/20 px-4 text-sm text-white outline-none placeholder:text-slate-600 focus:border-cyan-200/50 focus:ring-2 focus:ring-cyan-200/10 ${className}`}
      {...props}
    />
  )
}
