<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useProjectsStore } from '../stores/projects'
import type { AuraImportPreview, ProjectSnapshot } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ projects: ProjectSnapshot[]; initialProjectId?: string }>()
const emit = defineEmits<{ close: [] }>()
const projectsStore = useProjectsStore()
const mode = ref<'export' | 'import'>('export')
const projectId = ref(props.initialProjectId && props.initialProjectId !== 'all'
  ? props.initialProjectId
  : (props.projects[0]?.registration.id ?? ''))
const selectedTaskIds = ref<string[]>([])
const outputPath = ref('')
const packagePath = ref('')
const encrypt = ref(false)
const password = ref('')
const passwordConfirm = ref('')
const busy = ref(false)
const error = ref('')
const message = ref('')
const preview = ref<AuraImportPreview | null>(null)

const project = computed(() => props.projects.find((item) => item.registration.id === projectId.value))
const tasks = computed(() => project.value?.tasks ?? [])
const conflictCount = computed(() => preview.value?.items.filter((item) => item.conflict).length ?? 0)

const selectAllTasks = () => { selectedTaskIds.value = tasks.value.flatMap((task) => task.document.id ? [task.document.id] : []) }
watch(projectId, () => {
  selectAllTasks()
  preview.value = null
  message.value = ''
  error.value = ''
}, { immediate: true })
watch([packagePath, password], () => { preview.value = null })

