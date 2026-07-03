# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

基于 **Tauri 2 + Vue 3** 的桌面 SVN 客户端。后端通过 Rust 调用系统 `svn` 命令（不直接绑定 libsvn）。目标是提供比命令行更清晰的 SVN 工作流体验，而非简单包装命令行。

除通用 SVN 能力外，项目还原生整合了团队定制工作流：多级合并（develop → test → produce）、增量打包、本地配置预设、工作流看板。产品规格、实现状态和路线图见 `AGENTS.md`。

交互设计重点：可扫描的状态、连续的工作流、长任务有明确进度和可取消入口、危险操作有预览和确认。

## 常用命令

```bash
# 启动完整 Tauri 开发模式（自动启动前端，推荐）
npm run tauri dev

# 仅启动前端开发服务器（无 Tauri 后端，用于纯前端调试）
npm run dev

# 构建生产版本（vue-tsc 类型检查 → 前端构建 → Tauri 打包）
npm run tauri build

# 仅前端构建（用于快速验证编译，含 vue-tsc 类型检查）
npm run build

# Rust 后端编译检查
cargo check --manifest-path src-tauri/Cargo.toml
```

注意：vite 8 构建要求 Node 20+，默认 node@18 会崩。

## 已定技术选型

以下决策已落地，不要重新引入替代方案：

| 事项 | 选定方案 |
|------|---------|
| 状态管理 | Pinia（stores：repositories / workingCopies / status / tasks） |
| 路由 | 无 Vue Router，MainLayout 内 Tab 切换（force-mount 保持挂载） |
| UI 组件 | Tailwind CSS 4 + reka-ui（shadcn-vue 风格，`src/components/ui/`），**不是 Naive UI** |
| i18n | 不引入，仅中文 |
| 异步运行时 | tokio |
| XML 解析 | quick-xml（serde serialize feature） |
| 编码处理 | encoding_rs（Windows GBK 输出） |
| 打包压缩 | zip crate（deflate） |

## 架构说明

### 目录结构

```
src/                 ← Vue 3 前端（TypeScript）
  api/svn.ts         ← 所有 invoke 调用的统一封装
  views/             ← MainLayout + 8 个功能视图
  components/        ← 业务组件；ui/ 下是 reka-ui 基础组件
  stores/            ← Pinia
src-tauri/src/
  lib.rs             ← 入口，invoke_handler![] 注册全部 command（main.rs 只是壳）
  commands/mod.rs    ← 全部 Tauri command（薄层，调用 svn 模块）
  svn/               ← SVN 能力模块（见下表）
  models.rs          ← 前后端共享数据结构（serde）
  storage/           ← 配置持久化 + svn 二进制路径探测
  process/           ← 长任务注册表、取消
  errors.rs          ← AppError 统一错误类型
```

### 前后端通信

前端用 `invoke()` 调用后端，后端用 `#[tauri::command]` 暴露函数，所有 command 必须在 `lib.rs` 的 `invoke_handler![]` 中注册。前端一律经 `src/api/svn.ts` 封装调用，不在组件里直接 `invoke`。

长任务（checkout、update、commit）通过 Tauri Event 从 Rust 推送进度流，每个任务分配唯一 task_id，前端 tasks store + TaskCenter 统一订阅展示，支持取消（`svn_cancel_task`）。

### 后端 svn 模块划分

| 模块 | 职责 |
|------|------|
| `runner.rs` | 命令执行封装（参数数组、编码、错误包装） |
| `streaming.rs` | 长任务输出流式推送 |
| `info / status / log / diff / list` | 对应 svn 子命令 + XML 解析 |
| `merge.rs` | 多级合并：路由、mergeinfo eligible、同步提交过滤、版本号压缩、公司格式日志、脏副本搁置流程 |
| `package.rs` | 增量打包：log -v 提取变更、java→class 映射、版本文件、zip |
| `preset.rs` | 本地开发配置文件预设（捕获/预览/应用） |
| `project.rs` | 按 `work/{项目}/{环境}/{模块}` 启发式分组工作副本、扫描批量添加 |
| `shelve.rs` | 手动搁置（类似 git stash）：补丁保存/列表/恢复/删除；共享 wc_status_text / has_versioned_changes 给 merge 和 rollback |
| `conflict.rs` | 文本冲突三方内容读取（svn info --xml 的 conflict 节点）、svn resolve |
| `rollback.rs` | 回滚提交：反向合并指定 revision，脏副本先走搁置 |

