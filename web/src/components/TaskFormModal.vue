<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import type { LocatedTask, ProjectSnapshot, TaskDraft } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{
  mode: 'create' | 'edit'
  projects: ProjectSnapshot[]
  projectId: string
  task?: LocatedTask | null
  busy?: boolean
  error?: string | null
}>()

const emit = defineEmits<{ close: []; save: [projectId: string, draft: TaskDraft] }>()
const form = reactive({ projectId: '', title: '', priority: 'P1', task_type: 'feature', desc: '', accept: '' })

const hydrate = () => {
  form.projectId = props.projectId || props.projects[0]?.registration.id || ''
  form.title = props.task?.document.title ?? ''
  form.priority = props.task?.document.priority ?? 'P1'
  form.task_type = props.task?.document.type ?? 'feature'
  form.desc = props.task?.document.desc ?? ''
  form.accept = props.task?.document.accept.join('\n') ?? ''
}
watch(() => [props.task, props.projectId], hydrate, { immediate: true })

const valid = computed(() => Boolean(form.projectId && form.title.trim()))
const submit = () => {
  if (!valid.value) return
  emit('save', form.projectId, {
    title: form.title.trim(), priority: form.priority, task_type: form.task_type,
    desc: form.desc.trim() || null,
    accept: form.accept.split('\n').map((line) => line.trim()).filter(Boolean),
  })
}
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="$emit('close')">
    <form class="task-modal" role="dialog" aria-modal="true" :aria-label="mode === 'create' ? '新建任务' : '编辑任务'" @submit.prevent="submit">
      <header><div><span class="modal-mark"><UiIcon :name="mode === 'create' ? 'plus' : 'edit'"/></span><div><h2>{{ mode === 'create' ? '新建任务' : '编辑任务' }}</h2><p>{{ mode === 'create' ? '任务将安全写入所选项目的 backlog' : task?.document.id }}</p></div></div><button type="button" class="icon-button" aria-label="关闭" @click="$emit('close')"><UiIcon name="x"/></button></header>
      <div class="modal-body form-grid">
        <label class="field full"><span>项目</span><select v-model="form.projectId" :disabled="mode === 'edit'"><option v-for="project in projects" :key="project.registration.id" :value="project.registration.id">{{ project.project?.name ?? project.registration.path }}</option></select></label>
        <label class="field full"><span>标题 <i>*</i></span><input v-model="form.title" maxlength="120" autofocus placeholder="清晰描述需要完成的工作" /></label>
        <label class="field"><span>优先级</span><select v-model="form.priority"><option v-for="priority in ['P0','P1','P2','P3']" :key="priority">{{ priority }}</option></select></label>
        <label class="field"><span>类型</span><select v-model="form.task_type"><option v-for="type in ['feature','bug','refactor','docs','test','chore']" :key="type">{{ type }}</option></select></label>
        <label class="field full"><span>描述</span><textarea v-model="form.desc" rows="4" placeholder="说明背景、范围和关键约束" /></label>
        <label class="field full"><span>验收标准 <small>每行一项</small></span><textarea v-model="form.accept" rows="5" placeholder="功能按预期工作&#10;相关测试通过" /></label>
        <p v-if="error" class="form-error">{{ error }}</p>
      </div>
      <footer><button type="button" class="button secondary" @click="$emit('close')">取消</button><button class="button primary" :disabled="!valid || busy">{{ busy ? '保存中…' : mode === 'create' ? '创建任务' : '保存更改' }}</button></footer>
    </form>
  </div>
</template>
