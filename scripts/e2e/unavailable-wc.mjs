// 冒烟：路径不可用的工作副本应置灰，选中后状态页显示降级提示而不是报错 toast。
import { chromium } from 'playwright-core'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const here = dirname(fileURLToPath(import.meta.url))
const mockScript = readFileSync(join(here, 'tauri-mock.js'), 'utf-8')
const BASE = process.env.E2E_BASE_URL || 'http://localhost:5199'

const browser = await chromium.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: true,
})
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } })
await page.addInitScript(mockScript)
const fails = []
const check = (name, ok) => {
  console.log(ok ? `✓ ${name}` : `✗ ${name}`)
  if (!ok) fails.push(name)
}

await page.goto(BASE, { waitUntil: 'networkidle' })
await page.waitForTimeout(800)

check('不可用行已置灰', (await page.locator('.wc-item.unavailable').count()) === 1)

await page.locator('.wc-item.unavailable').click()
await page.waitForTimeout(400)
check('降级提示页出现', await page.locator('.unavailable-pane').first().isVisible())
check(
  '提示包含路径',
  (await page.locator('.unavailable-path').first().innerText()).includes('/Volumes/gone'),
)
check('提供重新检查按钮', await page.locator('.unavailable-pane button:has-text("重新检查")').first().isVisible())

// 切到历史 tab 同样降级
await page.getByRole('tab', { name: '历史' }).click()
await page.waitForTimeout(300)
check('历史页同样降级', await page.locator('.unavailable-pane').nth(1).isVisible())

const errors = await page.evaluate(() => window.__TAURI_MOCK_ERRORS__)
const realErrors = errors.filter((e) => !e.startsWith('unmocked invoke'))
check('无未处理页面错误', realErrors.length === 0)
if (realErrors.length) console.log(realErrors)

await browser.close()
if (fails.length) {
  console.log(`\n失败 ${fails.length} 项`)
  process.exit(1)
}
console.log('\n全部通过')
