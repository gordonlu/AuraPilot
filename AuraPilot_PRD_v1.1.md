# AuraPilot · 产品需求文档 (PRD) v1.1

> **文档状态**: Draft v1.1
> **作者**: 产品通
> **创建日期**: 2026-07-27
> **本次修订**: 2026-07-27（基于首轮深度反馈）
> **目标版本**: MVP v1.0
> **审阅人**: 待定（工程、设计、用户代表）

---

## 变更摘要（v1.0 → v1.1）

| # | 变更 | 类型 | 来源 |
|---|------|------|------|
| 1 | 北极星指标从"周活跃目录数"改为"周活跃任务操作数" | 调整 | 反馈 1.1 |
| 2 | Problem Statement 补充"AI Coding 重度试用者"画像 | 新增 | 反馈 1.2 |
| 3 | 明确 chokidar 在 `git pull` 后自动刷新机制 | 澄清 | 反馈 1.3 |
| 4 | yaml schema 3 处必改（additionalProperties / pr / commit） | 必改 | 反馈-协议 |
| 5 | 新增"宽松解析 + 严格展示"原则 | 新增 | 反馈-哲学 |
| 6 | 状态约束策略：正向校验+反向忽略 | 调整 | 反馈 2.2 |
| 7 | AGENTS.md "禁止同时领多任务" → "建议" | 调整 | 反馈 2.4 |
| 8 | git mv 跨平台兜底方案 | 补充 | 反馈 2.3 |
| 9 | P0 增加"删除任务"功能 | 补充 | 反馈 3.1 |
| 10 | P0 增加"编辑任务表单"（非 Monaco） | 补充 | 反馈 3.2 |
| 11 | P1 增加"阻塞聚焦视图" | 新增 | 反馈 3.3 |
| 12 | 新增"宽松模式（Lenient Mode）"概念 | 新增 | 反馈 4.1 |
| 13 | P0 提供 CLI 基础命令（init / status） | 决策 | 反馈 4.2 |
| 14 | 新增"空状态设计"P0 | 新增 | 反馈 4.3 |
| 15 | MVP 验证增加"5 个真实项目跑通全流程" | 补充 | 反馈 5 |
| 16 | 新增"关键假设（Key Assumptions）"板块 | 新增 | 反馈 6 |
| 17 | Q1-Q4 决策记录 | 决策 | 用户回答 |

---

## 0. 关键假设（Key Assumptions）

PRD 中隐含的核心假设及其验证方式。当某个假设被证伪时，应调整产品方向而非硬撑。

| # | 假设 | 验证方式 | 验证时间点 | 证伪后的应对 |
|---|------|---------|-----------|-------------|
| A1 | 目标用户确实同时维护 ≥ 3 个活跃 AI coding 项目 | 早期用户访谈 + 注册时填项目数 | MVP 上线前 | 若普遍 < 3，重新定位为单项目深度管理器 |
| A2 | AI Agent 会主动读 AGENTS.md 并按协议操作 | 协议遵循率监控 | MVP 上线后第 2 周 | 若 < 60%，强化 AGENTS.md 措辞 + 提供 Agent 提示词模板 |
| A3 | 用户愿意用 yaml 描述任务（而不是纯 UI 表单） | 手动 yaml 编辑占比 vs UI 操作占比 | MVP 上线后第 4 周 | 若手动编辑 > 30%，优化 UI 表单能力 |
| A4 | 分片文件 + git mv 能解决并发问题 | 真实冲突日志分析 | MVP 上线后第 2 周 | 若冲突频发，引入文件锁或乐观锁机制 |
| A5 | 跨项目看板是用户真正需要的视图（而非单项目视图） | 看板使用频率 vs 单项目视图使用频率 | MVP 上线后第 4 周 | 若单项目视图占主导，弱化跨项目聚合 |
| A6 | "宽松解析+严格展示"模型能让 Agent 自由发挥而不混乱 | 协议违规率 + 用户投诉 | MVP 上线后第 6 周 | 若混乱率高，引入严格模式开关 |

---

## 1. Problem Statement

Vibe coding 时代，独立开发者与小团队同时维护的 AI coding 项目数量从 1-2 个暴涨到 5-15 个。每个项目由不同的 AI Agent（Claude Code / Codex / Cursor / Gemini 等）开发，每天需要：跨仓库查看任务进度、为新需求撰写并下发 prompt、回收 Agent 工作结果、跟踪阻塞。

