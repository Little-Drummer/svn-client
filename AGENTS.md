# SVN Client 项目上下文

## 基本要求

1. 前后端项目可以执行打包命令来验证，不受限制。

## 项目定位

这是一个基于 Tauri + Vue 的桌面 SVN 客户端。Cornerstone 只是产品参考，不需要完全照着 Cornerstone 复制功能和交互。

产品应该在 SVN 核心能力扎实的基础上，加入更现代的桌面工具体验：更清晰的工作流、更强的状态洞察、更好的批量操作、更友好的冲突处理和更可追踪的命令执行过程。

核心目标：

1. 能管理远端 SVN 仓库。
2. 能 checkout 远端仓库项目到本地。
3. 能分析本地 SVN 工作副本。
4. 能查看提交日志、文件差异和历史版本。
5. 能执行 update、commit、merge、revert、resolve 等常用 SVN 操作。
6. 提供清晰的文件状态、冲突处理、合并和日志查看体验。
7. 用现代客户端的思路降低 SVN 的使用复杂度，而不是简单包装命令行。

## 当前技术栈

- 桌面框架：Tauri 2
- 前端框架：Vue 3
- 构建工具：Vite
- 语言：TypeScript + Rust
- UI 组件：Naive UI
- 代码 / Diff 查看：Monaco Editor

## 产品界面方向

主界面应采用桌面工具型工作台布局，不做营销型首页。

推荐结构：

- 左侧：远端仓库 / 本地工作副本列表
- 顶部：常用操作工具栏，包括 Checkout、Update、Commit、Log、Merge、Refresh
- 中间：当前项目文件树或状态列表
- 右侧或底部：详情面板，包括 Diff、Log detail、Commit files、Console output
- 底部：状态栏，显示当前项目 URL、revision、分支、操作状态

界面风格应偏专业工具软件，信息密度适中，重点是可扫描、可操作、状态清晰。

不要完全复刻 Cornerstone 的界面。可以参考它的 SVN 工作流覆盖范围，但交互设计应更现代、更直接。

现代化方向：

- 首页可以是工作区 Dashboard，集中展示最近项目、待提交变更、冲突、更新状态和最近操作。
- 文件状态视图应支持快速搜索、状态分组、批量选择和常用操作快捷入口。
- Diff / Log / Commit 可以形成连续工作流，用户从变更文件能自然进入 diff、提交、查看历史。
- 长任务需要有明确的进度、命令输出、成功 / 失败结果和可重试入口。
- 对危险操作，例如 revert、delete、switch、merge，应提供明确预览和确认。
- 可以加入“操作队列”或“任务中心”，统一展示 checkout、update、commit、merge 等后台任务。
- 可以加入收藏项目、项目标签、最近分支、常用 SVN URL 等效率功能。
- 可以提供命令行等价命令展示，方便高级用户理解软件实际执行了什么。
- 对新手用户减少 SVN 术语压力，对高级用户保留足够细节。

## 一期 MVP 功能

优先实现最常用的 SVN 工作流。

### 仓库管理

- 添加远端 SVN URL
- 保存仓库名称、地址、用户名
- 连接测试
- 展示最近访问仓库
- 编辑 / 删除仓库配置

### Checkout

- 输入远端 URL
- 选择本地目录
- 支持指定 revision
- 显示 checkout 进度和命令输出
- checkout 完成后自动加入本地工作副本列表

### 本地工作副本管理

- 添加已有本地 SVN 项目
- 自动识别是否为 SVN working copy
- 展示项目路径、URL、当前 revision、分支路径
- 支持刷新状态

### 文件状态视图

- 展示 modified、added、deleted、conflicted、unversioned、missing 等状态
- 支持按状态筛选
- 支持 add、delete、revert
- 支持忽略文件

### Diff 查看

- 选中文件后查看本地改动
- 使用 Monaco Editor 做差异展示
- 支持 unified diff 或左右对比
- 支持查看某个 revision 的差异
- 对二进制文件给出明确提示

### Commit

