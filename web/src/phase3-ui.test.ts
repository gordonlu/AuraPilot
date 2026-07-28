import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import BoardView from './components/BoardView.vue'
import TaskFormModal from './components/TaskFormModal.vue'
import { demoSnapshots } from './demo'

describe('Phase 3 production UI', () => {
  it('renders the four protocol states, blocked cues, and escapes untrusted task text', async () => {
    const snapshots = demoSnapshots()
    snapshots[0].tasks[0].document.title = '<img src=x onerror=alert(1)>'
    const wrapper = mount(BoardView, {
      props: { snapshots, search: '', selectedKey: null },
    })

    expect(wrapper.findAll('.board-column')).toHaveLength(4)
    expect(wrapper.text()).toContain('Backlog')
    expect(wrapper.text()).toContain('<img src=x onerror=alert(1)>')
    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.findAll('.task-card.blocked').length).toBeGreaterThan(0)

    await wrapper.find('.task-card').trigger('click')
    expect(wrapper.emitted('open')).toHaveLength(1)
  })

  it('normalizes acceptance criteria before submitting a new task', async () => {
    const snapshots = demoSnapshots()
    const wrapper = mount(TaskFormModal, {
      props: { mode: 'create', projects: snapshots, projectId: snapshots[0].registration.id },
    })
    await wrapper.find('input[placeholder="清晰描述需要完成的工作"]').setValue('新的生产任务')
    await wrapper.find('textarea[placeholder*="功能按预期工作"]').setValue('第一项\n\n第二项  ')
    await wrapper.find('form').trigger('submit')

    const event = wrapper.emitted('save')?.[0]
    expect(event?.[0]).toBe(snapshots[0].registration.id)
    expect(event?.[1]).toMatchObject({ title: '新的生产任务', accept: ['第一项', '第二项'] })
  })
})