**当前痛点**：现有工具（Rover / AoE / Orca / Multica 等）都聚焦"单仓库内多 Agent 并行"，没人解决"跨仓库的需求管理与进度聚合"。开发者每天在 5-15 个仓库间切换，平均损耗 1-2 小时在"找状态、写 prompt、回收结果"的体力活上。

**目标用户画像**（v1.1 补充）：

1. **独立开发者 / 小团队负责人**（主画像）
   - 同时维护 5-15 个 AI coding 项目
   - 每天跨仓库跟进任务进度
   - 痛点：上下文切换成本高、需求遗忘、PR 堆积

2. **AI Coding 工具重度试用者**（v1.1 新增画像）
   - 每周换一个 Agent 试用（Claude Code → Codex → Gemini → Cursor...）
   - 同时在 2-3 个新项目上试错
   - 每个 Agent 交互方式都不同，需要"统一控制台"
   - 痛点：每换一个 Agent 就要重新学一套交互；试错项目状态散落难追踪

**不解决的代价**：项目失控、需求遗忘、决策延迟、试用成本浪费。

---

## 2. Goals

### 用户目标
1. **跨项目可视**：在一个看板上看到所有项目的任务状态、健康度、阻塞项
2. **协议化下发**：在 UI 里给某项目创建任务，自动写入对应仓库的 yaml，Agent 读取后执行
3. **零适配接入**：任何会读写 yaml + git 的 AI Agent 都能接入，无需中央协调

### 业务目标
1. **MVP 上线后 30 天内**：50 个早期用户在 200+ 项目中部署 `.aurapilot/`
2. **任务流转率**：用户每周通过 AuraPilot 创建/流转 ≥ 10 个任务
3. **协议遵循率**：Agent 正确使用 `git mv` 流转状态的比例 ≥ 85%

### 北极星指标（v1.1 调整）

**原指标**：周活跃 `.aurapilot/` 目录数
**新指标**：**周活跃任务操作数**（每周通过 UI 或协议发生的状态流转次数）

**调整理由**（来自反馈 1.1）：原指标存在"初始化 10 个项目但只用 2 个"的虚高问题。新指标直接对应"用户真的在用 AuraPilot 管理任务"，更精准反映产品价值。原指标降级为驱动指标（监测部署广度）。

---

## 3. Non-Goals

| 非目标 | 不做的理由 |
|--------|-----------|
| ❌ 不做自己的 AI Agent | BYO 已有 CLI，避免重复造轮子 |
| ❌ 不做多人实时协作 | MVP 单人场景已足够大，协作留 v2 |
| ❌ 不做云端托管 | 全本地优先，避免运维负担 |
| ❌ 不做代码质量评分 | 让 Agent/人 review，AuraPilot 只搬运结果 |
| ❌ 不做远程 GitHub API 集成 | 用户 `git push` 后状态文件自动同步 |
| ❌ 不做自动任务调度 | Agent 通过协议自取任务，无中央调度器 |
| ❌ 不做跨项目任务依赖图 | 复杂度过高，MVP 不验证 |

**关于"不做远程 GitHub API 集成"的澄清**（v1.1 补充，来自反馈 1.3）：

虽然不做主动远程拉取，但 AuraPilot 通过 `chokidar` 监听本地仓库目录变更。当用户在机器 B 执行 `git pull` 时，本地文件变化会触发 chokidar 事件，AuraPilot 自动刷新看板。**用户无需手动点击刷新**。这一机制在文档和 UI 中明确说明，避免开发时误解。

---

## 4. User Stories

### P0 用户故事

**US-1 跨项目看板**
> 作为**同时维护多个 AI coding 项目的开发者**，我希望**在一个看板上看到所有项目的任务状态分布（Backlog / In Progress / In Review / Done）**，以便**不再每天切 10 个仓库找状态**。

**US-2 注册本地项目**
> 作为**开发者**，我希望**把本地 clone 的项目目录注册到 AuraPilot**，以便**AuraPilot 自动扫描其 `.aurapilot/` 目录并加入看板**。

**US-3 协议初始化**
> 作为**开发者**，我希望**对一个尚未使用 AuraPilot 的项目执行"初始化"操作**，以便**自动在项目根目录创建 `.aurapilot/` 完整目录骨架 + AGENTS.md + schema.json**。

