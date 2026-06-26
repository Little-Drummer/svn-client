// 冒烟：新建预设选行时高亮本地修改行，并支持一键选取修改行。
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
const page = await browser.newPage({ viewport: { width: 1180, height: 820 }, deviceScaleFactor: 2 })
await page.addInitScript(mockScript)
const fails = []
const check = (name, ok) => {
  console.log(ok ? `✓ ${name}` : `✗ ${name}`)
  if (!ok) fails.push(name)
}

await page.goto(BASE, { waitUntil: 'networkidle' })
await page.waitForTimeout(700)

await page.getByRole('tab', { name: '配置' }).click()
await page.waitForTimeout(250)
await page.locator('button:has-text("新建预设")').click()
await page.waitForTimeout(200)
await page.locator('button:has-text("选择文件")').click()
await page.waitForTimeout(300)
check('文件卡片出现', (await page.locator('.file-card').count()) === 1)

await page.locator('.mode-btn:has-text("指定行")').click()
await page.waitForTimeout(400)
check('行选择器出现', await page.locator('.line-picker').isVisible())
check('修改行高亮 2 行', (await page.locator('.pick-line.changed').count()) === 2)
check(
  '高亮的是第 4、5 行',
  (await page.locator('.pick-line.changed .ln').allInnerTexts()).join(',') === '4,5',
)
check('图例可见', await page.locator('.changed-legend').isVisible())

await page.locator('button:has-text("选取修改行")').click()
await page.waitForTimeout(200)
check('一键选取生成片段 4-5', (await page.locator('.chip').first().innerText()).includes('4-5'))
check('选中行有 picked 样式', (await page.locator('.pick-line.picked').count()) === 2)

await page.locator('.create-card').screenshot({ path: '/tmp/line-picker.png' })

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
