import { createPinia } from 'pinia'
import { mount, flushPromises } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import PushTaskModal from './components/PushTaskModal.vue'
import { demoSnapshots } from './demo'

describe('Phase 4 Push Dispatcher UI', () => {
  it('offers OpenCode and at least three agents while preserving the task state', async () => {
    const project = demoSnapshots()[0]
    const task = project.tasks.find((item) => item.state === 'backlog')!
    const originalState = task.state
    const wrapper = mount(PushTaskModal, {
      props: { project, task },
      global: { plugins: [createPinia()] },
    })
    await flushPromises()

    expect(wrapper.findAll('.agent-option').length).toBeGreaterThanOrEqual(3)
    expect(wrapper.text()).toContain('OpenCode')
    expect(wrapper.text()).toContain('.aurapilot/AGENTS.md')
    expect(wrapper.text()).toContain('由你选择新 Session 或项目已有 Session')
    expect(wrapper.text()).toContain('Session 会锁定所选 Profile')
    await wrapper.findAll('.agent-option')[4].trigger('click')
    expect(wrapper.find('button.button.primary').text()).toContain('新建 Session 并 Push 给 OpenCode')
    await wrapper.find('button.button.primary').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('OpenCode 已启动')
    expect(task.state).toBe(originalState)
    expect(task.document.assigned).toBeNull()
  })

  it('makes existing-session targeting explicit and exposes a manual ID fallback', async () => {
    const project = demoSnapshots()[0]
    const task = project.tasks.find((item) => item.state === 'backlog')!
    const wrapper = mount(PushTaskModal, {
      props: { project, task },
      global: { plugins: [createPinia()] },
    })
    await flushPromises()

    await wrapper.findAll('.push-mode-switch button')[1].trigger('click')
    expect(wrapper.text()).toContain('AuraPilot 无法保证会话上下文仍适合当前任务')
    expect(wrapper.find('button.button.primary').attributes('disabled')).toBeDefined()
    await wrapper.find('.manual-bind-toggle').trigger('click')
    expect(wrapper.find('.manual-session-form').exists()).toBe(true)
    expect(wrapper.find('.manual-session-form input[placeholder*="thr_"]').exists()).toBe(true)
  })
})