**US-4 创建任务**
> 作为**开发者**，我希望**在 UI 里给某项目创建一个任务（填标题/优先级/类型/描述/验收标准）**，以便**AuraPilot 自动写入 `.aurapilot/tasks/backlog/TASK-XXX.yaml`**。

**US-5 流转任务状态**
> 作为**开发者**，我希望**在 UI 里点击"领任务/完成/审核通过"，AuraPilot 自动执行 `git mv` 把 yaml 挪到对应状态目录**，以便**状态变更原子化、有 git 历史**。

**US-6 删除任务**（v1.1 新增，来自反馈 3.1）
> 作为**开发者**，我希望**在 UI 里删除一个任务（无论它在哪个状态目录）**，以便**处理需求取消、重复创建、测试数据**。

**US-7 编辑任务**（v1.1 新增，来自反馈 3.2）
> 作为**开发者**，我希望**双击任务卡片弹出编辑表单，修改 title/priority/type/desc/accept**，以便**不需要删了重建或手动改 yaml**。

**US-8 实时刷新**
> 作为**开发者**，我希望**当某个项目的 yaml 被 Agent 修改或 `git pull` 拉取新文件后，看板在 5 秒内自动刷新**，以便**不需要手动 F5**。

**US-9 CLI 快捷操作**（v1.1 新增，来自反馈 4.2）
> 作为**开发者**，我希望**在终端用 `aurapilot init` 和 `aurapilot status` 快速操作**，以便**初始化项目或查看状态时不需要打开 UI**。

**US-10 空状态引导**（v1.1 新增，来自反馈 4.3）
> 作为**首次打开 AuraPilot 的用户**，我希望**看到清晰的引导（添加项目 / 初始化流程图示 / 示例 demo）**，以便**不会面对空界面不知所措**。

### P1 用户故事

**US-11 内嵌 Monaco 编辑器**
> 作为**开发者**，我希望**在 AuraPilot UI 里直接编辑某个 yaml 文件（含 schema 校验）**，以便**处理复杂字段修改**。

**US-12 健康度信号灯**
> 作为**开发者**，我希望**每个项目显示 🟢/🟡/🔴 健康度，红灯项目置顶**，以便**优先处理卡住的项目**。

**US-13 阻塞高亮**
> 作为**开发者**，我希望**有 `blockers` 字段非空的任务在看板上特殊标记**，以便**一眼看到要介入的项**。

**US-14 阻塞聚焦视图**（v1.1 新增，来自反馈 3.3）
> 作为**开发者**，我希望**有一个视图只显示所有项目中 `blockers` 非空的任务并置顶**，以便**集中处理卡点**（与 US-12 项目级健康度互补：US-12 看项目，US-14 看任务）。

**US-15 任务历史**
> 作为**开发者**，我希望**点击任务能看其完整状态流转历史（git log --follow）**，以便**追溯 Agent 工作过程**。

### P2 用户故事（v1 不做）

- US-16 跨项目任务编号反查
- US-17 项目模板（new project 时选择 React/Node/Python 模板自动初始化）
- US-18 多人协作（共享看板、任务分配）
- US-19 协议版本升级迁移工具
- US-20 严格模式开关（关闭宽松解析，强制 schema 严格校验）

---

## 5. Requirements

### 5.1 协议规范

#### 5.1.1 目录结构

```
<项目根>/
└── .aurapilot/
    ├── AGENTS.md               # Agent 工作协议说明书
    ├── project.yaml            # 项目元数据
    ├── schema.json             # JSON Schema 强校验
    └── tasks/
        ├── backlog/            # 待办（PM 风格命名）
        ├── in-progress/        # 进行中
        ├── in-review/          # 待审核
        └── done/               # 已完成
```

**`.aurapilot/` 与 `.gitignore` 关系**（v1.1 决策，来自 Q2）：
- 默认**不**进入 `.gitignore`（状态文件应被 git 追踪，跨机器同步）
- 用户在初始化时可在 UI 中勾选"不提交到 git"选项，勾选则自动追加到 `.gitignore`

**验收标准**：
- Given 一个未初始化的项目目录
- When 用户在 AuraPilot 里点击"初始化 AuraPilot"
- Then `.aurapilot/` 完整目录骨架被创建，含 AGENTS.md / project.yaml / schema.json 和 4 个空状态目录
- And 默认不修改 `.gitignore`，但 UI 提供"加入 .gitignore"选项
- And 弹窗提示用户："状态文件默认会被 git 追踪，可在设置中修改"

