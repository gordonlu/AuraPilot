import { createPinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import PendingMenu from './components/PendingMenu.vue'
import { usePendingStore } from './stores/pending'

describe('global pending menu', () => {
  it('groups actionable items and routes back to their existing screens', async () => {
    const pinia = createPinia()
    const pending = usePendingStore(pinia)
    pending.items = [{
      project_id: 'project-1', project_name: 'AuraPilot', kind: 'approval',
      task_id: 'TASK-001', title: 'pnpm test', detail: '运行测试', path: null,
      repair_kind: null, approval_id: 'approval-1', created_at: new Date().toISOString(),
    }, {
      project_id: 'project-1', project_name: 'AuraPilot', kind: 'repair',
      task_id: 'TASK-002', title: '缺少协议字段', detail: '补充 blockers: []',
      path: '/repo/.aurapilot/tasks/backlog/TASK-002.yaml',
      repair_kind: 'fill_protocol_fields', approval_id: null,
      created_at: new Date().toISOString(),
    }]
    const wrapper = mount(PendingMenu, { global: { plugins: [pinia] } })
    expect(wrapper.get('.pending-button').text()).toContain('2')
    await wrapper.get('.pending-button').trigger('click')
    expect(wrapper.text()).toContain('等待审批')
    expect(wrapper.text()).toContain('确认修复')
    await wrapper.findAll('.pending-item')[0].trigger('click')
    expect(wrapper.emitted('select')?.[0]).toEqual([{
      view: 'execution', project_id: 'project-1', approval_id: 'approval-1', path: null,
    }])
  })
})
