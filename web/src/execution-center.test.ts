import { createPinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import ExecutionCenter from './components/ExecutionCenter.vue'
import { demoSnapshots } from './demo'
import { useAgentsStore } from './stores/agents'

describe('Agent execution observability', () => {
  it('separates provider evidence from push state and exposes failures and raw detail', async () => {
    const snapshots = demoSnapshots()
    const project = snapshots[0]
    const task = project.tasks[0].document.id!
    const pinia = createPinia()
    const agents = useAgentsStore(pinia)
    vi.spyOn(agents, 'loadExecutionEvents').mockResolvedValue([])
    agents.pushAttempts = [{
      id: 'attempt-1', project_id: project.registration.id, task_id: task,
      agent_profile_id: 'codex', created_at: '2026-08-10T10:00:00Z',
      status: 'started', process_id: null, error: null, delivery: 'process',
    }]
    agents.executionEvents = [{
      id: 'event-1', project_id: project.registration.id, task_id: task,
      profile_id: 'codex', provider: 'codex', session_binding_id: 'session-binding-1',
      attempt_id: 'attempt-1', kind: 'command', level: 'info', phase: 'item/started',
      message: '开始执行命令：pnpm test', detail: '{"method":"item/started"}',
      created_at: '2026-08-10T10:00:01Z',
    }, {
      id: 'event-2', project_id: project.registration.id, task_id: task,
      profile_id: 'codex', provider: 'codex', session_binding_id: 'session-binding-1',
      attempt_id: 'attempt-1', kind: 'approval', level: 'warning', phase: 'requestApproval',
      message: 'Agent 请求交互审批；AuraPilot 当前无法代替用户响应，该请求会被拒绝。',
      detail: '{"method":"requestApproval"}', created_at: '2026-08-10T10:00:02Z',
    }]

    const wrapper = mount(ExecutionCenter, {
      props: { snapshots, activeProject: project.registration.id },
      global: { plugins: [pinia] },
    })

    expect(wrapper.text()).toContain('Agent 执行中心')
    expect(wrapper.text()).toContain('Agent 已接收')
    expect(wrapper.text()).toContain('开始执行命令：pnpm test')
    expect(wrapper.text()).toContain('请求交互审批')
    expect(wrapper.text()).toContain('任务 YAML 仍是任务状态唯一事实来源')
    await wrapper.find('.execution-event details summary').trigger('click')
    expect(wrapper.find('.execution-event pre').text()).toContain('item/started')
    await wrapper.find('.execution-event-content header button').trigger('click')
    expect(wrapper.emitted('openTask')?.[0]).toEqual([project.registration.id, task])

    await wrapper.findAll('.execution-tabs button')[2].trigger('click')
    expect(wrapper.findAll('.execution-event')).toHaveLength(1)
    expect(wrapper.text()).toContain('请求交互审批')
  })

  it('shows pending Codex approval as an explicit user decision', async () => {
    const snapshots = demoSnapshots()
    const project = snapshots[0]
    const task = project.tasks[0].document.id!
    const pinia = createPinia()
    const agents = useAgentsStore(pinia)
    vi.spyOn(agents, 'loadExecutionEvents').mockResolvedValue([])
    vi.spyOn(agents, 'loadApprovals').mockResolvedValue([])
    const respond = vi.spyOn(agents, 'respondApproval').mockResolvedValue({} as never)
    agents.approvals = [{
      id: 'approval-1', project_id: project.registration.id, task_id: task,
      profile_id: 'codex', provider: 'codex', session_binding_id: 'session-1',
      attempt_id: 'attempt-1', turn_id: 'turn-1', item_id: 'item-1',
      provider_request_key: '51', kind: 'command_execution', command: 'pnpm test',
      cwd: '/repo', reason: '运行测试', status: 'pending', decision: null, error: null,
      created_at: '2026-08-10T10:00:00Z', updated_at: '2026-08-10T10:00:00Z', resolved_at: null,
    }]

    const wrapper = mount(ExecutionCenter, {
      props: { snapshots, activeProject: project.registration.id },
      global: { plugins: [pinia] },
    })
    expect(wrapper.text()).toContain('Codex 审批请求')
    expect(wrapper.text()).toContain('pnpm test')
    expect(wrapper.text()).toContain('等待处理')
    await wrapper.find('.approval-actions .primary').trigger('click')
    expect(respond).toHaveBeenCalledWith('approval-1', 'accept')
  })
})
