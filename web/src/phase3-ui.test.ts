import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { describe, expect, it, vi } from 'vitest'
import BoardView from './components/BoardView.vue'
import AddProjectModal from './components/AddProjectModal.vue'
import AppSidebar from './components/AppSidebar.vue'
import TaskFormModal from './components/TaskFormModal.vue'
import TaskDrawer from './components/TaskDrawer.vue'
import ProjectsView from './components/ProjectsView.vue'
import WorldSkinPickerModal from './components/WorldSkinPickerModal.vue'
import { demoSnapshots } from './demo'
import { useProjectsStore } from './stores/projects'

describe('Phase 3 production UI', () => {
  it('selects a world skin explicitly and exposes its runtime state', async () => {
    const wrapper = mount(WorldSkinPickerModal, {
      props: {
        current: 'classic',
        runtimeState: { skin: 'classic', status: 'idle', error: null },
      },
    })

    expect(wrapper.findAll('.world-skin-option')).toHaveLength(4)
    expect(wrapper.find('.world-skin-option.classic').attributes('aria-checked')).toBe('true')
    expect(wrapper.text()).toContain('静态界面')
    expect(wrapper.text()).toContain('动态世界')

    await wrapper.find('.world-skin-option.seascape').trigger('click')
    expect(wrapper.emitted('select')?.[0]).toEqual(['seascape'])
    await wrapper.find('.world-skin-option.stellar').trigger('click')
    expect(wrapper.emitted('select')?.[1]).toEqual(['stellar'])
    await wrapper.find('.world-skin-option.polarscape').trigger('click')
    expect(wrapper.emitted('select')?.[2]).toEqual(['polarscape'])

    await wrapper.setProps({
      current: 'seascape',
      runtimeState: { skin: 'seascape', status: 'loading', error: null },
    })
    expect(wrapper.find('.world-skin-option.seascape').attributes('aria-checked')).toBe('true')
    expect(wrapper.find('.world-skin-option.seascape').text()).toContain('正在启动')

    await wrapper.setProps({
      current: 'classic',
      runtimeState: { skin: 'classic', status: 'idle', error: null },
      error: '无法启动海岸世界：WebGL unavailable',
      failedSkin: 'seascape',
    })
    expect(wrapper.find('.world-skin-option.seascape').text()).toContain('启动失败')
    expect(wrapper.find('[role="alert"]').text()).toContain('WebGL unavailable')
  })

  it('summarizes every project and opens the selected project board', async () => {
    const snapshots = demoSnapshots()
    const pinia = createPinia()
    const store = useProjectsStore(pinia)
    const openFolder = vi.spyOn(store, 'openFolder').mockResolvedValue()
    const wrapper = mount(ProjectsView, {
      props: { snapshots, search: '' },
      global: { plugins: [pinia] },
    })

    expect(wrapper.findAll('.project-overview-card')).toHaveLength(3)
    expect(wrapper.text()).toContain('项目一览')
    expect(wrapper.text()).toContain('aurapilot')
    expect(wrapper.text()).toContain('server-core')
    expect(wrapper.text()).toContain('web-dashboard')
    expect(wrapper.find('.project-summary').text()).toContain('阻塞2')

    await wrapper.findAll('.project-folder')[0].trigger('click')
    expect(openFolder).toHaveBeenCalledWith(snapshots[0].registration.id)
    await wrapper.findAll('.project-open')[1].trigger('click')
    expect(wrapper.emitted('open')?.[0]).toEqual([snapshots[1].registration.id])

    await wrapper.setProps({ search: 'web-dashboard' })
    expect(wrapper.findAll('.project-overview-card')).toHaveLength(1)
    expect(wrapper.text()).toContain('web-dashboard')
  })

  it('only marks a positive blocked-task count as dangerous', async () => {
    const snapshots = demoSnapshots()
    for (const snapshot of snapshots) {
      for (const task of snapshot.tasks) task.document.blockers = []
    }
    const wrapper = mount(AppSidebar, {
      props: {
        snapshots,
        activeProject: 'all',
        activeView: 'board',
        theme: 'light',
        worldSkin: 'classic',
        diagnosticCount: 0,
      },
    })

    const count = wrapper.find('.primary-nav button:last-child b')
    expect(count.text()).toBe('0')
    expect(count.classes()).not.toContain('danger-count')
    expect(wrapper.find('.sidebar-footer button').text()).toContain('经典界面')
    await wrapper.find('.sidebar-footer button').trigger('click')
    expect(wrapper.emitted('worldSkin')).toHaveLength(1)

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
    expect(wrapper.find('.task-card img').exists()).toBe(false)
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

  it('completes without asking for a commit that AuraPilot did not create', async () => {
    const project = demoSnapshots()[0]
    const task = project.tasks.find((item) => item.state === 'in-review')!
    const wrapper = mount(TaskDrawer, {
      props: { project, task, diagnostics: [] },
    })

    await wrapper.find('.transition-section select').setValue('done')
    expect(wrapper.find('.commit-field').exists()).toBe(false)
    expect(wrapper.text()).toContain('完成任务不会创建 Git Commit')
    expect(wrapper.text()).toContain('不会自动关联仓库最新提交')
    await wrapper.find('.transition-section .button').trigger('click')
    expect(wrapper.emitted('transition')?.[0]?.[0]).toEqual({
      target: 'done', assigned: 'gemini-cli', branch: 'feature/bulk-actions',
    })
  })
})
