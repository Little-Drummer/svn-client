// 简单的"代际令牌"工具：每次调用 next() 拿到一个新令牌，旧令牌的结果应丢弃。
// 用途：异步加载在结果返回前被新的请求超越时，避免旧结果覆盖新数据。
//
//   const gen = createGeneration()
//   const t = gen.next()
//   const data = await fetch(...)
//   if (!gen.isCurrent(t)) return    // 已被超越，丢弃
//   apply(data)
export interface Generation {
  next: () => number
  isCurrent: (token: number) => boolean
  invalidate: () => void
}

export function createGeneration(): Generation {
  let current = 0
  return {
    next: () => ++current,
    isCurrent: (token) => token === current,
    invalidate: () => {
      current++
    },
  }
}