#### 5.1.2 `project.yaml` Schema

```yaml
name: my-awesome-app          # 必填，项目标识
owner: gordon                  # 必填，所有者
health: green                  # 必填，枚举: green/yellow/red
sprint: 2026-W30               # 可选，迭代标识
notes: |                       # 可选，多行备注
  本周聚焦认证模块
schema_version: 1              # 必填，协议版本号
created: 2026-07-27            # 必填，初始化日期
```

#### 5.1.3 任务 yaml Schema（v1.1 修订 3 处）

文件路径：`.aurapilot/tasks/{state}/TASK-{NNN}.yaml`

```yaml
id: TASK-001
title: 实现邮箱登录
priority: P0
type: feature
created: 2026-07-25

# 以下字段按状态目录递进填写
assigned: Claude Code
branch: refactor/auth
started: 2026-07-27T10:30:00+08:00

pr: 42                                    # v1.1 改：统一 integer
waiting: gordon

completed: 2026-07-27T15:00:00+08:00
commit: a1b2c3d                           # v1.1 改：minLength/maxLength 校验

desc: |
  把 auth 逻辑从 controller 抽到 service 层。
accept:
  - 登录接口返回 access_token + refresh_token
log:                                      # v1.1 改：additionalProperties: true
  - ts: 2026-07-27T10:30:00+08:00
    msg: 创建分支 refactor/auth
    model: claude-3.5                     # Agent 可加扩展字段，UI 忽略
    tokens_used: 1234
blockers: []
```

**3 处必改详情**（来自反馈-协议）：

| 字段 | v1.0 | v1.1 | 理由 |
|------|------|------|------|
| `log.items.additionalProperties` | `false` | `true` | Agent 经常顺手加 `model`/`tokens`/`duration_ms` 等调试字段；强制 false 会导致"明明没写错但报违规"的挫败感 |
| `pr` | `["integer","string"]` | `integer, minimum: 1` | 允许字符串会迫使排序/链接跳转做类型判断；GitHub/GitLab PR 号本质都是数字 |
| `commit` | `pattern: ^[0-9a-f]{7,40}$` | `minLength:7, maxLength:40, pattern: ^[0-9a-f]+$` | ajv 严格模式下 `{7,40}` 量词可能报警；分离长度与字符集更稳妥 |

#### 5.1.4 解析器原则：宽松解析 + 严格展示（v1.1 新增）

**核心原则**（来自反馈-哲学问题，已采纳）：

> 字段的存在与否以"目录状态"为准，但不把"错误目录里的多余字段"视为违规，只是 UI 不展示它。

具体执行规则：

| 校验类型 | 行为 | 例子 |
|---------|------|------|
| **正向必填校验** | 缺失则报错 | `in-progress/` 下缺 `assigned` → ❌ 报错 |
| **反向多余字段** | 忽略，UI 不展示 | `backlog/` 下有 `assigned` → ⚠️ 不报错，UI 隐藏该字段 |
| **格式错误** | 报错 | `pr` 不是数字 → ❌ 报错 |
| **扩展字段** | 接受，UI 提示 | log 对象含 `model: claude-3.5` → ✅ 接受，UI 角落显示"含扩展元数据"灰色图标 |
| **文件位置违规** | 报错 | yaml 文件不在 4 个状态目录之一 → ❌ 报错 |

**为什么这样设计**：

实际 AI 工作流中，Agent 的逻辑常是"先在 `backlog/` 创建文件写好所有信息（含 assigned），然后执行 `git mv` 挪到 `in-progress/`"。如果这两个动作被拆成两次 git commit，中间会有几秒钟的"脏状态"。宽松解析让 Agent 写入顺序不影响可用性，严格展示保证 UI 始终准确。

这是 Postel's Law 在协议设计中的应用：*"be liberal in what you accept, conservative in what you send"*。

#### 5.1.5 状态流转规则

| 操作 | git 命令 | 触发字段更新 |
|------|---------|-------------|
| 领任务 | `git mv backlog/TASK-X.yaml in-progress/` | 添加 `assigned/branch/started` |
| 提交审核 | `git mv in-progress/TASK-X.yaml in-review/` | 可选添加 `pr/waiting` |
| 完成归档 | `git mv in-review/TASK-X.yaml done/` | 添加 `completed/commit` |
| 重新打开 | `git mv done/TASK-X.yaml backlog/` | 清除 `assigned/branch/started/completed/commit` |
| 删除任务 | `git rm tasks/{state}/TASK-X.yaml` | （无字段更新，文件删除） |

