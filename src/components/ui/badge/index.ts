import type { VariantProps } from "class-variance-authority"
import { cva } from "class-variance-authority"

export { default as Badge } from "./Badge.vue"

/**
 * macOS 风 tinted pill：浅色填充 + 同色文字 + 同色 hairline 描边 + 圆胶囊
 *   default     → accent (modified)
 *   secondary   → 灰
 *   destructive → 红
 *   success     → 绿
 *   warning     → 橙
 *   outline     → 纯描边胶囊（不带 tint）
 */
export const badgeVariants = cva(
  [
    "inline-flex items-center gap-1 whitespace-nowrap",
    "h-[18px] px-2 rounded-full",
    "text-[11px] font-medium leading-none",
    "border border-transparent",
    "transition-colors focus:outline-none",
  ].join(" "),
  {
    variants: {
      variant: {
        default: [
          "text-[color:var(--accent)]",
          "bg-[var(--accent-soft)]",
          "[border-color:color-mix(in_srgb,var(--accent)_28%,transparent)]",
        ].join(" "),
        secondary: [
          "text-[color:var(--fg-muted)]",
          "bg-[color:color-mix(in_srgb,var(--fg)_6%,transparent)]",
          "[border-color:color-mix(in_srgb,var(--fg)_12%,transparent)]",
        ].join(" "),
        destructive: [
          "text-[color:var(--danger)]",
          "bg-[var(--danger-soft)]",
          "[border-color:color-mix(in_srgb,var(--danger)_32%,transparent)]",
        ].join(" "),
        success: [
          "text-[color:var(--success)]",
          "bg-[var(--success-soft)]",
          "[border-color:color-mix(in_srgb,var(--success)_32%,transparent)]",
        ].join(" "),
        warning: [
          "text-[color:var(--warning)]",
          "bg-[var(--warning-soft)]",
          "[border-color:color-mix(in_srgb,var(--warning)_34%,transparent)]",
        ].join(" "),
        outline: [
          "text-[color:var(--fg)]",
          "bg-transparent",
          "[border-color:var(--stroke)]",
        ].join(" "),
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
)

export type BadgeVariants = VariantProps<typeof badgeVariants>
