import type { ButtonHTMLAttributes } from 'react'

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: 'primary' | 'secondary'
}

export function Button({
  className = '',
  type = 'button',
  variant = 'primary',
  ...props
}: ButtonProps) {
  const styles =
    variant === 'primary'
      ? 'border border-cyan-200/20 bg-cyan-200 text-slate-950 hover:bg-cyan-100 disabled:bg-cyan-200/50'
      : 'border border-white/10 bg-white/[0.06] text-slate-200 hover:bg-white/10 disabled:opacity-50'

  return (
    <button
      type={type}
      className={`inline-flex min-h-11 items-center justify-center gap-2 rounded-xl px-4 text-sm font-semibold transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cyan-300 disabled:cursor-not-allowed ${styles} ${className}`}
      {...props}
    />
  )
}