### 前端视图与 Tab 对应

MainLayout 顶部 Tab：总览（WorkflowBoardView）/ 状态（StatusView）/ 历史（LogView）/ 合并（MergeView）/ 打包（PackageView）/ 配置（ConfigPresetView）。选中左侧远端仓库时 RemoteBrowserView 接管内容区；检出（CheckoutView）在 Dialog 中打开。看板通过 overrideProject 机制跳转合并/打包/配置并携带项目上下文。

## 关键实现约定

### SVN 命令调用

- **使用参数数组，不用字符串拼接**：`Command::new(svn_bin).args([...])`，防注入。svn 二进制路径统一从 `ConfigState::svn_bin()` 取（含 macOS 自动探测），不要硬编码 `"svn"`。
- **统一加 `--xml` 和 `--non-interactive`**：避免交互式认证提示卡住进程；优先解析 XML，不解析人类可读输出。
- **认证参数**：`--username` `--password` 显式传递，不依赖系统 keychain（Tauri 子进程环境隔离）。
- **输出编码**：Windows 上 svn 输出可能是 GBK，用 encoding_rs 处理，不假设 UTF-8。

### Tauri Command 命名

- Rust 端与前端 invoke 统一用 snake_case，**不用 `rename_all = "camelCase"`**，便于全局搜索。

### 错误处理

- 后端统一返回 `AppResult<T>`（`errors.rs` 的 AppError，实现 Serialize），带错误类型 + stderr + 退出码，不要只返回字符串。
- 前端 try/catch invoke，经 use-error-toast 统一展示。

### 跨平台路径

- Rust 端用 `PathBuf`，前后端传字符串。Windows 路径含反斜杠和盘符，测试要覆盖两套样本。

### 配置与持久化

- 用 `app_config_dir()` 存 JSON（仓库、工作副本、配置预设、合并路由）。合并搁置文件在 `app_config_dir/shelves`。
- 认证密码不能明文落盘，后续接系统 Keychain（`keyring` crate）。
- 配置 schema 变更要写迁移逻辑，不能让旧版用户启动崩溃。

### Monaco Editor

- worker 已在 `composables/monaco-setup.ts` 配置，主题在 `monaco-theme.ts`（svn-light / svn-dark），新增编辑器场景复用这两个模块。
- 大文件（>1MB）diff 做截断提示，避免主线程卡死。

### 危险操作

- revert、delete、switch、merge 必须有预览和二次确认（用 `use-confirm-dialog`）。
- 提供"显示等价命令行"入口，让高级用户确认实际执行的命令。

### Tauri 权限模型

- 已启用插件：log、dialog、window-state、notification。
- 新增文件系统/shell 能力需在 `src-tauri/capabilities/default.json` 追加 permission。svn 命令走 Rust `std::process::Command`，不给前端开 shell 权限。

## 已知风险（改动相关模块前必读）

- **合并的脏副本防护链路**：merge_execute 前自动搁置 target 版本化修改（二进制拒绝）→ update → merge → 用户检查后提交 → 自动恢复搁置，另有撤销合并入口。改 `merge.rs` 或 `shelve.rs` 时不要破坏这条链路（搁置目录：`app_config_dir/shelves/{会话id}`，手动搁置在 `shelves/manual/{id}`）。
- 合并/打包逻辑移植自两个 Python 脚本（`~/.hermes/skills/devops/svn-merge-code`、`svn-incremental-package`），脚本是公司约定的权威参照，修改对应逻辑时逐函数对照脚本，不凭记忆重写。多级合并与增量打包已在真实项目实测正常（2026-07-03）。
- 冲突处理 / 手动搁置 / 回滚提交（2026-07-03 落地）尚未在真实场景 runtime 实测。

## 代码风格

遵循 `~/.claude/CLAUDE.md` 中的全局个人规则。本项目特别强调：

- 注释解释"为什么"，不解释"做了什么"，简短自然。
- 日志描述使用中文；提交消息使用中文。
- Vue SFC 用 `<script setup lang="ts">` 风格。
- Rust 端遵循 `cargo fmt` 默认风格。
- TypeScript 已开启 `noUnusedLocals` `noUnusedParameters` `noFallthroughCasesInSwitch`，不要用下划线变通绕过。