const chooseExportPath = async () => {
  error.value = ''
  try {
    const name = `${project.value?.project?.name ?? 'aurapilot'}-tasks.aura`
    const selected = await projectsStore.chooseAuraExportPath(name)
    if (selected) outputPath.value = selected.endsWith('.aura') ? selected : `${selected}.aura`
  } catch (caught) { error.value = String(caught) }
}
const choosePackage = async () => {
  error.value = ''
  try {
    const selected = await projectsStore.chooseAuraPackage()
    if (selected) packagePath.value = selected
  } catch (caught) { error.value = String(caught) }
}
const exportPackage = async () => {
  error.value = ''; message.value = ''
  if (!projectId.value) { error.value = '请选择项目'; return }
  if (!selectedTaskIds.value.length) { error.value = '请至少选择一个任务'; return }
  if (!outputPath.value) { error.value = '请选择导出文件'; return }
  if (encrypt.value && password.value !== passwordConfirm.value) { error.value = '两次输入的密码不一致'; return }
  if (encrypt.value && !password.value) { error.value = '加密导出需要密码'; return }
  busy.value = true
  try {
    const result = await projectsStore.exportAura(
      projectId.value,
      selectedTaskIds.value,
      outputPath.value,
      encrypt.value ? password.value : null,
    )
    message.value = `已导出 ${result.task_count} 个任务（${result.encrypted ? '已加密' : '未加密'}）`
    password.value = ''; passwordConfirm.value = ''
  } catch (caught) { error.value = `导出失败：${String(caught)}` }
  finally { busy.value = false }
}
const previewPackage = async () => {
  error.value = ''; message.value = ''
  if (!projectId.value) { error.value = '请选择目标项目'; return }
  if (!packagePath.value) { error.value = '请选择 .aura 文件'; return }
  busy.value = true
  try {
    preview.value = await projectsStore.previewAuraImport(
      projectId.value,
      packagePath.value,
      password.value || null,
    )
  } catch (caught) { error.value = `预览失败：${String(caught)}` }
  finally { busy.value = false }
}
const importPackage = async () => {
  if (!preview.value || preview.value.has_conflicts) return
  error.value = ''; message.value = ''; busy.value = true
  try {
    const result = await projectsStore.importAura(
      projectId.value,
      packagePath.value,
      password.value || null,
      preview.value.package_sha256,
    )
    message.value = `已导入 ${result.imported.length} 个任务，项目看板已刷新`
    password.value = ''
    preview.value = null
  } catch (caught) { error.value = `导入失败：${String(caught)}` }
  finally { busy.value = false }
}
const close = () => {
  password.value = ''; passwordConfirm.value = ''
  emit('close')
}
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="close">
    <section class="task-modal aura-transfer-modal" role="dialog" aria-modal="true" aria-labelledby="aura-transfer-title">
      <header>
        <div><span class="modal-mark"><UiIcon name="archive"/></span><div><h2 id="aura-transfer-title">任务包导入与导出</h2><p>.aura 用于迁移未提交到 Git 的任务记录</p></div></div>
        <button class="icon-button" aria-label="关闭" :disabled="busy" @click="close"><UiIcon name="x"/></button>
      </header>
      <div class="modal-body transfer-layout">
        <div class="view-switch transfer-tabs" aria-label="操作类型">
          <button :class="{ active: mode === 'export' }" @click="mode = 'export'">导出</button>
          <button :class="{ active: mode === 'import' }" @click="mode = 'import'">导入</button>
        </div>

        <label class="field"><span>{{ mode === 'export' ? '来源项目' : '目标项目' }}</span><select v-model="projectId" :disabled="busy"><option v-for="item in projects" :key="item.registration.id" :value="item.registration.id">{{ item.project?.name ?? item.registration.path }}</option></select></label>

        <template v-if="mode === 'export'">
          <fieldset class="task-selection">
            <legend>选择任务 <small>{{ selectedTaskIds.length }}/{{ tasks.length }}</small></legend>
            <div class="selection-actions"><button class="text-button" type="button" @click="selectAllTasks">全选</button><button class="text-button" type="button" @click="selectedTaskIds = []">清空</button></div>
            <label v-for="task in tasks" :key="task.document.id ?? task.path" class="task-check"><input v-if="task.document.id" v-model="selectedTaskIds" type="checkbox" :value="task.document.id"/><span><strong>{{ task.document.id }}</strong>{{ task.document.title }}</span><small>{{ task.state }}</small></label>
            <p v-if="!tasks.length" class="field-help">这个项目还没有可导出的任务。</p>
          </fieldset>
          <label class="field"><span>导出文件</span><div class="path-picker"><input v-model="outputPath" readonly placeholder="选择新的 .aura 文件"/><button class="button secondary browse-button" type="button" :disabled="busy" @click="chooseExportPath">选择文件</button></div></label>
          <label class="encryption-toggle"><input v-model="encrypt" type="checkbox"/><span><strong>使用密码加密</strong><small>可选；未启用时包内任务内容可被读取</small></span></label>
          <div v-if="encrypt" class="form-grid">
            <label class="field"><span>密码</span><input v-model="password" type="password" autocomplete="new-password"/></label>
            <label class="field"><span>确认密码</span><input v-model="passwordConfirm" type="password" autocomplete="new-password"/></label>
          </div>
        </template>

        <template v-else>
          <label class="field"><span>任务包</span><div class="path-picker"><input v-model="packagePath" readonly placeholder="选择 .aura 文件"/><button class="button secondary browse-button" type="button" :disabled="busy" @click="choosePackage">选择文件</button></div></label>
          <label class="field"><span>密码 <small>仅加密包需要</small></span><input v-model="password" type="password" autocomplete="current-password" placeholder="普通包请留空"/></label>
          <p class="push-notice">导入先做完整性与冲突预览，不会直接写入任务。已有同 ID 任务不会被覆盖。</p>
          <section v-if="preview" class="import-preview" aria-live="polite">
            <header><strong>{{ preview.items.length }} 个任务</strong><span>{{ preview.encrypted ? '已验证加密包' : '普通包 · 内容未加密' }}</span></header>
            <div v-for="item in preview.items" :key="item.task_id" :class="['preview-row', { conflict: item.conflict }]">
              <strong>{{ item.task_id }}</strong><span>{{ item.relative_path }}</span><b>{{ item.conflict ? '冲突' : '可导入' }}</b>
            </div>
            <p v-if="preview.has_conflicts" class="form-error">发现 {{ conflictCount }} 个冲突。AuraPilot 不会覆盖目标项目中的现有任务。</p>
          </section>
        </template>

        <p v-if="message" class="push-result success" role="status">{{ message }}</p>
        <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      </div>
      <footer>
        <span class="push-safety">{{ busy ? '正在处理，界面仍可响应…' : '所有写入都在后台执行并返回明确结果' }}</span>
        <button class="button secondary" :disabled="busy" @click="close">关闭</button>
        <button v-if="mode === 'export'" class="button primary" :disabled="busy || !tasks.length" @click="exportPackage">{{ busy ? '正在导出…' : '导出任务包' }}</button>
        <button v-else-if="!preview" class="button primary" :disabled="busy" @click="previewPackage">{{ busy ? '正在验证…' : '预览导入' }}</button>
        <button v-else class="button primary" :disabled="busy || preview.has_conflicts" @click="importPackage">{{ busy ? '正在导入…' : '确认导入' }}</button>
      </footer>
    </section>
  </div>
</template>
