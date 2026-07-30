import { createPinia } from 'pinia'
import { mount, flushPromises } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import PushTaskModal from './components/PushTaskModal.vue'
import { demoSnapshots } from './demo'
import { useAgentsStore } from './stores/agents'

describe('Phase 4 Push Dispatcher UI', () => {
  it('offers an explicit Git branch strategy before a new Session without changing task state', async () => {
    const project = demoSnapshots()[0]
    const task = project.tasks.find((item) => item.state === 'backlog')!
    const originalState = task.state
    const pinia = createPinia()
    const wrapper = mount(PushTaskModal, {
      props: { project, task },
      global: { plugins: [pinia] },
    })
    await flushPromises()
    const agents = useAgentsStore(pinia)
    const push = vi.spyOn(agents, 'push')

    expect(wrapper.findAll('.agent-option').length).toBeGreaterThanOrEqual(3)
    expect(wrapper.text()).toContain('OpenCode')
    expect(wrapper.text()).toContain('.aurapilot/AGENTS.md')
    expect(wrapper.text()).toContain('由你选择新 Session 或项目已有 Session')
    expect(wrapper.text()).toContain('Session 会锁定所选 Profile')
    expect(wrapper.text()).toContain('Git 分支策略')
    expect(wrapper.text()).toContain('沿用当前分支')
    expect(wrapper.text()).toContain('与 Agent Session 分支是两个独立概念')
    await wrapper.findAll('.branch-option input')[1].setValue(true)
    await wrapper.find('.branch-name input').setValue('task/TASK-009')
    await wrapper.findAll('.agent-option')[4].trigger('click')
    expect(wrapper.find('button.button.primary').text()).toContain('新建 Session 并 Push 给 OpenCode')
    await wrapper.find('button.button.primary').trigger('click')
    await flushPromises()

    expect(push).toHaveBeenCalledWith(project.registration.id, task.document.id, 'opencode', 'task/TASK-009')
    expect(wrapper.text()).toContain('OpenCode 已启动')
    expect(wrapper.text()).toContain('当前工作树已切换到 task/TASK-009')
    expect((wrapper.find('.branch-option input:checked').element as HTMLInputElement).value).toBe('current')
    expect(task.state).toBe(originalState)
    expect(task.document.assigned).toBeNull()
  })

  it('makes existing-session targeting explicit and exposes a manual ID fallback', async () => {
    const project = demoSnapshots()[0]
    const task = project.tasks.find((item) => item.state === 'backlog')!
    const pinia = createPinia()
    const wrapper = mount(PushTaskModal, {
      props: { project, task },
      global: { plugins: [pinia] },
    })
    await flushPromises()

    await wrapper.findAll('.push-mode-switch button')[1].trigger('click')
    expect(wrapper.text()).toContain('AuraPilot 无法保证会话上下文仍适合当前任务')
    expect(wrapper.text()).toContain('此模式不会切换 Git 分支')
    expect(wrapper.find('button.button.primary').attributes('disabled')).toBeDefined()
    await wrapper.find('.manual-bind-toggle').trigger('click')
    expect(wrapper.find('.manual-session-form').exists()).toBe(true)
    expect(wrapper.find('.manual-session-form input[placeholder*="thr_"]').exists()).toBe(true)
    await wrapper.find('.manual-session-form select').setValue('codex')
    await wrapper.find('.manual-session-form input[placeholder*="thr_"]').setValue('thr_existing')
    await wrapper.find('.manual-session-form').trigger('submit')
    await flushPromises()

    const fork = wrapper.find('.session-actions button')
    expect(fork.text()).toContain('创建 Session 分支并 Push')
    expect(fork.attributes('disabled')).toBeUndefined()
    expect(wrapper.text()).toContain('这不会创建 Git 分支')
    await fork.trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('已创建 Codex Session 分支')

    const agents = useAgentsStore(pinia)
    agents.sessions[0].state = 'running'
    agents.sessions[0].active_turn_id = 'turn_active'
    await nextTick()
    const actions = wrapper.findAll('.session-actions button')
    expect(actions.map((button) => button.text())).toContain('追加到当前 Turn')
    expect(actions.map((button) => button.text())).toContain('中断后追加')
    await actions.find((button) => button.text().includes('追加到当前 Turn'))!.trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('已追加到 Codex 当前 Turn')
  })

  it('edits an idle managed binding with confirmation and exposes OpenCode controls by state', async () => {
    const project = demoSnapshots()[0]
    const task = project.tasks.find((item) => item.state === 'backlog')!
    const pinia = createPinia()
    const wrapper = mount(PushTaskModal, {
      props: { project, task },
      global: { plugins: [pinia] },
    })
    await flushPromises()
    const agents = useAgentsStore(pinia)
    agents.sessions = [{
      id: 'binding-open-code',
      project_id: project.registration.id,
      profile_id: 'opencode',
      provider: 'open_code',
      external_session_id: 'ses_original',
      source: 'managed',
      verification: 'verified',
      display_name: 'OpenCode task',
      working_directory: project.registration.path,
      state: 'idle',
      active_turn_id: null,
      hidden: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      last_used_at: new Date().toISOString(),
    }]
    await nextTick()

    await wrapper.findAll('.push-mode-switch button')[1].trigger('click')
    await wrapper.find('.session-option').trigger('click')
    expect(wrapper.text()).toContain('创建 Session 分支并 Push')
    const editButton = wrapper.findAll('.session-binding-actions button')
      .find((button) => button.text().includes('编辑所选绑定'))!
    await editButton.trigger('click')
    const form = wrapper.findAll('.manual-session-form').find((item) => item.text().includes('Profile（不可更换）'))!
    const inputs = form.findAll('input')
    await inputs.find((input) => input.attributes('autocomplete') === 'off')!.setValue('ses_replacement')
    expect(form.text()).toContain('我确认替换 AuraPilot 自动记录的 Session ID')
    const save = form.findAll('button').find((button) => button.text().includes('保存并重新验证'))!
    expect(save.attributes('disabled')).toBeDefined()
    await form.find('input[type="checkbox"]').setValue(true)
    expect(save.attributes('disabled')).toBeUndefined()
    await form.trigger('submit')
    await flushPromises()
    expect(agents.sessions[0].external_session_id).toBe('ses_replacement')
    expect(agents.sessions[0].profile_id).toBe('opencode')

    agents.sessions[0].state = 'running'
    agents.sessions[0].active_turn_id = 'msg_active'
    await nextTick()
    expect(wrapper.text()).toContain('中断后追加')
    expect(wrapper.text()).toContain('运行中的 Session 不能修改名称或 ID')
    expect(editButton.attributes('disabled')).toBeDefined()
  })
})
