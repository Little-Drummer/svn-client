# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

基于 **Tauri 2 + Vue 3** 的桌面 SVN 客户端。后端通过 Rust 调用系统 `svn` 命令（不直接绑定 libsvn）。目标是提供比命令行更清晰的 SVN 工作流体验，而非简单包装命令行。

产品不复刻 Cornerstone 界面，交互设计应更现代、更直接，重点是：可扫描的状态、连续的工作流、长任务有明确进度和可重试入口、危险操作有预览和确认。

## 常用命令

```bash
# 启动完整 Tauri 开发模式（自动启动前端，推荐）
npm run tauri dev

# 仅启动前端开发服务器（无 Tauri 后端，用于纯前端调试）
npm run dev

# 构建生产版本（vue-tsc 类型检查 → 前端构建 → Tauri 打包）
npm run tauri build

# 仅前端构建（用于快速验证编译）
npm run build
```

## 架构说明

### 目录结构

```
src/           ← Vue 3 前端（TypeScript）
src-tauri/
  src/         ← Rust 后端
    main.rs    ← 入口，注册 Tauri commands
    lib.rs     ← 备用入口（移动端）
  tauri.conf.json
  capabilities/default.json  ← Tauri 权限配置
```

### 前后端通信

前端用 `invoke()` 调用后端，后端用 `#[tauri::command]` 暴露函数，所有 command 必须在 `main.rs` 的 `invoke_handler![]` 中注册。

长任务（checkout、update、commit）不能用普通 `invoke` 阻塞等待，应通过 Tauri Event 从 Rust 向前端推送进度流。

```
前端 invoke("svn_checkout", {...})
  └─ Rust 异步执行 svn checkout
       └─ 每行输出 emit("checkout-progress", line)
            └─ 前端监听 event，实时展示命令输出
```

### 前端模块划分

| 目录 | 职责 |
|------|------|
| `repositories` | 远端 SVN URL 管理（添加、编辑、删除、连接测试） |
| `workingCopies` | 本地工作副本管理（添加、识别、刷新状态） |
| `status` | 文件状态视图（modified/added/deleted/conflicted/unversioned） |
| `diff` | 差异查看（Monaco Editor，unified diff 或左右对比） |
| `commit` | 提交流程（选文件、写 message、检查冲突、展示结果） |
| `log` | 提交历史（分页、过滤、点击查看 diff） |
| `merge` | 合并流程（dry-run 预览、revision range、冲突结果） |
| `settings` | SVN 路径、认证、主题、忽略规则 |

### 后端模块划分

| 模块 | 职责 |
|------|------|
| `svn` | 封装系统 `svn` 命令调用，解析 XML 输出 |
| `commands` | Tauri command 入口（薄层，调用 svn 模块） |
| `models` | 前后端共享数据结构（serde 序列化） |
| `storage` | 持久化仓库列表、工作副本列表、用户配置 |
| `process` | 长任务管理、进度推送、命令输出流 |
| `errors` | 统一错误类型和错误信息 |

## 开发顺序（按优先级）

按照以下顺序推进，先把核心 SVN 工作流做扎实，再扩展远端浏览和高级能力。

### 第一阶段：核心 SVN 工作流（MVP）

1. **封装基础 SVN 命令**
   - `svn info --xml`：识别本地 working copy，获取 URL、revision、分支路径
   - `svn status --xml`：获取文件变更状态列表
2. **搭建主工作台布局**（左侧列表 + 顶部工具栏 + 中间内容区 + 右侧/底部详情面板 + 底部状态栏）
3. **本地工作副本管理**：添加已有本地项目、自动识别 SVN working copy、展示文件变更列表
4. **Diff 查看**：选中文件查看本地改动，Monaco Editor 展示，支持 unified / 左右对比，二进制文件给出提示
5. **Commit 流程**：选文件、写 message、提交前检查冲突、展示新 revision
6. **Update 操作**：整个工作副本或指定文件，支持指定 revision，显示更新结果（新增/修改/删除/冲突）
7. **Log 查看**：分页提交历史，显示 revision/作者/时间/message/变更文件，支持过滤
8. **Checkout 流程**：输入远端 URL、选本地目录、支持指定 revision、流式展示进度、完成后加入工作副本列表

### 第二阶段：核心增强

- 远端仓库浏览器（目录树、文件内容、历史、子目录 checkout）
- 分支与标签管理（识别 trunk/branches/tags、创建、switch、对比）
- Merge 流程（revision range、dry-run 预览、冲突展示）
- 冲突处理（mine/theirs/base、Monaco 合并视图、标记 resolved）
- Blame/Annotate、文件历史与版本对比

### 第三阶段：高级能力

- 多工作副本统一工作区（批量 update、统一状态视图）
- 认证与凭据管理（Keychain、SSH key、清除缓存）
- SVN Properties 查看/编辑（svn:ignore、svn:externals）
- 操作历史记录（命令输出、失败重试）

## 环境依赖

