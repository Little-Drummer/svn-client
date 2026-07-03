# SVN Client

一个基于 **Tauri 2 + Vue 3 + TypeScript + Rust** 的桌面 SVN 客户端。

项目目标不是简单包一层 `svn` 命令行，而是在保留 SVN 核心能力的基础上，提供更现代、更清晰的桌面工具体验：工作副本状态洞察、差异查看、日志追踪、提交 / 更新 / 检出流程，以及围绕真实团队工作流的多级合并、增量打包和配置预设能力。

## 技术栈

- 桌面框架：Tauri 2
- 前端：Vue 3（`<script setup>`）+ TypeScript + Vite
- 状态管理：Pinia
- UI：Tailwind CSS 4 + reka-ui（shadcn-vue 风格组件）+ lucide-vue-next
- Diff / 代码查看：Monaco Editor（自定义 svn-light / svn-dark 主题）
- 大列表渲染：@tanstack/vue-virtual
- 后端：Rust 调用系统 `svn` 命令（不绑定 libsvn），quick-xml 解析 XML 输出，tokio 异步运行时

## 功能总览

### 环境与配置

- 启动时检测本机 `svn` 是否可用，不可用时给出引导提示。
- macOS 下自动探测 svn 安装位置（Homebrew / MacPorts / 系统路径），解决 GUI 应用不继承终端 PATH 的问题。
- 支持手动指定 SVN 可执行文件路径。
- 仓库列表、工作副本列表、用户配置持久化到 Tauri 应用配置目录。
- 窗口大小和位置记忆。

### 远端仓库

- 添加、编辑、删除远端 SVN 仓库配置，测试连接。
- 远端目录树浏览（侧边栏内联展开 + 内容区详情），查看远端文件文本内容。
- 从远端浏览直接发起检出（弹窗内完成，自动预填目录）。

### 本地工作副本

- 添加已有 working copy，通过 `svn info --xml` 自动识别。
- 项目分组：按 `work/{项目}/{环境}/{模块}` 目录约定把扁平工作副本聚合成项目视图，支持扫描项目根目录批量添加。
- 展示路径、仓库 URL、revision，支持自定义显示名。
- 工作副本路径不可用（磁盘未挂载、目录移动）时给出明确提示和重新检查入口。
- 浏览本地文件树、创建文件夹、在访达 / 终端中打开。

### 文件状态与操作

- `svn status --xml` 展示 modified / added / deleted / conflicted / unversioned / missing 等状态，支持流式加载。
- 批量 `add` / `delete` / `revert` / `ignore`，危险操作有确认。
- `svn cleanup` 修复锁死的工作副本。

### Diff 与历史

- Monaco Editor 展示本地改动、任意 revision 的 diff，支持查看 BASE 与历史版本内容。
- `svn log --xml` 提交历史：limit、revision range、作者、日期、关键词筛选，展示变更文件并可查看对应 diff。

### Commit / Update / Checkout

- Commit：选文件、写 message（按工作副本自动保存草稿）、提交前检查冲突、展示新 revision。
- Update：整个工作副本或指定路径，支持指定 revision，归类展示更新结果。
- Checkout：指定 URL、目录、revision、凭据，流式展示进度。
- 所有长任务经任务中心统一管理：实时输出流、成功 / 失败状态、退出码、可取消。

### 工作流套件（团队定制能力）

- **多级合并**：按 develop → test → produce 路由合并，基于 mergeinfo eligible 计算待合并版本，自动过滤双向同步提交、压缩连续版本号、生成公司格式合并日志；目标副本有未提交改动时自动走搁置 → 更新 → 合并 → 提交 → 恢复流程。
- **增量打包**：从 `svn log -v` 提取版本区间内的变更文件，java → class 映射（含内部类），生成 `1.x.YYMMDD(REV)` 版本号文件并 zip 打包，支持提交版本文件。
- **配置预设**：捕获 / 预览 / 应用一组本地开发配置文件，切环境不再手改配置。
- **工作流看板**：项目卡片总览分支矩阵与本地改动，一键跳转合并 / 打包 / 配置。

