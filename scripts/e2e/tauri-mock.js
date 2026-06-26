// Playwright 注入脚本：在纯浏览器环境里模拟 Tauri v2 的 __TAURI_INTERNALS__，
// 让前端跑通启动与基本交互流程，用于无后端的 UI 回归。
(() => {
  let callbackId = 1
  const callbacks = new Map()

  const wc = (id, path, url, root, available = true) => ({
    id,
    path,
    url,
    repositoryRoot: root,
    revision: 100,
    lastSeenAt: null,
    relativeUrl: null,
    displayName: null,
    available,
  })

  const ROOT = 'https://svn.example.com/repo'
  const workingCopies = [
    wc('wc-1', '/Volumes/work/demo/develop/rest', `${ROOT}/branches/develop/rest`, ROOT),
    wc('wc-2', '/Volumes/work/demo/produce/rest', `${ROOT}/trunk/rest`, ROOT),
    wc('wc-3', '/Volumes/gone/demo2/develop/rest', `${ROOT}/branches/develop/rest`, ROOT, false),
  ]

  const projects = [
    {
      name: 'demo',
      branches: [
        {
          environment: 'develop',
          modules: [
            { module: 'rest', workingCopyId: 'wc-1', path: '/Volumes/work/demo/develop/rest', url: `${ROOT}/branches/develop/rest` },
          ],
        },
        {
          environment: 'produce',
          modules: [
            { module: 'rest', workingCopyId: 'wc-2', path: '/Volumes/work/demo/produce/rest', url: `${ROOT}/trunk/rest` },
          ],
        },
      ],
    },
  ]

  const handlers = {
    svn_check_environment: () => 'svn, version 1.14.3 (mock)',
    get_svn_bin: () => 'svn',
    list_repositories: () => [
      { id: 'repo-1', name: '测试仓库', url: ROOT, username: null, lastAccessedAt: null },
    ],
    list_working_copies: () => workingCopies,
    list_projects: () => projects,
    list_config_presets: () => [
      {
        id: 'preset-1',
        projectName: null,
        name: '本地开发配置',
        files: [
          {
            relPath: 'src/main/resources/application-dev.yml',
            content: 'a\nurl: dev\nb\n',
            fragments: [
              { startLine: 2, endLine: 2, lines: ['url: dev'], contextBefore: ['a'], contextAfter: ['b'] },
            ],
          },
          { relPath: 'src/main/java/ConstantsSystem.java', content: 'whole file', fragments: [] },
        ],
      },
    ],
    preview_config_preset: () => [
      {
        relPath: 'src/main/resources/application-dev.yml',
        action: 'patch',
        detail: '替换 1 处行片段',
        oldLines: ['url: prod'],
        newLines: ['url: dev'],
      },
      {
        relPath: 'src/main/java/ConstantsSystem.java',
        action: 'overwrite',
        detail: '整文件覆盖',
        oldLines: [],
        newLines: [],
      },
    ],
    apply_config_preset: () => [
      {
        relPath: 'src/main/resources/application-dev.yml',
        action: 'patch',
        detail: '替换 1 处行片段',
        oldLines: ['url: prod'],
        newLines: ['url: dev'],
      },
      {
        relPath: 'src/main/java/ConstantsSystem.java',
        action: 'overwrite',
        detail: '整文件覆盖',
        oldLines: [],
        newLines: [],
      },
    ],
    merge_list_routes: () => [],
    package_fetch_revisions: () => [],
    svn_get_status: () => [],
    svn_get_status_stream: () => 'task-mock-1',
    svn_get_info: () => ({ url: ROOT, repositoryRoot: ROOT, revision: 100 }),
    svn_list_remote: () => [
      { name: 'trunk', path: 'trunk', url: `${ROOT}/trunk`, kind: 'dir', size: null, revision: 100, author: 'dev', date: null },
    ],
    read_file_text: () =>
      'server:\n  port: 8080\ndb:\n  url: jdbc:kingbase://localhost\n  user: dev\nlogging: info\n',
    svn_get_diff: () =>
      [
        'Index: application-dev.yml',
        '===================================================================',
        '--- application-dev.yml\t(revision 100)',
        '+++ application-dev.yml\t(working copy)',
        '@@ -1,6 +1,6 @@',
        ' server:',
        '   port: 8080',
        ' db:',
        '-  url: jdbc:prod',
        '-  user: prod',
        '+  url: jdbc:kingbase://localhost',
        '+  user: dev',
        ' logging: info',
        '',
      ].join('\n'),
    'plugin:dialog|open': () => [
      '/Volumes/work/demo/develop/rest/src/main/resources/application-dev.yml',
    ],
  }

  window.__TAURI_MOCK_CALLS__ = []
  window.__TAURI_MOCK_ERRORS__ = []
  window.addEventListener('error', (e) => {
    window.__TAURI_MOCK_ERRORS__.push(String(e.error?.stack || e.message))
  })
  window.addEventListener('unhandledrejection', (e) => {
    window.__TAURI_MOCK_ERRORS__.push('unhandledrejection: ' + String(e.reason?.stack || e.reason))
  })

  window.__TAURI_INTERNALS__ = {
    metadata: {
      currentWebview: { label: 'main' },
      currentWindow: { label: 'main' },
    },
    transformCallback(cb, once) {
      const id = callbackId++
      callbacks.set(id, { cb, once })
      return id
    },
    unregisterCallback(id) {
      callbacks.delete(id)
    },
    convertFileSrc(p) {
      return p
    },
    async invoke(cmd, args) {
      window.__TAURI_MOCK_CALLS__.push(cmd)
      if (cmd === 'plugin:event|listen') return callbackId
      if (cmd === 'plugin:event|unlisten') return null
      const h = handlers[cmd]
      if (h) return h(args)
      // 未知命令：记录并以可识别错误拒绝，避免静默吞掉
      window.__TAURI_MOCK_ERRORS__.push('unmocked invoke: ' + cmd)
      throw { kind: 'other', message: `mock 未实现命令 ${cmd}` }
    },
  }
})()
