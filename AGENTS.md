# AGENTS.md — SVN Client 产品规格与实现状态

面向所有 AI Agent 的项目上下文：这个产品是什么、做到哪了、接下来做什么。工程约定（命令调用、错误处理、命名、权限等）见 `CLAUDE.md`，两份文档配合阅读。

## 基本要求

1. 前后端可以随时执行构建命令验证（`npm run build` / `cargo check`），不受限制。
2. 提交消息使用中文。

## 产品定位

基于 Tauri 2 + Vue 3 的桌面 SVN 客户端。Cornerstone 只是能力范围参考，不复刻其界面；交互应更现代、更直接。核心思路是用现代客户端降低 SVN 的使用复杂度，而不是包装命令行。

产品同时服务两类场景：

1. **通用 SVN 工作流**：仓库管理、检出、状态、diff、提交、更新、日志、远端浏览。
2. **用户真实团队工作流**（差异化能力）：一次完整交付 = 检出 → 改本地开发配置 → 开发提交 → 多级合并（develop → test → produce，含 updatesql）→ 打增量包。其中合并与打包的业务规则移植自两个 Python 脚本（`~/.hermes/skills/devops/svn-merge-code`、`svn-incremental-package`），**脚本是公司约定的权威参照，修改相关逻辑必须逐函数对照，不凭记忆重写**。

## 实现状态

### 已完成 ✅

**通用 SVN 能力（一期 MVP 全部落地）**

- 仓库管理：增删改、连接测试、远端目录树浏览（侧边栏内联展开）、远端文件查看、从远端发起检出。
- 工作副本：添加/识别/刷新/自定义显示名、路径不可用检测、本地文件树、访达/终端打开。
- 状态视图：status --xml（含流式加载）、批量 add/delete/revert/ignore、cleanup。
- Diff：本地改动、任意 revision、BASE/历史版本内容，Monaco 渲染。
- Commit：选文件、消息草稿（localStorage 按工作副本隔离）、冲突预检、新 revision 展示。
- Update / Checkout：指定 revision、流式进度、任务中心统一管理（task_id 事件流、可取消）。
- Log：limit/revision range/作者/日期/关键词筛选、变更文件、关联 diff。
- 环境：svn 检测与引导、macOS 自动探测 svn 路径、自定义二进制路径。

**团队工作流套件（全部落地）**

- 项目分组（`svn/project.rs`）：按 `work/{项目}/{环境}/{模块}` 启发式聚合，扫描批量添加。
- 多级合并（`svn/merge.rs` + MergeView）：合并路由配置、mergeinfo eligible 计算、双向同步提交过滤、连续版本号压缩、公司格式合并日志、脏副本搁置→更新→合并→提交→恢复。
- 增量打包（`svn/package.rs` + PackageView）：log -v 提取变更、java→class 映射（含内部类 `Foo$1`）、version 文件 `1.x.YYMMDD(REV)`、zip 打包、版本文件提交。
- 配置预设（`svn/preset.rs` + ConfigPresetView）：捕获/预览/应用本地开发配置文件组。
- 工作流看板（WorkflowBoardView）：项目卡片、分支矩阵、按需改动检查、携带项目上下文跳转合并/打包/配置。

### 已知风险 / 未验证项 ⚠️

- merge 的 commit 提交整个 target 工作副本，非 sync 路由下 target 预存改动会被带上（缺脏检查防护）。
- merge / package 全流程仅通过编译和构建验证，**未在真实项目 runtime 实测**。
- 仓库凭据存储未接系统 Keychain。

### 未实现 ❌

冲突处理（resolve / 三方视图）、分支管理（switch / 创建分支 tag / 分支对比）、Blame、SVN Properties 通用查看编辑（仅有 ignore 特例）、独立搁置功能、回滚提交、补丁导入导出、锁管理、Externals 管理、文件变动自动刷新、Windows svn 路径探测。

## 路线图（按优先级）

1. **真实项目验证合并 / 打包流程**，并给 merge 加 target 脏检查防护（不干净时强制搁置或阻断）。
2. **冲突处理**：冲突文件列表、mine / theirs / base 三方信息、Monaco 合并视图、标记 resolved、mine/theirs 快速解决。commit 前已能检出冲突，但用户目前只能回终端解决，这是工作流最后的断点。
3. **分支管理**：`svn switch`、`svn copy` 创建分支/tag、两分支对比。多级合并已在用分支概念，切换却要回命令行。
4. **低成本高收益项**：
   - 搁置独立功能化——merge.rs 已有完整 shelve/restore 实现，抽出来暴露成用户可用的"暂存改动"。
   - 回滚提交——Log 中选 revision 一键 reverse merge（`svn merge -c -REV`），复用危险操作确认机制。
5. **补全**：Blame / Annotate（Monaco 行级标注）、SVN Properties（至少 svn:ignore、svn:externals 可视化）、凭据接入 Keychain（`keyring` crate）。
6. **体验与健壮性**：
   - notify crate 监听工作副本自动刷新 status（防抖 + 过滤 `.svn`）。
   - 提交消息历史 / 公司格式模板。
   - 长任务完成系统通知（tauri-plugin-notification 已装未用）。
   - Windows svn 路径自动探测（对齐 macOS 的实现）。
   - svn XML 解析（info/status/log）单元测试——拿真实 XML 输出做 fixture；合并的版本号压缩、提交过滤等移植逻辑加对照测试。

## 界面结构（现状）

- 左侧：远端仓库列表 + 工作副本列表（项目分组、改动速览 popover）。
- 顶部：面包屑 + segmented Tab（总览 / 状态 / 历史 / 合并 / 打包 / 配置）+ 刷新。
- 内容区：Tab 对应视图，force-mount 保持挂载；选中远端仓库时远端浏览接管。
- 底部：svn 环境健康状态 + 任务中心；检出在 Dialog 中完成。

界面风格偏专业工具软件：信息可扫描、操作路径短、状态反馈明确。对新手减少 SVN 术语压力，对高级用户保留细节（等价命令行展示）。

## 设计原则（做新功能时遵守）

- 长任务必须走 task_id 事件流 + 任务中心，不用普通 invoke 阻塞等待。
- 危险操作（revert / delete / switch / merge / 回滚）必须有预览和二次确认。
- Diff / Log / Commit 形成连续工作流：从变更文件自然进入 diff → 提交 → 历史。
- 优先解析 `--xml` 输出；所有 svn 调用带 `--non-interactive`。
- 新增后端能力：`src-tauri/src/svn/` 独立模块 → `commands/mod.rs` 薄层 → `lib.rs` 注册 → `src/api/svn.ts` 封装 → 组件消费。
