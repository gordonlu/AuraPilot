# AuraPilot Bootstrap

你正在为当前代码仓库安装 AuraPilot 协议。

AuraPilot 是一个基于仓库文件的 AI Coding 任务管理协议。安装完成后，用户和不同的 Coding Agent 可以通过 `.aurapilot/` 目录创建、领取、更新和完成任务。

请严格按照本文执行安装。不要在安装过程中执行任何业务任务。

---

## 1. 安装原则

安装过程必须满足以下原则：

1. 仅修改当前代码仓库，不修改用户全局配置。
2. 不覆盖、删除或重写已有的 Agent 指令。
3. 不安装插件、MCP Server、Git Hook、后台服务或额外依赖。
4. 不执行 `git commit`、`git push` 或远程操作。
5. 不领取或执行 `.aurapilot/tasks/` 中的任务。
6. 所有修改必须可审计、可重复执行、可安全回滚。
7. 如果仓库已安装 AuraPilot，应执行检查或升级，不得重复插入配置。

---

## 2. 检查现有环境

首先检查：

* 当前仓库根目录；
* 是否已经存在 `.aurapilot/`；
* 是否已经存在 `.aurapilot/installation.yaml`；
* 仓库中已有的 Agent 指令文件；
* 当前 Agent 实际支持的仓库级持久化指令机制。

可能存在的指令文件包括但不限于：

* `AGENTS.md`
* `CLAUDE.md`
* `GEMINI.md`
* `.github/copilot-instructions.md`
* `.cursor/rules/*.mdc`
* 当前 Agent 官方支持的其他仓库级规则文件

不要仅根据文件名猜测当前 Agent。优先使用你确定支持的持久化指令机制。

如果无法确定当前 Agent 支持哪种持久化指令文件，默认使用仓库根目录下的 `AGENTS.md`，并在最终报告中提示用户确认其 Agent 是否会自动读取该文件。

---

## 3. 创建 AuraPilot 目录

如果 `.aurapilot/` 不存在，创建以下结构：

```text
.aurapilot/
├── AGENTS.md
├── project.yaml
├── schema.json
├── installation.yaml
└── tasks/
    ├── backlog/
    ├── in-progress/
    ├── in-review/
    └── done/
```

如果目录已经存在，不要删除或覆盖现有任务文件。

---

## 4. 创建项目元数据

如果 `.aurapilot/project.yaml` 不存在，创建基础内容：

```yaml
name: <根据仓库目录或项目文件推断>
owner: <无法可靠确定时填写 unknown>
health: green
schema_version: 1
created: <当前日期，YYYY-MM-DD>
```

只填写可以可靠推断的信息。不要编造所有者、团队或项目背景。

如果文件已经存在，保留现有内容。

---

## 5. 创建任务协议

如果 `.aurapilot/AGENTS.md` 不存在，创建一份 AuraPilot Agent 工作协议。

协议至少必须定义：

1. 四种任务状态：

   * `backlog`
   * `in-progress`
   * `in-review`
   * `done`
2. 如何选择和领取任务；
3. 如何更新 `assigned`、`branch`、`started`；
4. 如何向 `log` 追加进度；
5. 如何记录 `blockers`；
6. 如何提交审核；
7. 如何归档完成任务；
8. 不得删除已有日志；
9. 不得未经协议直接修改历史完成记录；
10. 任务文件和 `.aurapilot/AGENTS.md` 是 AuraPilot 工作模式下的唯一事实来源。

如果该文件已经存在，不要覆盖。检查其中是否存在协议版本信息，并在最终报告中说明当前版本。

---

## 6. 创建任务 Schema

如果 `.aurapilot/schema.json` 不存在，创建 AuraPilot Protocol v1 的任务 JSON Schema。

任务至少支持以下字段：

```text
id
title
priority
type
created
assigned
branch
started
pr
waiting
completed
commit
desc
accept
log
blockers
```

Schema 应允许 `log` 项包含额外扩展字段，但必须要求每条日志至少包含：

```text
ts
msg
```

如果 Schema 已存在，不要覆盖。

---

## 7. 配置当前 Agent

选择当前 Agent 确实支持的仓库级持久化指令文件。

不要复制完整 AuraPilot 协议，只插入以下最小引用：

```markdown
<!-- aurapilot:start -->
## AuraPilot

本项目使用 AuraPilot 管理 AI Coding 任务。

处理项目任务前，必须读取：

- `.aurapilot/AGENTS.md`
- 用户指定的 `.aurapilot/tasks/` 任务文件

任务领取、执行、进度、阻塞、审核和完成流程，以 `.aurapilot/AGENTS.md` 为准。

当用户说“执行 AuraPilot 任务 TASK-XXX”时：

1. 读取 `.aurapilot/AGENTS.md`；
2. 定位对应任务文件；
3. 按协议领取任务；
4. 执行和验证任务；
5. 持续更新任务进度；
6. 完成后提交审核，不要自行跳过协议步骤。
<!-- aurapilot:end -->
```

配置规则：

* 如果标记区不存在，在适当位置追加；
* 如果标记区已经存在，检查并更新标记区内容；
* 不得重复插入；
* 不得修改标记区之外的现有内容；
* 保持原文件的格式和换行风格；
* 如果需要新建根目录 `AGENTS.md`，只写入上述 AuraPilot 引用；
* 如果当前 Agent 使用独立规则目录，可以创建单独的 AuraPilot 规则文件；
* 不要同时修改多个工具专用文件，除非仓库明确同时维护这些工具的配置。

---

## 8. 写入安装记录

创建或更新 `.aurapilot/installation.yaml`：

```yaml
protocol_version: 1
installed_at: <当前 ISO 8601 时间>
updated_at: <当前 ISO 8601 时间>
configured_files:
  - <实际修改的指令文件>
agent_detected: <当前 Agent 名称；无法确定时为 unknown>
mode: repository
status: ready
```

如果安装记录已经存在：

* 保留最初的 `installed_at`；
* 更新 `updated_at`；
* 更新实际配置文件列表；
* 不删除未知扩展字段。

---

## 9. 验证安装

完成后执行只读验证：

1. `.aurapilot/` 目录结构完整；
2. 四个任务状态目录存在；
3. `.aurapilot/AGENTS.md` 可读取；
4. `.aurapilot/project.yaml` 可解析；
5. `.aurapilot/schema.json` 是有效 JSON；
6. Agent 指令文件中只存在一个 AuraPilot 标记区；
7. 安装过程没有修改标记区以外的既有指令；
8. 没有执行任务；
9. 没有执行 Git commit 或 push；
10. 没有修改仓库之外的文件。

---

## 10. 最终报告

安装完成后，向用户输出：

### 安装结果

* AuraPilot 状态：已安装 / 已升级 / 已存在且无需修改
* 协议版本
* 当前 Agent
* 配置的指令文件
* 新建文件
* 修改文件
* 保留未修改的现有文件
* 警告或需要用户确认的事项

### 使用方法

告诉用户以后可以使用：

```text
执行 AuraPilot 任务 TASK-001
```

或者：

```text
读取 .aurapilot/AGENTS.md，然后处理 backlog 中优先级最高且未被领取的任务。
```

不要在完成安装后自动开始执行任务。
