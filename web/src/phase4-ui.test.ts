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
    await wrapper.findAll('.agent-option')[4].trigger('click')
    await wrapper.find('button.button.primary').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('OpenCode 已启动')
    expect(task.state).toBe(originalState)
    expect(task.document.assigned).toBeNull()
  })
})