- 用户系统必须安装 `svn` 命令行工具（macOS：`brew install subversion`；Windows：TortoiseSVN 自带 CLI 或 SlikSVN；Linux：发行版包管理器）。
- 应用启动时应检测 `svn --version`，未安装时给出清晰的引导提示，不要静默失败。
- 允许用户在 `settings` 中自定义 `svn` 二进制路径，覆盖系统默认。

## 待决策事项（开发前需确认）

以下事项 AGENTS.md 未规定，需要在动手前选定：

| 事项 | 候选方案 | 备注 |
|------|---------|------|
| 状态管理 | Pinia / Composable + provide-inject | 模块多则用 Pinia，否则 composable 即可 |
| 路由 | Vue Router / Tab 切换组件 | 桌面工具型应用通常 Tab 切换更自然 |
| i18n | vue-i18n / 暂不引入（仅中文） | AGENTS.md 文案为中文，初期可不引入 |
| 主题 | Naive UI 暗色/亮色切换 | 跟随系统或手动切换 |
| 异步运行时 | tokio / async-std | Tauri 默认 tokio，沿用即可 |
| XML 解析 | quick-xml / serde-xml-rs | 推荐 quick-xml（性能好、支持 serde） |

## 关键实现约定

### SVN 命令调用

- **使用参数数组，不用字符串拼接**：`Command::new("svn").args(["info", "--xml", path])`，防止路径或 URL 中的空格、特殊字符引发注入。
- **统一加 `--xml` 或 `--non-interactive`**：避免 svn 弹出交互式认证提示卡住进程。
- **认证参数**：通过 `--username` `--password` 显式传递，不要依赖系统 keychain（Tauri 子进程环境隔离）。
- **输出编码**：Windows 上 svn 输出可能是 GBK 而非 UTF-8，解析前要检测编码或强制设置 `LANG=en_US.UTF-8`。
- **优先解析 XML**：`svn info/status/log` 都用 `--xml`，不要解析人类可读输出（不稳定且本地化差异大）。

### Tauri Command 命名

- Rust 端用 snake_case：`#[tauri::command] fn svn_status(...)`
- 前端 invoke 时用同样的 snake_case：`invoke("svn_status", { path })`
- 不要使用 `rename_all = "camelCase"`，保持前后端命名一致便于全局搜索。

### 错误处理

- Rust 端定义统一 `AppError` 枚举（svn 命令失败、IO、解析、认证等），实现 `serde::Serialize`，让 Tauri 自动转为前端 Promise reject。
- 命令失败时返回结构化错误（错误类型 + 原始 stderr + 退出码），不要只返回字符串，方便前端针对性处理（例如认证失败时弹登录框）。
- 前端 `try/catch` invoke 调用，统一在错误展示组件里渲染。

### 跨平台路径

- 路径在 Rust 端用 `std::path::PathBuf`，前后端传输用字符串（统一为正斜杠或保持系统原生形式），前端展示时再格式化。
- Windows 工作副本路径可能含反斜杠和盘符，写测试时两套样本都要覆盖。

### 配置与持久化

- 用 Tauri 的 `app_config_dir()` 存储仓库列表、工作副本列表、用户配置（JSON 即可）。
- 认证密码不能明文落盘，使用 `tauri-plugin-stronghold` 或系统 Keychain（后续阶段引入）。
- 配置文件 schema 变更时要写迁移逻辑，不能让旧版用户启动崩溃。

### Monaco Editor 集成

- Vite + Monaco 需要单独配置 worker（参考 `vite-plugin-monaco-editor` 或手动 `?worker` import），否则 diff 视图会因 worker 加载失败而白屏。
- 大文件（>1MB）diff 要做分片或截断提示，避免主线程卡死。

### 长任务进度

- 通过 Tauri Event 推送，前端展示命令输出流，操作完成/失败后给出明确结果和可重试入口。
- 每个长任务分配唯一 task_id，前端按 task_id 订阅事件，支持多任务并行。

### 危险操作

- revert、delete、switch、merge 前端必须提供预览和二次确认，不能直接触发。
- 提供"显示等价命令行"功能，让高级用户确认实际执行的命令。

### Tauri 权限模型

- `src-tauri/capabilities/default.json` 当前只开了 `core:default`。
- 新增文件系统访问、shell 调用等能力时需在此追加 permission，否则前端调用会被拒绝。
- shell 执行 svn 命令应使用 Rust `std::process::Command`（已通过自定义 command 暴露），不需要开启 `shell:allow-execute` 给前端。

## 代码风格

遵循 `~/.claude/CLAUDE.md` 中的全局个人规则。本项目特别强调：

- 注释解释"为什么"，不解释"做了什么"，简短自然。
- 日志描述使用中文。
- Vue SFC 用 `<script setup lang="ts">` 风格。
- Rust 端遵循 `cargo fmt` 默认风格。
- TypeScript 已开启 `noUnusedLocals` `noUnusedParameters` `noFallthroughCasesInSwitch`，不要用下划线变通绕过。