**跨平台兜底**（v1.1 补充，来自反馈 2.3）：

`git mv` 在 Windows 文件系统大小写不敏感时会触发"文件已存在"错误。AuraPilot 的状态变更封装层应：

```typescript
// 伪代码
async function safeGitMove(from: string, to: string) {
  try {
    await exec(`git mv ${from} ${to}`);
  } catch (e) {
    if (isCaseSensitivityError(e)) {
      // 兜底：先 git add -A，再 git rm --cached，再创建新文件
      await exec(`git add -A`);
      await exec(`git rm --cached ${from}`);
      await fs.copyFile(from, to);
      await fs.unlink(from);
      await exec(`git add ${to}`);
    } else {
      throw e;
    }
  }
}
```

#### 5.1.6 AGENTS.md 协议（v1.1 调整）

**v1.0 → v1.1 关键改动**：
- "禁止同时领多个任务" → "建议同时只领 1 个；如必须并行，每个任务需独立分支"（来自反馈 2.4）

```markdown
# AuraPilot Agent 协议

本项目使用 AuraPilot 协议管理开发任务。任何 AI coding agent 接入前必须读完本文档。

## 目录结构
- `.aurapilot/tasks/backlog/` - 待办
- `.aurapilot/tasks/in-progress/` - 进行中
- `.aurapilot/tasks/in-review/` - 待审核
- `.aurapilot/tasks/done/` - 已完成

## 工作流

### 1. 领任务
1. 列 `.aurapilot/tasks/backlog/` 目录下所有 yaml 文件
2. 解析每个文件，选 `assigned` 字段为空且 `priority` 最高的
3. 用 `git mv` 把文件挪到 `.aurapilot/tasks/in-progress/`
4. 编辑文件，添加：`assigned: <你的名字>`、`branch: <分支名>`、`started: <ISO 时间>`
5. 创建分支：`git checkout -b <branch>`

### 2. 执行任务
- 每完成里程碑，在 `log:` 数组追加 `{ts, msg}` 对象（只追加，不修改历史）
- 可在 log 对象中添加扩展字段（如 `model`、`tokens`、`duration_ms`），UI 会忽略展示
- 遇到阻塞，在 `blockers:` 数组追加描述
- **建议同时只领 1 个任务**；如必须并行，每个任务需独立分支

### 3. 提交审核
- 自测通过后
- 用 `git mv` 把文件挪到 `.aurapilot/tasks/in-review/`
- 可选：编辑文件添加 `pr: <PR编号>` 和 `waiting: <审核人>`
- commit message 必须包含 `TASK-XXX`

### 4. 归档
- PR 合并后
- 用 `git mv` 挪到 `.aurapilot/tasks/done/`
- 编辑文件添加 `completed: <ISO 时间>` 和 `commit: <hash 前 7 位>`

## 字段约定
- priority: P0(阻塞) / P1(本周) / P2(本月) / P3(可选)
- type: feature / bug / refactor / docs / test / chore
- health: green / yellow / red

## 协议宽松性
- 解析器对扩展字段宽容（你可以在 log 对象加任意调试字段）
- 字段格式必须正确（如 `pr` 必须是数字，`commit` 必须是 hex）
- 必填字段缺失会报错（如 `in-progress/` 下必须有 `assigned`）

## 禁止
- 不要修改 `done/` 下的历史文件
- 不要删除 `log:` 已有条目
- 不要跨状态目录直接挪文件（必须 `git mv`）
```

### 5.2 应用功能 Requirements

#### P0（MVP 必须实现）

**R-1 项目注册表**
- 用户可添加本地项目目录路径到 AuraPilot
- 路径必须含 `.aurapilot/` 才能成功注册（否则提示先初始化）
- 已注册项目可移除
- 项目列表持久化到本地配置文件 `~/.aurapilot/config.json`

