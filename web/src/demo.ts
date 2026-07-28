import type { ProjectSnapshot, RegisteredProject, TaskDocument, TaskState } from './types/protocol'

const projects: RegisteredProject[] = [
  { id: 'demo-aura', path: '/workspace/aurapilot', registered_at: '2026-07-28T08:00:00Z', last_profile_id: 'opencode' },
  { id: 'demo-core', path: '/workspace/server-core', registered_at: '2026-07-28T08:05:00Z', last_profile_id: null },
  { id: 'demo-web', path: '/workspace/web-dashboard', registered_at: '2026-07-28T08:10:00Z', last_profile_id: null },
]

const task = (
  id: string,
  state: TaskState,
  title: string,
  priority: string,
  extra: Partial<TaskDocument> = {},
) => ({
  path: `/workspace/.aurapilot/tasks/${state}/${id}.yaml`,
  state,
  document: {
    id,
    title,
    priority,
    type: 'feature',
    created: '2026-07-28',
    assigned: null,
    branch: null,
    started: null,
    pr: null,
    waiting: null,
    completed: null,
    commit: null,
    desc: '构建清晰、可靠且不依赖特定 Agent 的本地任务工作流。',
    accept: ['界面状态与任务文件保持一致', '键盘和鼠标均可完成核心操作'],
    log: [],
    blockers: [],
    ...extra,
  },
})

export function demoSnapshots(): ProjectSnapshot[] {
  return [
    {
      registration: projects[0],
      project: {
        name: 'aurapilot', owner: 'gordon', health: 'green', sprint: '2026-W31', notes: null,
        schema_version: 1, created: '2026-07-27',
      },
      tasks: [
        task('TASK-142', 'backlog', '支持导入 .auraignore 文件', 'P2'),
        task('TASK-143', 'backlog', '优化任务搜索的模糊匹配', 'P3'),
        task('TASK-137', 'in-progress', '任务看板拖拽排序逻辑', 'P1', {
          assigned: 'codex', branch: 'feature/board-order', started: '2026-07-28T09:20:00Z',
          blockers: ['跨平台 rename 事件需要归一化'], progress: 60,
        }),
        task('TASK-138', 'in-progress', '阻塞视图与看板联动', 'P2', {
          assigned: 'claude-code', branch: 'feature/blocked-sync', started: '2026-07-28T08:40:00Z', progress: 35,
        }),
        task('TASK-136', 'in-review', '支持任务批量操作', 'P2', {
          assigned: 'gemini-cli', branch: 'feature/bulk-actions', started: '2026-07-27T12:00:00Z', pr: 48, waiting: 'review', progress: 90,
        }),
        task('TASK-134', 'done', '新增快捷键说明', 'P2', {
          assigned: 'codex', branch: 'docs/shortcuts', started: '2026-07-26T09:00:00Z', completed: '2026-07-26T12:00:00Z', commit: 'abc1234',
        }),
      ],
      diagnostics: [],
    },
    {
      registration: projects[1],
      project: {
        name: 'server-core', owner: 'platform', health: 'yellow', sprint: '2026-W31', notes: null,
        schema_version: 1, created: '2026-07-20',
      },
      tasks: [
        task('TASK-214', 'in-progress', '用户认证接口重构', 'P1', {
          assigned: 'codex', branch: 'refactor/auth', started: '2026-07-28T07:20:00Z', progress: 80,
        }),
        task('TASK-218', 'backlog', '新增分页参数校验', 'P2'),
        task('TASK-213', 'in-review', '修复 Token 刷新并发问题', 'P0', {
          assigned: 'claude-code', branch: 'fix/token-refresh', started: '2026-07-27T08:00:00Z', pr: 72, waiting: 'backend-review',
          blockers: ['等待安全团队确认刷新策略'], progress: 100,
        }),
        task('TASK-211', 'done', '日志切割配置优化', 'P2', {
          assigned: 'codex', branch: 'ops/log-rotate', started: '2026-07-24T08:00:00Z', completed: '2026-07-24T11:00:00Z', commit: 'def5678',
        }),
      ],
      diagnostics: [{
        severity: 'warning', code: 'missing_required', message: '一个历史任务缺少 waiting 字段',
        field: 'waiting', path: '/workspace/server-core/.aurapilot/tasks/in-review/TASK-213.yaml',
      }],
    },
    {
      registration: projects[2],
      project: {
        name: 'web-dashboard', owner: 'frontend', health: 'green', sprint: '2026-W31', notes: null,
        schema_version: 1, created: '2026-07-18',
      },
      tasks: [
        task('TASK-188', 'in-review', '图表组件懒加载', 'P2', {
          assigned: 'gemini-cli', branch: 'perf/lazy-charts', started: '2026-07-26T08:00:00Z', pr: 19, progress: 70,
        }),
        task('TASK-189', 'in-progress', '仪表盘性能优化', 'P2', {
          assigned: 'codex', branch: 'perf/dashboard', started: '2026-07-28T06:00:00Z', progress: 25,
        }),
      ],
      diagnostics: [],
    },
  ]
}
