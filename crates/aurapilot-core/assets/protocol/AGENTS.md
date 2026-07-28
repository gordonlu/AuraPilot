# AuraPilot Agent 协议

本项目使用 AuraPilot 协议管理开发任务。任何 Coding Agent 接入前必须读完本文档。

协议版本：1

## 唯一事实来源

任务文件和本文件是 AuraPilot 工作模式下的唯一事实来源。UI、Push 记录和 Agent 进程状态不能替代任务文件状态。

## 目录结构

- `.aurapilot/tasks/backlog/`：待办
- `.aurapilot/tasks/in-progress/`：进行中
- `.aurapilot/tasks/in-review/`：待审核
- `.aurapilot/tasks/done/`：已完成

## 工作流

### 1. 领取任务

1. 读取用户指定的 backlog 任务文件。
2. 确认任务仍位于 `backlog/` 且未被领取。
3. 将任务移动到 `in-progress/`。
4. 添加 `assigned`、`branch`、`started`。
5. 使用独立业务分支执行任务。

建议同时只领取一个任务；如必须并行，每个任务必须使用独立分支。

### 2. 执行任务

- 每完成一个可验证里程碑，在 `log` 末尾追加包含 `ts` 和 `msg` 的记录。
- 可以在日志项中加入 `model`、`tokens`、`duration_ms` 等扩展字段。
- 遇到阻塞时，在 `blockers` 中追加清晰描述。
- 不得删除或重写既有日志。

### 3. 提交审核

- 完成任务验收和自测后，将文件移动到 `in-review/`。
- 可填写整数 `pr` 和 `waiting`。
- 提交信息应包含任务 ID。

### 4. 完成归档

- 审核或合并完成后，将文件移动到 `done/`。
- 添加 `completed` 和 7–40 位小写十六进制 `commit`。
- 不得未经协议修改历史完成记录。

## 字段约定

- `priority`：P0 / P1 / P2 / P3
- `type`：feature / bug / refactor / docs / test / chore
- `health`：green / yellow / red
- 时间使用 RFC 3339；日期使用 `YYYY-MM-DD`

## 安全约束

- 只修改当前仓库内与任务直接相关的文件。
- 不修改用户全局 Agent 配置。
- 不因收到 Push 就假定任务已经领取。
- 不跳过任务文件中的验收标准。
- 不自动执行与当前任务无关的任务。
