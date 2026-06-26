// 冒烟：配置预设视图渲染、应用预览对话框、确认应用调用链。
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

await page.getByRole('tab', { name: '配置' }).click()
await page.waitForTimeout(300)

check('配置页可见', await page.locator('.preset-view').isVisible())
check('预设卡片渲染', await page.locator('.preset-card').count() === 1)
check(
  '片段行号标注',
  (await page.locator('.frag-tag').first().innerText()).includes('2'),
)
check(
  '目标下拉含项目分组',
  (await page.locator('.preset-card optgroup[label="demo"] option').count()) >= 2,
)

// 预览应用
await page.locator('.preset-card button:has-text("预览应用")').click()
await page.waitForTimeout(400)
check('预览对话框打开', await page.locator('.preview-dialog').isVisible())
check('patch 计划展示', await page.locator('.plan-badge.act-patch').count() === 1)
check('旧行展示', (await page.locator('.d-old').first().innerText()).includes('url: prod'))
check('新行展示', (await page.locator('.d-new').first().innerText()).includes('url: dev'))

await page.locator('.preview-dialog button:has-text("确认应用")').click()
await page.waitForTimeout(400)
const calls = await page.evaluate(() => window.__TAURI_MOCK_CALLS__)
check('调用了 preview_config_preset', calls.includes('preview_config_preset'))
check('调用了 apply_config_preset', calls.includes('apply_config_preset'))
check('对话框已关闭', !(await page.locator('.preview-dialog').isVisible().catch(() => false)))

// 新建预设表单基本交互
await page.locator('button:has-text("新建预设")').click()
await page.waitForTimeout(200)
check('新建卡片出现', await page.locator('.create-card').isVisible())
check('来源副本分组下拉', (await page.locator('.create-card optgroup').count()) >= 1)

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
