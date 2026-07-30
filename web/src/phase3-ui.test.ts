import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import BoardView from './components/BoardView.vue'
import AddProjectModal from './components/AddProjectModal.vue'
import AppSidebar from './components/AppSidebar.vue'
import TaskFormModal from './components/TaskFormModal.vue'
import TaskDrawer from './components/TaskDrawer.vue'
import { demoSnapshots } from './demo'

describe('Phase 3 production UI', () => {
  it('only marks a positive blocked-task count as dangerous', async () => {
    const snapshots = demoSnapshots()
    for (const snapshot of snapshots) {
      for (const task of snapshot.tasks) task.document.blockers = []
    }
    const wrapper = mount(AppSidebar, {
      props: { snapshots, activeProject: 'all', activeView: 'board', theme: 'light', diagnosticCount: 0 },
    })

    const count = wrapper.find('.primary-nav button:last-child b')
    expect(count.text()).toBe('0')
    expect(count.classes()).not.toContain('danger-count')

    snapshots[0].tasks[0].document.blockers = ['waiting']
    await wrapper.setProps({ snapshots: [...snapshots] })
    expect(count.text()).toBe('1')
    expect(count.classes()).toContain('danger-count')
  })

  it('offers native directory selection and an in-place initialization recovery action', async () => {
    const wrapper = mount(AddProjectModal, {
      props: {
        path: '/repo without protocol',
        canInitialize: true,
        error: null,
      },
    })

    expect(wrapper.text()).toContain('选择目录')
    expect(wrapper.text()).toContain('这个项目还没有 AuraPilot 协议')
    await wrapper.find('.browse-button').trigger('click')
    await wrapper.find('.initialization-callout .button').trigger('click')

    expect(wrapper.emitted('browse')).toHaveLength(1)
    expect(wrapper.emitted('initialize')?.[0]).toEqual(['/repo without protocol'])
    expect(wrapper.attributes('aria-labelledby')).toBeUndefined()
    expect(wrapper.find('[role="dialog"]').attributes('aria-labelledby')).toBe('add-project-title')
  })

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

  it('marks status transition as optional and keeps Push independent from task state', async () => {
    const project = demoSnapshots()[0]
    const task = project.tasks.find((item) => item.state === 'backlog')!
    const originalState = task.state
    const wrapper = mount(TaskDrawer, {
      props: { project, task, diagnostics: [] },
    })

    expect(wrapper.text()).toContain('状态变更 非必选')
    expect(wrapper.text()).toContain('Push 只会发送任务指令，不会自动领取或改变任务状态')
    expect(wrapper.find('.transition-section select').element).toHaveProperty('value', '')
    await wrapper.find('.drawer-footer .button.primary').trigger('click')

    expect(wrapper.emitted('push')).toHaveLength(1)
    expect(wrapper.emitted('transition')).toBeUndefined()
    expect(task.state).toBe(originalState)
  })
})