**R-2 yaml 解析器（含宽松模式）**
- 使用 `js-yaml` 解析所有 yaml
- 使用 `ajv` 按 `schema.json` 校验，遵循 5.1.4 "宽松解析 + 严格展示"原则
- 解析失败的项目在 UI 上显示"⚠️ 解析失败"并高亮错误文件
- **宽松模式（Lenient Mode）**（v1.1 新增，来自反馈 4.1）：MVP 阶段默认启用
  - 必填字段缺失：warning 但不拒绝（UI 显示黄色 ⚠️）
  - 文件在非标准位置：警告但尝试解析
  - 用户可在设置中切换为"严格模式"

**R-3 跨项目看板**
- 4 列：Backlog / In Progress / In Review / Done
- 任务卡片显示：项目名、TASK-ID、标题、优先级徽章、`assigned` Agent 名
- `blockers` 非空的卡片红色边框
- 按项目分组折叠/展开
- 含扩展字段的任务卡片角落显示灰色"ℹ️"图标（hover 显示扩展字段列表）

**R-4 状态变更操作**
- UI 上每个任务卡片有"领任务/提交审核/完成"按钮
- 点击触发 `git mv`（含跨平台兜底）+ yaml 字段更新 + git commit
- 失败时回滚 + 显示错误

**R-5 任务 CRUD**（v1.1 重命名，含创建/编辑/删除）
- **创建任务**：UI 表单（项目、标题、优先级、类型、描述、验收标准）→ 生成 TASK-XXX → 写入 `backlog/` → git commit
- **编辑任务**（v1.1 新增，来自反馈 3.2）：双击任务卡片弹出编辑表单，可改 title/priority/type/desc/accept；提交后 git commit
- **删除任务**（v1.1 新增，来自反馈 3.1）：从任一状态目录中 `git rm` 并 git commit；删除前弹窗二次确认

**R-6 实时刷新**
- 使用 `chokidar` 监听所有已注册项目的 `.aurapilot/tasks/` 目录
- 文件变更/新建/删除触发增量解析
- `git pull` 拉取的文件变更同样触发刷新（v1.1 明确，来自反馈 1.3）
- 看板在 5 秒内反映变更
- UI 显示"最后刷新时间"

**R-7 CLI 工具**（v1.1 新增，来自反馈 4.2）
- `aurapilot init [path]`：在指定路径初始化 `.aurapilot/`
- `aurapilot status`：列出所有已注册项目及其任务统计
- `aurapilot add [path]`：注册项目到 AuraPilot
- CLI 与 UI 共享同一份 `~/.aurapilot/config.json`

**R-8 空状态设计**（v1.1 新增，来自反馈 4.3）
- 首次打开 AuraPilot 无任何已注册项目时显示：
  - "添加已有项目"按钮（引导注册）
  - "在现有项目中初始化 AuraPilot"的 3 步流程图示
  - "加载示例项目"按钮（一键 clone 内置 demo repo 并注册）

#### P1（v1.1 快速跟进）

**R-9 内嵌 Monaco 编辑器**：UI 里直接编辑 yaml，含 schema 校验提示
**R-10 健康度信号灯**：从 `project.yaml` 读取 `health` 字段，红灯项目置顶
**R-11 阻塞聚焦视图**（v1.1 新增，来自反馈 3.3）：切换视图只显示所有 `blockers` 非空任务
**R-12 任务历史时间线**：调用 `git log --follow` 渲染状态流转历史
**R-13 协议违规检测面板**：扫描所有任务，列出违规项（缺失必填/格式错误/位置错误）

#### P2（v2 远期）

- R-14 跨项目任务反查
- R-15 项目模板库
- R-16 多人协作
- R-17 协议版本迁移工具
- R-18 严格模式开关（关闭宽松解析）

---

## 6. Success Metrics

### 6.1 北极星指标（v1.1 调整）

**周活跃任务操作数**（每周通过 UI 或协议发生的状态流转次数）

测量方法：统计 `git log` 中含 `TASK-XXX` 的 commit 数（按周聚合）。

原指标"周活跃 .aurapilot/ 目录数"降级为驱动指标。

### 6.2 驱动指标（Leading）

| 指标 | 目标（上线 30 天） | 测量方法 |
|------|-------------------|---------|
| 项目初始化转化率 | 注册用户中 ≥ 60% 至少初始化 1 个项目 | 启动埋点 |
| 周活跃 `.aurapilot/` 目录数 | ≥ 200（原北极星，降级监测部署广度） | yaml 文件计数 |
| 跨项目看板日均访问 | DAU 中 ≥ 80% 每天打开看板 ≥ 1 次 | UI 埋点 |
| 任务创建数 | 每个活跃项目每周 ≥ 2 个新任务 | yaml 文件计数 |
| `git mv` 状态流转成功率 | ≥ 95%（失败重试不算） | 操作日志 |
| Agent 协议遵循率 | ≥ 85%（按 AGENTS.md 流程操作的比例） | yaml 字段合规性扫描 |
| CLI 使用率 | ≥ 30% 用户每周使用 CLI ≥ 1 次 | CLI 埋点 |