## 环境要求

- Node.js 20+（vite 8 构建要求）
- Rust（1.77+）与 Tauri 2 系统依赖
- Subversion 命令行工具（macOS：`brew install subversion`）

```bash
svn --version   # 确认 svn 可用
```

## 开发与构建

```bash
npm install          # 安装依赖
npm run tauri dev    # 启动完整桌面应用（推荐）
npm run dev          # 仅前端 Vite 服务（无后端能力）
npm run build        # vue-tsc 类型检查 + 前端构建（快速验证编译）
npm run tauri build  # 生成桌面安装包
```

## 项目结构

```text
.
├── src/                       # Vue 前端
│   ├── api/svn.ts             # Tauri command 调用封装
│   ├── components/            # 业务组件（CommitPanel、DiffViewer、TaskCenter…）
│   │   └── ui/                # reka-ui 基础组件（shadcn-vue 风格）
│   ├── composables/           # Monaco 配置、toast、确认框等组合式逻辑
│   ├── stores/                # Pinia：repositories / workingCopies / status / tasks
│   ├── types/svn.ts           # 前端类型定义
│   └── views/                 # 主视图
│       ├── MainLayout.vue     # 工作台骨架 + Tab 切换
│       ├── WorkflowBoardView  # 总览看板
│       ├── StatusView         # 文件状态
│       ├── LogView            # 提交历史
│       ├── MergeView          # 多级合并
│       ├── PackageView        # 增量打包
│       ├── ConfigPresetView   # 配置预设
│       ├── RemoteBrowserView  # 远端浏览
│       └── CheckoutView       # 检出（弹窗内使用）
├── src-tauri/                 # Rust 后端
│   └── src/
│       ├── lib.rs             # 入口，invoke_handler 注册全部 command
│       ├── commands/          # Tauri command 入口（薄层）
│       ├── svn/               # SVN 能力模块
│       │   ├── runner.rs      # 命令执行封装
│       │   ├── streaming.rs   # 长任务输出流
│       │   ├── info / status / log / diff / list
│       │   ├── merge.rs       # 多级合并流程
│       │   ├── package.rs     # 增量打包
│       │   ├── preset.rs      # 配置预设
│       │   └── project.rs     # 项目分组与扫描
│       ├── models.rs          # 前后端共享数据结构
│       ├── storage/           # 配置持久化 + svn 路径探测
│       ├── process/           # 任务注册表与取消
│       └── errors.rs          # 统一错误类型
└── package.json
```

## 界面布局

- 左侧：远端仓库列表 + 本地工作副本列表（含项目分组、改动速览）。
- 顶部：工作副本面包屑 + 功能 Tab（总览 / 状态 / 历史 / 合并 / 打包 / 配置）+ 刷新。
- 内容区：对应 Tab 视图；选中远端仓库时切换为远端浏览。
- 底部状态栏：svn 环境健康状态 + 任务中心。

## 路线图

按优先级排序：

1. **合并 / 打包流程在真实项目上的验证**，以及合并前目标副本脏检查防护。
2. **冲突处理**：mine / theirs / base 三方视图、Monaco 合并编辑、标记 resolved。
3. **分支管理**：switch、创建分支 / tag、分支对比。
4. **搁置（shelve）独立功能** 与 **回滚提交**（reverse merge）。
5. Blame / Annotate、SVN Properties 查看编辑、凭据接入系统 Keychain。
6. 体验增强：文件变动自动刷新状态、提交消息历史、补丁导入导出、长任务系统通知、Windows svn 路径自动探测。

## 贡献约定

- Rust 新增 SVN 能力放入 `src-tauri/src/svn` 独立模块，经 `commands` 暴露，并在 `lib.rs` 注册。
- 前端按领域拆分（repositories / workingCopies / status / log / diff / commit / merge…）。
- 提交消息使用中文；注释解释"为什么"，简短自然。
- 工程细节约定见 `CLAUDE.md`，产品规格与实现状态见 `AGENTS.md`。
