import type { VariantProps } from "class-variance-authority"
import { cva } from "class-variance-authority"

export { default as Button } from "./Button.vue"

/**
 * macOS 风格 Button：
 *   default → accent capsule（蓝渐变 + 内高光）
 *   destructive → 红色 capsule
 *   outline → bezeled（白底 + hairline + 极轻投影）
 *   secondary → 浅灰胶囊
 *   ghost → 透明，仅 hover 显
 *   link → 文本按钮
 *
 * 尺寸节奏对齐 macOS：
 *   xs 22 / sm 26 / default 28 / lg 36
 *   icon-* 提供等宽 icon-only 按钮（hover-revealed action 用）
 */
export const buttonVariants = cva(
  [
    "inline-flex items-center justify-center gap-1.5 whitespace-nowrap",
    "font-medium select-none cursor-default",
    "transition-[background-color,box-shadow,filter,color] duration-150 ease-out",
    "focus-visible:outline-none focus-visible:[box-shadow:inset_0_0_0_0.5px_var(--accent),0_0_0_3px_var(--accent-ring)]",
    "disabled:pointer-events-none disabled:opacity-45",
    "[&_svg]:pointer-events-none [&_svg]:shrink-0",
  ].join(" "),
  {
    variants: {
      variant: {
        default: "ctl-accent",
        destructive: "ctl-destructive",
        outline: "ctl-bezeled",
        secondary: "ctl-secondary",
        ghost: "ctl-ghost",
        link: "ctl-link h-auto",
      },
      size: {
        "default": "h-7 px-3 text-[13px] rounded-md",
        "xs": "h-[22px] px-2 text-[11px] rounded-[5px]",
        "sm": "h-[26px] px-2.5 text-[12px] rounded-md",
        "lg": "h-9 px-5 text-[14px] rounded-md",
        "icon": "size-7 rounded-md",
        "icon-sm": "size-[26px] rounded-md",
        "icon-lg": "size-9 rounded-md",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
)

export type ButtonVariants = VariantProps<typeof buttonVariants>
