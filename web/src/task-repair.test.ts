import { createPinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import DiagnosticsPanel from './components/DiagnosticsPanel.vue'
import { demoSnapshots } from './demo'
import { useProjectsStore } from './stores/projects'
import type { RepairPlan } from './types/protocol'

describe('task repair confirmation', () => {
  it('previews a missing blockers repair and requires a second click to apply', async () => {
    const pinia = createPinia()
    const projects = useProjectsStore(pinia)
    const snapshot = demoSnapshots()[0]
    snapshot.diagnostics = [{
      severity: 'warning', code: 'missing_required',
      message: 'missing required field `blockers`', field: 'blockers',
      path: '/repo/.aurapilot/tasks/backlog/TASK-001.yaml',
    }]
    const plan: RepairPlan = {
      id: 'repair-1', kind: 'fill_protocol_fields', path: snapshot.diagnostics[0].path!,
      summary: '缺少可以确定补全的协议字段', detail: '确认后规范化 YAML',
      action: { type: 'rewrite', new_content: 'blockers: []\n', changes: ['补充协议字段 blockers: []'] },
      source_sha256: 'digest',
    }
    vi.spyOn(projects, 'previewRepairs').mockResolvedValue([plan])
    const apply = vi.spyOn(projects, 'applyRepair').mockResolvedValue({
      applied: { kind: plan.kind, path: plan.path, message: '已修复' }, snapshot,
    })

    const wrapper = mount(DiagnosticsPanel, {
      props: { snapshots: [snapshot] }, global: { plugins: [pinia] },
    })
    await wrapper.find('.repair-toolbar button').trigger('click')
    expect(wrapper.text()).toContain('补充协议字段 blockers: []')
    await wrapper.find('.repair-card .primary').trigger('click')
    expect(apply).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('确认执行此修复')
    await wrapper.find('.repair-card .primary').trigger('click')
    expect(apply).toHaveBeenCalledWith(snapshot.registration.id, plan)
  })
})