### 6.3 健康指标（Lagging）

| 指标 | 目标（上线 90 天） | 测量方法 |
|------|-------------------|---------|
| 周留存率 | ≥ 50% | 启动埋点 |
| 项目数 / 用户 | ≥ 3（用户真的在多项目管理场景使用） | 注册表统计 |
| 协议违规率 | ≤ 5% | yaml schema 校验 |
| 卸载率 | ≤ 20%（30 天内） | 卸载埋点 |

### 6.4 反指标（警惕）

- **单项目用户占比**：如果 ≥ 60% 用户只注册 1 个项目 → 定位失败信号（参考假设 A1）
- **手动 yaml 编辑占比**：如果 ≥ 30% 任务变更绕过 UI 直接改 yaml → UI 不好用信号（参考假设 A3）
- **宽松模式违规堆积**：如果宽松模式下违规率持续 > 20% → 协议设计有问题，需收紧

---

## 7. Open Questions

### 已决策（v1.1 来自用户回答）

| # | 问题 | 决策 | 来源 |
|---|------|------|------|
| Q1 | 开源协议 | **MIT** | 用户回答 |
| Q2 | `.aurapilot/` 是否进 `.gitignore` | **默认不进，用户可在初始化时选择加入** | 用户回答 |
| Q3 | Windows `git mv` 大小写敏感 | **后续工程阶段解答**（用户回答"后面有解答"） | 用户回答 |
| Q4 | 多机器共享同一项目时配置冲突 | **走 git 合并冲突流程，人介入处理** | 用户回答 |

### 阻塞性问题（开发前必须回答）

| # | 问题 | 负责人 |
|---|------|--------|
| Q5 | schema_version 升级时的迁移策略（自动 / 提示 / 拒绝） | 工程 |
| Q6 | pre-commit hook 如何在不污染用户仓库 hook 的情况下安装（husky? 单独 hook 文件?） | 工程 |
| Q7 | 内置 demo repo 的内容设计（哪些任务、哪些 Agent 风格） | 产品 + 设计 |

### 非阻塞性问题（开发中可解）

| # | 问题 | 负责人 |
|---|------|--------|
| Q8 | 协议推广策略：是否推动社区采纳为开放标准 | 产品 |
| Q9 | 严格模式开关的具体阈值（多少违规触发提示切换） | 产品 |
| Q10 | CLI 是否支持 Windows PowerShell 路径风格 | 工程 |

---

## 8. Timeline Considerations

### 8.1 MVP 路线图（2-3 周）

| 阶段 | 周次 | 交付物 | 验收 |
|------|------|--------|------|
| Phase 1 | Week 1 | yaml 解析器（含宽松模式）+ schema 校验 + 单项目看板渲染 | 能解析一个真实项目的 `.aurapilot/` 并渲染看板 |
| Phase 2 | Week 2 | 多项目注册表 + chokidar 监听 + 跨项目看板 + CLI 基础命令 | 能注册 5 个项目并实时刷新 |
| Phase 3 | Week 3 | 任务 CRUD（创建/编辑/删除）+ `git mv` 状态变更 + 空状态设计 + git log 历史 | 能在 UI 完成"创建 → 领任务 → 编辑 → 完成"全流程 |

### 8.2 MVP 验证标准（v1.1 补充，来自反馈 5）

**5 个真实项目（含 3 个不同 Agent 生成的）在 AuraPilot 中能跑通"创建 → 流转 → 完成"全流程，且不需要修改 Agent 的任何配置。**

这是对"零适配接入"承诺的硬验证。如果某个 Agent（比如 Gemini）写出的 yaml 格式与预期不同，说明 schema 需要做兼容性调整。

### 8.3 依赖与风险

- **依赖**：Tauri 2.0 稳定版、js-yaml 性能（大项目 100+ yaml 解析 < 500ms）、isomorphic-git 或 shell 调用 git 的稳定性
- **风险**：Agent 不遵循协议 → 通过宽松模式 + AGENTS.md 强化
- **里程碑**：v1.0 上线后 2 周收 5 个深度用户访谈，决定 v1.1 优先级

