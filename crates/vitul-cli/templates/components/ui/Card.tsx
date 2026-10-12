import type { HTMLAttributes } from 'react'

export function Card({
  className = '',
  ...props
}: HTMLAttributes<HTMLElement>) {
  return (
    <section
      className={`rounded-2xl border border-white/[0.09] bg-[#11141b]/85 shadow-[0_18px_55px_rgba(0,0,0,0.22)] backdrop-blur ${className}`}
      {...props}
    />
  )
}
