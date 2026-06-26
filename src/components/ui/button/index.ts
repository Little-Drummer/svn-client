import type { VariantProps } from "class-variance-authority"
import { cva } from "class-variance-authority"

export { default as Button } from "./Button.vue"

/**
 * macOS 风格 Button：
 *   default → accent 胶囊（近纯色 + 顶部高光 + 同色软投影，禁用变灰）
 *   destructive → 红色胶囊，同一套语言
 *   outline → bezeled（白底 + hairline + 极轻投影）
 *   secondary → 浅灰胶囊
 *   ghost → 透明，仅 hover 显
 *   link → 文本按钮
 *
 * 尺寸节奏对齐 macOS：
 *   xs 24 / sm 26 / default 28 / lg 36
 *   icon-* 提供等宽 icon-only 按钮（hover-revealed action 用）
 *
 * 形状：accent / destructive 这类 prominent 按钮用胶囊形，圆角与内边距
 * 在 compoundVariants 覆盖；禁用态由 ctl-accent / ctl-destructive 的
 * CSS 接管（变灰 bezel），其余 variant 禁用时整体降透明度。
 */
export const buttonVariants = cva(
  [
    "inline-flex items-center justify-center gap-1.5 whitespace-nowrap",
    "font-medium select-none cursor-default",
    "transition-[background-color,box-shadow,filter,color] duration-150 ease-out",
    "focus-visible:outline-none focus-visible:[box-shadow:inset_0_0_0_0.5px_var(--accent),0_0_0_3px_var(--accent-ring)]",
    "disabled:pointer-events-none",
    "[&_svg]:pointer-events-none [&_svg]:shrink-0",
  ].join(" "),
  {
    variants: {
      variant: {
        default: "ctl-accent",
        destructive: "ctl-destructive",
        outline: "ctl-bezeled disabled:opacity-45",
        secondary: "ctl-secondary disabled:opacity-45",
        ghost: "ctl-ghost disabled:opacity-45",
        link: "ctl-link h-auto disabled:opacity-45",
      },
      size: {
        "default": "h-7 px-3 text-[13px] rounded-md",
        "xs": "h-6 px-2.5 text-[12px] rounded-md",
        "sm": "h-[26px] px-2.5 text-[12px] rounded-md",
        "lg": "h-9 px-5 text-[14px] rounded-md",
        "icon": "size-7 rounded-md",
        "icon-sm": "size-[26px] rounded-md",
        "icon-lg": "size-9 rounded-md",
      },
    },
    compoundVariants: [
      // prominent 按钮统一胶囊形，水平内边距按胶囊节奏放宽
      { variant: "default", size: "default", class: "rounded-full px-4" },
      { variant: "destructive", size: "default", class: "rounded-full px-4" },
      { variant: "default", size: "xs", class: "rounded-full px-3" },
      { variant: "destructive", size: "xs", class: "rounded-full px-3" },
      { variant: "default", size: "sm", class: "rounded-full px-3.5" },
      { variant: "destructive", size: "sm", class: "rounded-full px-3.5" },
      { variant: "default", size: "lg", class: "rounded-full px-6" },
      { variant: "destructive", size: "lg", class: "rounded-full px-6" },
      { variant: "default", size: "icon", class: "rounded-full" },
      { variant: "destructive", size: "icon", class: "rounded-full" },
      { variant: "default", size: "icon-sm", class: "rounded-full" },
      { variant: "destructive", size: "icon-sm", class: "rounded-full" },
      { variant: "default", size: "icon-lg", class: "rounded-full" },
      { variant: "destructive", size: "icon-lg", class: "rounded-full" },
    ],
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
)

export type ButtonVariants = VariantProps<typeof buttonVariants>