- 选择要提交的文件
- 输入 commit message
- 提交前检查冲突
- 显示提交结果和新 revision
- 保存最近提交消息草稿

### Update

- 更新整个工作副本
- 更新指定文件或目录
- 支持更新到指定 revision
- 显示新增、修改、删除、冲突等更新结果

### Log

- 查看项目提交历史
- 支持分页加载
- 显示 revision、作者、时间、message、变更文件
- 支持按作者、日期、关键词筛选
- 点击 revision 可以查看对应改动 diff

## 二期核心增强

### 远端仓库浏览器

- 浏览 SVN URL 下的目录树
- 查看远端文件内容
- 查看远端目录历史
- 支持 checkout 某个子目录
- 支持创建目录、删除、重命名、复制

### 分支与标签

- 识别 trunk、branches、tags
- 创建 branch
- 创建 tag
- switch 工作副本到其他分支
- 对比两个分支路径

### Merge

- 从分支合并到当前工作副本
- 支持选择 revision range
- merge 前预览变更
- merge 后展示本地变更和冲突
- 支持 dry-run

### 冲突处理

- 展示冲突文件列表
- 展示 mine、theirs、base 信息
- 使用 Monaco Editor 做合并视图
- 支持标记 resolved
- 支持使用 mine / theirs 快速解决

### Blame / Annotate

- 查看每一行最后修改人、revision、时间
- 点击 revision 跳转日志
- 用于排查代码历史

### 文件历史

- 查看单个文件的 revision 历史
- 查看旧版本内容
- 对比任意两个 revision
- 支持恢复某个历史版本

## 三期高级能力

### 批量项目工作区

- 多个 working copy 统一管理
- 一键刷新所有项目状态
- 一键 update 多个项目
- 按项目分组显示待提交改动

### 认证与凭据

- 支持用户名密码
- 支持系统 Keychain / Credential Store
- 支持 SSH key 场景
- 支持清除认证缓存

### SVN Properties

- 查看 / 编辑 svn:ignore
- 查看 / 编辑 svn:externals
- 查看 mime-type、eol-style 等属性

### Externals 支持

- 展示 externals 依赖
- 单独 update external
- 检查 external 是否异常

### 操作历史

- 记录每次 SVN 命令
- 查看命令输出
- 失败操作可重试
- 用于排查问题

## 技术实现建议

第一版后端建议通过 Rust 调用系统 `svn` 命令，不直接绑定 libsvn。

优先封装这些命令：

- `svn info --xml`
- `svn status --xml`
- `svn log --xml`
- `svn diff`
- `svn checkout`
- `svn update`
- `svn commit`
- `svn merge --dry-run`
- `svn resolve`

原因：

- 实现速度快
- 行为接近用户命令行环境
- 跨平台成本较低
- 更容易调试和展示命令输出

后续如果需要更深集成，再考虑绑定 SVN native library。

## 推荐前端模块拆分

- `repositories`：远端仓库管理
- `workingCopies`：本地工作副本管理
- `status`：文件状态
- `log`：提交历史
- `diff`：差异查看
- `commit`：提交流程
- `merge`：合并流程
- `settings`：SVN 路径、认证、主题、忽略规则

## 推荐后端模块拆分

- `svn`：封装 SVN 命令执行
- `commands`：Tauri command 入口
- `models`：前后端共享的数据结构
- `storage`：保存仓库列表、工作副本列表、用户配置
- `process`：长任务、进度、命令输出流
- `errors`：统一错误类型和错误信息

## 推荐开发顺序

1. 封装 `svn info` 和 `svn status`，先能识别本地 working copy。
2. 搭建主工作台布局。
3. 实现添加本地项目、刷新状态、展示文件变更。
4. 接入 diff 查看。
5. 实现 commit / update。
6. 实现 log。
7. 实现 checkout。
8. 再做远端仓库浏览、merge、冲突处理。

这个顺序的核心原则是：先把“打开一个本地 SVN 项目，看见改了什么，能提交和更新”做扎实，再扩展远端浏览、分支、合并和高级能力。