### 8.4 后续版本规划

- **v1.1**: P1 功能（健康度、内嵌编辑器、阻塞聚焦视图、历史时间线）
- **v1.2**: CLI 高级命令、协议违规自动修复、严格模式开关
- **v2.0**: 多人协作、协议开放标准化

---

## 9. 附录

### 9.1 完整 `.aurapilot/schema.json`（v1.1 修订版）

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://aurapilot.dev/schema/task.json",
  "title": "AuraPilot Task",
  "type": "object",
  "required": ["id", "title", "priority", "type", "created"],
  "properties": {
    "id": {"type": "string", "pattern": "^TASK-\\d{3,}$"},
    "title": {"type": "string", "minLength": 1, "maxLength": 120},
    "priority": {"enum": ["P0", "P1", "P2", "P3"]},
    "type": {"enum": ["feature", "bug", "refactor", "docs", "test", "chore"]},
    "created": {"type": "string", "format": "date"},
    "assigned": {"type": "string"},
    "branch": {"type": "string"},
    "started": {"type": "string", "format": "date-time"},
    "pr": {"type": "integer", "minimum": 1},
    "waiting": {"type": "string"},
    "completed": {"type": "string", "format": "date-time"},
    "commit": {
      "type": "string",
      "minLength": 7,
      "maxLength": 40,
      "pattern": "^[0-9a-f]+$"
    },
    "desc": {"type": "string"},
    "accept": {"type": "array", "items": {"type": "string"}},
    "log": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["ts", "msg"],
        "properties": {
          "ts": {"type": "string", "format": "date-time"},
          "msg": {"type": "string"}
        },
        "additionalProperties": true
      }
    },
    "blockers": {"type": "array", "items": {"type": "string"}}
  }
}
```

### 9.2 完整 `project.yaml` Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "AuraPilot Project",
  "type": "object",
  "required": ["name", "owner", "health", "schema_version", "created"],
  "properties": {
    "name": {"type": "string", "minLength": 1, "maxLength": 64},
    "owner": {"type": "string"},
    "health": {"enum": ["green", "yellow", "red"]},
    "sprint": {"type": "string"},
    "notes": {"type": "string"},
    "schema_version": {"type": "integer", "minimum": 1},
    "created": {"type": "string", "format": "date"}
  }
}
```

### 9.3 关键术语表

| 术语 | 定义 |
|------|------|
| AuraPilot 协议 | `.aurapilot/` 目录结构 + yaml schema + AGENTS.md 工作流的统称 |
| 状态目录 | `backlog/in-progress/in-review/done` 四个目录之一 |
| 状态流转 | 通过 `git mv` 把 yaml 在状态目录间移动 |
| 协议违规 | yaml 不符合 schema 或文件出现在错误状态目录 |
| 宽松解析 + 严格展示 | 解析器对扩展字段宽容，UI 严格按目录状态展示对应字段 |
| 正向校验 | 检查当前状态目录的必填字段是否缺失（缺失则报错） |
| 反向忽略 | 上层状态目录的多余字段不报错，UI 隐藏 |
| Lenient Mode | MVP 默认启用的宽松模式，缺失必填字段警告但不拒绝 |
| Agent 适配 | 任何会读写 yaml + git 的 AI Agent，无需 AuraPilot 写适配器 |

---

## 10. 变更记录

| 版本 | 日期 | 变更 | 作者 |
|------|------|------|------|
| v1.0 Draft | 2026-07-27 | 初版 PRD | 产品通 |
| v1.1 Draft | 2026-07-27 | 17 处修订（见变更摘要），含 yaml schema 3 处必改、宽松解析原则、Key Assumptions 板块 | 产品通 |

---

## 待审阅清单

- [ ] 工程负责人确认技术可行性（Tauri + chokidar + isomorphic-git + 跨平台 git mv 兜底）
- [ ] 设计负责人确认 UI 交互流程（含空状态、阻塞聚焦视图）
- [ ] 法务确认 MIT 协议
- [ ] 至少 3 位目标用户（含 1 位"重度试用者"画像）确认 PRD 解决了真实痛点
- [ ] 工程评估 MVP 工期（建议 2-3 周可行性）
- [ ] 解答阻塞性问题 Q5-Q7
