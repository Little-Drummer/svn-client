// 复现：在配置 tab 下点击侧栏远端仓库列表项，主区域应切换到远端浏览视图。
// 用本机 Chrome + mock 的 Tauri internals 驱动 vite dev 页面。
import { chromium } from 'playwright-core'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const here = dirname(fileURLToPath(import.meta.url))
const mockScript = readFileSync(join(here, 'tauri-mock.js'), 'utf-8')
const BASE = process.env.E2E_BASE_URL || 'http://localhost:1420'

const browser = await chromium.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: true,
})
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } })
await page.addInitScript(mockScript)
page.on('console', (msg) => {
  if (msg.type() === 'error' || msg.type() === 'warning') {
    console.log(`[console.${msg.type()}]`, msg.text())
  }
})

await page.goto(BASE, { waitUntil: 'networkidle' })
await page.waitForTimeout(800)

async function dump(label) {
  const errors = await page.evaluate(() => window.__TAURI_MOCK_ERRORS__)
  if (errors.length) console.log(`[${label}] 页面错误:`, errors)
  const calls = await page.evaluate(() => window.__TAURI_MOCK_CALLS__.splice(0))
  console.log(`[${label}] invoke:`, calls.join(', ') || '(无)')
}

await dump('启动')

// 1. 选中一个工作副本
const wcRow = page.locator('.wc-item, [class*="copy-row"], [class*="wc-row"]').first()
const wcCount = await wcRow.count()
console.log('工作副本行数(初选选择器):', wcCount)
// 兜底：直接按文本点
if (wcCount === 0) {
  await page.getByText('rest', { exact: false }).first().click()
} else {
  await wcRow.click()
}
await page.waitForTimeout(300)
await dump('选中工作副本')

// 2. 切到配置 tab
await page.getByRole('tab', { name: '配置' }).click()
await page.waitForTimeout(300)
const configVisible = await page.locator('.preset-view').isVisible().catch(() => false)
console.log('配置页可见:', configVisible)
await dump('切到配置')

// 3. 点击侧栏远端仓库列表项
const repoItem = page.locator('.repo-item').first()
console.log('远端仓库行数:', await repoItem.count())
await repoItem.click()
await page.waitForTimeout(600)
await dump('点击远端仓库')

// 4. 断言远端浏览视图出现
const remoteVisible = await page.locator('.remote-browser').isVisible().catch(() => false)
const presetStillVisible = await page.locator('.preset-view').isVisible().catch(() => false)
console.log('远端浏览可见:', remoteVisible, '/ 配置页仍可见:', presetStillVisible)

if (!remoteVisible) {
  console.log('=== 复现成功：未跳转 ===')
  console.log(await page.locator('.main').innerHTML().catch(() => '(no .main)'))
} else {
  console.log('=== 未复现：跳转正常 ===')
}

await browser.close()
