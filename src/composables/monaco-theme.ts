/**
 * Monaco 自定义主题：svn-light / svn-dark
 *
 * 设计目标：让 diff 视图的颜色、行高亮、滚动条、选区与 app 的 design token 完全对齐，
 * 避免在白底 mat-content 上突然出现 monaco 默认的灰边和饱和红绿。
 *
 * 注意：monaco 主题只接受 #RRGGBB / #RRGGBBAA 格式，不能用 CSS var，
 * 所以这里把 style.css 的 token 值用 hex 重新表达一遍。
 */

import * as monaco from 'monaco-editor'

const LIGHT_BG = '#fbfbfc'
const LIGHT_FG = '#2c2c2e'
const LIGHT_FG_MUTED = '#6e6e73'
const LIGHT_FG_SUBTLE = '#98989d'
const LIGHT_ACCENT = '#007aff'
const LIGHT_SUCCESS = '#34c759'
const LIGHT_DANGER = '#ff3b30'

const DARK_BG = '#232428'
const DARK_FG = '#e3e3e6'
const DARK_FG_MUTED = '#9a9aa1'
const DARK_FG_SUBTLE = '#6f6f76'
const DARK_ACCENT = '#0a84ff'
const DARK_SUCCESS = '#32d74b'
const DARK_DANGER = '#ff453a'

export const LIGHT_THEME = 'svn-light'
export const DARK_THEME = 'svn-dark'

const lightTheme: monaco.editor.IStandaloneThemeData = {
  base: 'vs',
  inherit: true,
  rules: [
    { token: '', foreground: LIGHT_FG.slice(1) },
    { token: 'comment', foreground: LIGHT_FG_SUBTLE.slice(1), fontStyle: 'italic' },
    { token: 'string', foreground: '0a7d4f' },
    { token: 'number', foreground: 'b15c00' },
    { token: 'keyword', foreground: 'a000a0' },
    { token: 'type', foreground: '0b6cc7' },
    // 'diff' 语言下的 token：unified 文本模式直接用到
    { token: 'inserted', foreground: LIGHT_SUCCESS.slice(1) },
    { token: 'deleted', foreground: LIGHT_DANGER.slice(1) },
    { token: 'header', foreground: LIGHT_FG_MUTED.slice(1), fontStyle: 'bold' },
    { token: 'meta', foreground: LIGHT_ACCENT.slice(1) },
  ],
  colors: {
    'editor.background': LIGHT_BG,
    'editor.foreground': LIGHT_FG,
    'editor.lineHighlightBackground': '#00000008',
    'editor.lineHighlightBorder': '#00000000',
    'editorCursor.foreground': LIGHT_ACCENT,
    'editor.selectionBackground': '#007aff33',
    'editor.inactiveSelectionBackground': '#007aff1a',
    'editor.selectionHighlightBackground': '#007aff1f',
    'editor.wordHighlightBackground': '#007aff1a',
    'editorLineNumber.foreground': LIGHT_FG_SUBTLE,
    'editorLineNumber.activeForeground': LIGHT_FG_MUTED,
    'editorIndentGuide.background1': '#00000014',
    'editorIndentGuide.activeBackground1': '#00000026',
    'editorWhitespace.foreground': '#98989d40',
    'editorGutter.background': LIGHT_BG,
    'editorGutter.addedBackground': LIGHT_SUCCESS,
    'editorGutter.deletedBackground': LIGHT_DANGER,
    'editorGutter.modifiedBackground': LIGHT_ACCENT,
    // diff editor —— 整行 + 字符两层
    'diffEditor.insertedLineBackground': '#34c75914',
    'diffEditor.insertedTextBackground': '#34c75924',
    'diffEditor.removedLineBackground': '#ff3b3014',
    'diffEditor.removedTextBackground': '#ff3b3024',
    'diffEditor.border': '#00000012',
    'diffEditorGutter.insertedLineBackground': '#34c75922',
    'diffEditorGutter.removedLineBackground': '#ff3b3022',
    'diffEditorOverview.insertedForeground': '#34c75966',
    'diffEditorOverview.removedForeground': '#ff3b3066',
    // 滚动条 / 装饰
    'scrollbar.shadow': '#00000000',
    'scrollbarSlider.background': '#2c2c2e29',
    'scrollbarSlider.hoverBackground': '#2c2c2e47',
    'scrollbarSlider.activeBackground': '#2c2c2e66',
    'minimap.background': LIGHT_BG,
    'editorOverviewRuler.border': '#00000012',
    // 弹层
    'editorWidget.background': '#fcfcfd',
    'editorWidget.border': '#0000001f',
    'editorSuggestWidget.background': '#fcfcfd',
    'editorSuggestWidget.border': '#0000001f',
    'editorSuggestWidget.selectedBackground': '#007aff1f',
    focusBorder: LIGHT_ACCENT,
    'editorBracketMatch.background': '#007aff1f',
    'editorBracketMatch.border': '#007aff66',
    'editor.findMatchBackground': '#ff9f0a4d',
    'editor.findMatchHighlightBackground': '#ff9f0a26',
  },
}

