import { createPinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import AuraTransferModal from './components/AuraTransferModal.vue'
import { demoSnapshots } from './demo'
import { useProjectsStore } from './stores/projects'

describe('Aura task package UI', () => {
  it('exports selected tasks with optional encryption and observable completion', async () => {
    const projects = demoSnapshots()
    const pinia = createPinia()
    const wrapper = mount(AuraTransferModal, {
      props: { projects, initialProjectId: projects[0].registration.id },
      global: { plugins: [pinia] },
    })
    const store = useProjectsStore(pinia)
    vi.spyOn(store, 'chooseAuraExportPath').mockResolvedValue('/tmp/demo.aura')
    const exportAura = vi.spyOn(store, 'exportAura').mockResolvedValue({
      output: '/tmp/demo.aura', encrypted: true, task_count: projects[0].tasks.length,
      package_sha256: 'abc',
    })

    expect(wrapper.findAll('.task-check')).toHaveLength(projects[0].tasks.length)
    expect(wrapper.findAll('.task-check input:checked')).toHaveLength(projects[0].tasks.length)
    await wrapper.find('.path-picker button').trigger('click')
    await flushPromises()
    await wrapper.find('.encryption-toggle input').setValue(true)
    const passwords = wrapper.findAll('input[type="password"]')
    await passwords[0].setValue('secret')
    await passwords[1].setValue('secret')
    await wrapper.find('footer .button.primary').trigger('click')
    await flushPromises()

    expect(exportAura).toHaveBeenCalledWith(
      projects[0].registration.id,
      projects[0].tasks.map((task) => task.document.id),
      '/tmp/demo.aura',
      'secret',
    )
    expect(wrapper.text()).toContain('已导出')
    expect((passwords[0].element as HTMLInputElement).value).toBe('')
  })

  it('requires a conflict-free preview before enabling import', async () => {
    const projects = demoSnapshots()
    const pinia = createPinia()
    const wrapper = mount(AuraTransferModal, {
      props: { projects },
      global: { plugins: [pinia] },
    })
    const store = useProjectsStore(pinia)
    vi.spyOn(store, 'chooseAuraPackage').mockResolvedValue('/tmp/incoming.aura')
    vi.spyOn(store, 'previewAuraImport').mockResolvedValue({
      format_version: 1,
      encrypted: false,
      package_sha256: 'preview-sha',
      has_conflicts: true,
      items: [{ task_id: 'TASK-001', state: 'backlog', relative_path: 'tasks/backlog/TASK-001.yaml', conflict: true }],
    })

    await wrapper.findAll('.transfer-tabs button')[1].trigger('click')
    await wrapper.find('.path-picker button').trigger('click')
    await flushPromises()
    await wrapper.find('footer .button.primary').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('内容未加密')
    expect(wrapper.text()).toContain('发现 1 个冲突')
    expect(wrapper.find('footer .button.primary').attributes('disabled')).toBeDefined()
  })
})
