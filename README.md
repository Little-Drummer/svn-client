# SVN Client

一个基于 **Tauri 2 + Vue 3 + TypeScript + Rust** 的桌面 SVN 客户端。

项目目标不是简单包一层 `svn` 命令行，而是在保留 SVN 核心能力的基础上，提供更现代、更清晰的桌面工具体验：工作副本状态洞察、差异查看、日志追踪、提交 / 更新 / 检出流程，以及可追踪的命令输出。

## 项目定位

SVN Client 面向仍在使用 Subversion 的团队和个人开发者，重点解决这些日常工作流：

- 管理远端 SVN 仓库地址。
- 检出远端仓库到本地目录。
- 添加并识别本地 SVN working copy。
- 查看本地文件状态和改动差异。
- 执行 `add`、`delete`、`revert`、`ignore` 等常用文件操作。
- 查看提交历史、修订版本详情和变更文件。
- 执行 `update`、`commit`、`checkout` 等长任务，并展示命令输出。
- 在界面中保留 SVN 命令细节，方便高级用户排查问题。

## 技术栈

- 桌面框架：Tauri 2
- 前端框架：Vue 3
- 构建工具：Vite
- 前端语言：TypeScript
- 后端语言：Rust
- 状态管理：Pinia
- UI 基础：Tailwind CSS + Reka UI 风格组件
- 图标：lucide-vue-next
- Diff / 代码查看：Monaco Editor
- SVN 能力：Rust 后端调用系统 `svn` 命令

## 当前功能

### 环境与配置

- 检测本机 `svn` 命令是否可用。
- 支持读取和设置 SVN 可执行文件路径。
- 使用 Tauri 本地配置保存仓库和工作副本信息。

### 远端仓库

- 添加、编辑、删除远端 SVN 仓库配置。
- 保存仓库名称、URL、用户名。
- 测试远端仓库连接。
- 浏览远端目录树。
- 查看远端文件文本内容。
- 从仓库入口进入检出流程。

### 本地工作副本

- 添加已有本地 SVN working copy。
- 通过 `svn info --xml` 自动识别工作副本。
- 展示工作副本路径、仓库 URL、repository root、revision。
- 刷新工作副本元信息。
- 浏览本地工作副本文件树。
- 创建本地文件夹。

### 文件状态与操作

- 通过 `svn status --xml` 展示文件状态。
- 支持 modified、added、deleted、conflicted、unversioned、missing 等 SVN 状态。
- 支持批量 `add`、`delete`、`revert`、`ignore`。
- 对危险操作在前端保留确认入口。

### Diff 查看

- 通过 `svn diff` 查看本地改动。
- 支持读取 BASE 内容和当前文件内容。
- 支持查看指定 revision 的 diff。
- 前端使用 Monaco Editor 展示差异。

### Commit / Update / Checkout

- Commit 前检查冲突文件。
- 支持选择文件并输入提交消息。
- 支持 update 到 HEAD 或指定 revision。
- 支持 checkout 指定 URL、目标目录、revision、用户名和密码。
- 长任务通过事件流展示 stdout、stderr、完成状态和退出码。

### Log

- 通过 `svn log --xml` 查看提交历史。
- 支持 limit、revision range、作者、日期、关键词筛选。
- 展示 revision、作者、时间、message 和变更路径。

## 开发环境要求

请先安装：

- Node.js
- npm
- Rust
- Tauri 2 所需系统依赖
- Subversion 命令行工具，也就是可执行的 `svn`

确认 SVN 可用：

```bash
svn --version
```

## 安装依赖

```bash
npm install
```

## 本地开发

启动 Tauri 桌面应用：

```bash
npm run tauri dev
```

只启动前端 Vite 服务：

```bash
npm run dev
```

## 构建

生成前端产物：

```bash
npm run build
```

生成桌面应用安装包：

```bash
npm run tauri build
```

## 项目结构

```text
.
├── src/                    # Vue 前端源码
│   ├── api/                # Tauri command 调用封装
│   ├── components/         # 通用组件和业务组件
│   ├── composables/        # Vue 组合式逻辑
│   ├── stores/             # Pinia 状态管理
│   ├── types/              # 前端类型定义
│   └── views/              # 主视图：状态、日志、远端、检出
├── src-tauri/              # Tauri / Rust 后端源码
│   ├── src/
│   │   ├── commands/       # Tauri command 入口
│   │   ├── errors/         # 错误类型
│   │   ├── models/         # 前后端共享数据模型
│   │   ├── process/        # 长任务和命令输出事件
│   │   ├── storage/        # 本地配置存储
│   │   └── svn/            # SVN 命令封装与 XML 解析
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
└── vite.config.ts
```

## 后端实现思路

当前后端优先通过 Rust 调用系统 `svn` 命令，保证行为接近用户的命令行环境，也便于将等价命令和输出暴露给界面。

已封装的主要命令包括：

- `svn info --xml`
- `svn status --xml`
- `svn log --xml`
- `svn diff`
- `svn cat`
- `svn list --xml`
- `svn add`
- `svn delete`
- `svn revert`
- `svn propset svn:ignore`
- `svn update`
- `svn commit`
- `svn checkout`

长耗时命令会通过任务事件返回输出，前端可以展示执行过程、成功 / 失败结果和退出码。

## 前端工作台布局

当前界面采用桌面工具型工作台：

- 左侧：远端仓库列表、本地工作副本列表。
- 顶部：当前工作副本面包屑、主功能切换、刷新入口。
- 中间：状态、历史、远端浏览、检出视图。
- 底部：SVN 环境状态和版本信息。

整体设计目标是信息可扫描、操作路径短、状态反馈明确。

## 后续规划

一期继续完善：

- 更完整的提交文件选择和提交消息草稿。
- 更细的更新结果归类。
- 二进制文件 diff 提示。
- 更稳定的远端浏览和检出联动。
- 更清晰的命令输出与失败重试。

二期核心增强：

- 分支 / 标签识别与切换。
- Merge dry-run、预览和冲突结果展示。
- 冲突解决视图。
- 文件历史、旧版本内容和任意 revision 对比。
- Blame / Annotate。
- SVN properties 管理。

三期高级能力：

- 多 working copy 批量刷新和批量 update。
- 认证与凭据管理。
- Externals 展示和单独 update。
- 操作历史与任务中心。

## 贡献约定

- 代码风格优先保持现有模块边界和组件组织方式。
- Rust 后端新增 SVN 能力时，优先放入 `src-tauri/src/svn` 下独立模块，再从 `commands` 暴露给前端。
- 前端新增业务能力时，优先按 `repositories`、`workingCopies`、`status`、`log`、`diff`、`commit` 等领域拆分。
- 关键逻辑可以添加简洁中文注释，说明意图即可。
- 提交消息使用中文。