const darkTheme: monaco.editor.IStandaloneThemeData = {
  base: 'vs-dark',
  inherit: true,
  rules: [
    { token: '', foreground: DARK_FG.slice(1) },
    { token: 'comment', foreground: DARK_FG_SUBTLE.slice(1), fontStyle: 'italic' },
    { token: 'string', foreground: '7ee2a8' },
    { token: 'number', foreground: 'f5b452' },
    { token: 'keyword', foreground: 'd594ff' },
    { token: 'type', foreground: '7cb9ff' },
    { token: 'inserted', foreground: DARK_SUCCESS.slice(1) },
    { token: 'deleted', foreground: DARK_DANGER.slice(1) },
    { token: 'header', foreground: DARK_FG_MUTED.slice(1), fontStyle: 'bold' },
    { token: 'meta', foreground: DARK_ACCENT.slice(1) },
  ],
  colors: {
    'editor.background': DARK_BG,
    'editor.foreground': DARK_FG,
    'editor.lineHighlightBackground': '#ffffff0a',
    'editor.lineHighlightBorder': '#00000000',
    'editorCursor.foreground': DARK_ACCENT,
    'editor.selectionBackground': '#0a84ff4d',
    'editor.inactiveSelectionBackground': '#0a84ff29',
    'editor.selectionHighlightBackground': '#0a84ff29',
    'editor.wordHighlightBackground': '#0a84ff26',
    'editorLineNumber.foreground': DARK_FG_SUBTLE,
    'editorLineNumber.activeForeground': DARK_FG_MUTED,
    'editorIndentGuide.background1': '#ffffff14',
    'editorIndentGuide.activeBackground1': '#ffffff2e',
    'editorWhitespace.foreground': '#6f6f7666',
    'editorGutter.background': DARK_BG,
    'editorGutter.addedBackground': DARK_SUCCESS,
    'editorGutter.deletedBackground': DARK_DANGER,
    'editorGutter.modifiedBackground': DARK_ACCENT,
    'diffEditor.insertedLineBackground': '#32d74b1f',
    'diffEditor.insertedTextBackground': '#32d74b33',
    'diffEditor.removedLineBackground': '#ff453a1f',
    'diffEditor.removedTextBackground': '#ff453a33',
    'diffEditor.border': '#ffffff14',
    'diffEditorGutter.insertedLineBackground': '#32d74b29',
    'diffEditorGutter.removedLineBackground': '#ff453a29',
    'diffEditorOverview.insertedForeground': '#32d74b80',
    'diffEditorOverview.removedForeground': '#ff453a80',
    'scrollbar.shadow': '#00000000',
    'scrollbarSlider.background': '#9a9aa133',
    'scrollbarSlider.hoverBackground': '#9a9aa15c',
    'scrollbarSlider.activeBackground': '#9a9aa180',
    'minimap.background': DARK_BG,
    'editorOverviewRuler.border': '#ffffff14',
    'editorWidget.background': '#2f3034',
    'editorWidget.border': '#ffffff1f',
    'editorSuggestWidget.background': '#2f3034',
    'editorSuggestWidget.border': '#ffffff1f',
    'editorSuggestWidget.selectedBackground': '#0a84ff33',
    focusBorder: DARK_ACCENT,
    'editorBracketMatch.background': '#0a84ff33',
    'editorBracketMatch.border': '#0a84ff80',
    'editor.findMatchBackground': '#ffb34066',
    'editor.findMatchHighlightBackground': '#ffb34033',
  },
}

let registered = false
export function registerSvnThemes() {
  if (registered) return
  registered = true
  monaco.editor.defineTheme(LIGHT_THEME, lightTheme)
  monaco.editor.defineTheme(DARK_THEME, darkTheme)
}

export function currentSvnTheme(): string {
  const dark =
    typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches
  return dark ? DARK_THEME : LIGHT_THEME
}
