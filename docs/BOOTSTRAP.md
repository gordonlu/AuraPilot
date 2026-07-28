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
* `aurapilot` CLI 是否可用（执行 `aurapilot --version`）；
* 是否已经存在 `.aurapilot/` 和 `.aurapilot/installation.yaml`；
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

如果 CLI 不可用，停止并提示用户按照安装文档安装。不要退回为手工拼装协议文件，因为 CLI 内置了当前版本的协议、Schema 和安全检查。

---

## 3. 使用 CLI 初始化并注册

在仓库根目录执行：

```sh
aurapilot init .
aurapilot add .
```

`init` 必须先完成，`add` 用于把项目注册到 AuraPilot 桌面端和 CLI 共用的本地项目列表。即使项目已经初始化，也要执行 `aurapilot add .`；若已经注册，只需在最终报告中说明，无需改写协议文件。

不要自行创建、覆盖或修补 `.aurapilot/` 中的协议文件。它们由 `aurapilot init` 按当前协议版本生成或安全修复。默认让 `.aurapilot/` 进入 Git 版本控制；只有用户明确要求不跟踪时，才使用 `aurapilot init . --ignore`。

---

## 4. 核对初始化结果

确认 CLI 已创建或保留：

* `.aurapilot/project.yaml`；
* `.aurapilot/AGENTS.md`；
* `.aurapilot/schema.json`；
* `.aurapilot/installation.yaml`；
* `tasks/backlog`、`tasks/in-progress`、`tasks/in-review`、`tasks/done` 四个状态目录。

`.aurapilot/AGENTS.md` 是完整的 Agent 工作协议，位于 `.aurapilot/` 内；仓库根目录的 `AGENTS.md` 只保存下一节所述的最小引用。不要把两者混为一份文件。

如果 CLI 报错，保留现场并报告原始错误。不要通过手工写文件绕过 CLI 校验。

---

## 5. 配置当前 Agent

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

## 6. 核对安装记录

`aurapilot init` 会创建或保留 `.aurapilot/installation.yaml`。不要手工重写该文件。配置完仓库级 Agent 引用后，仅核对记录与实际情况；当前 CLI 尚未记录外部 Agent 文件时，在最终报告中列出实际配置文件即可。

安装记录的基础结构为：

```yaml
protocol_version: 1
installed_at: <当前 ISO 8601 时间>
updated_at: <当前 ISO 8601 时间>
configured_files: []
agent_detected: unknown
mode: repository
status: protocol_initialized
```

---

## 7. 验证安装

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
10. `aurapilot status` 中可以看到当前项目；
11. 没有修改仓库之外的业务文件（AuraPilot 本地注册表除外）。

---

## 8. 最终报告

安装完成后，向用户输出：

### 安装结果

* AuraPilot 状态：已安装 / 已升级 / 已存在且无需修改
* 协议版本
* 当前 Agent
* 配置的指令文件
* CLI 注册结果
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
