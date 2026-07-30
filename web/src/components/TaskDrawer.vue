<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import type { Diagnostic, LocatedTask, ProjectSnapshot, TaskState, TaskTransition } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const props = defineProps<{
  task: LocatedTask
  project: ProjectSnapshot
  diagnostics: Diagnostic[]
  busy?: boolean
  error?: string | null
}>()
const emit = defineEmits<{ close: []; edit: []; delete: []; push: []; transition: [input: TaskTransition] }>()

const transition = reactive({ target: '' as TaskState | '', assigned: '', branch: '' })
watch(() => props.task.path, () => {
  transition.target = ''
  transition.assigned = props.task.document.assigned ?? ''
  transition.branch = props.task.document.branch ?? ''
}, { immediate: true })

const progress = computed(() => typeof props.task.document.progress === 'number' ? props.task.document.progress : null)
const confirmTransition = () => {
  if (!transition.target) return
  emit('transition', {
    target: transition.target,
    assigned: transition.assigned || null,
    branch: transition.branch || null,
  })
}
</script>

<template>
  <aside class="task-drawer" role="dialog" aria-modal="false" :aria-label="`${task.document.id} 任务详情`">
    <header class="drawer-header">
      <div><span class="task-id large">{{ task.document.id }}</span><span :class="['priority', (task.document.priority ?? 'P3').toLowerCase()]">{{ task.document.priority }}</span></div>
      <button class="icon-button" aria-label="关闭任务详情" @click="$emit('close')"><UiIcon name="x"/></button>
    </header>
    <div class="drawer-content">
      <section class="detail-title"><span>{{ project.project?.name ?? project.registration.path }}</span><h1>{{ task.document.title }}</h1><p>{{ task.document.desc || '暂无任务描述。' }}</p></section>
      <dl class="detail-grid">
        <div><dt>状态</dt><dd>{{ task.state }}</dd></div><div><dt>类型</dt><dd>{{ task.document.type }}</dd></div>
        <div><dt>负责人</dt><dd>{{ task.document.assigned || '未分配' }}</dd></div><div><dt>分支</dt><dd class="mono">{{ task.document.branch || '—' }}</dd></div>
      </dl>
      <section v-if="progress !== null" class="detail-section"><div class="section-heading"><h2>进度</h2><span>{{ progress }}%</span></div><div class="progress-track large"><i :style="{ width: `${progress}%` }" /></div></section>
      <section class="detail-section"><h2>验收标准</h2><ul v-if="task.document.accept.length" class="accept-list"><li v-for="item in task.document.accept" :key="item"><span><UiIcon name="check" :size="14"/></span>{{ item }}</li></ul><p v-else class="muted">暂无验收标准。</p></section>
      <section v-if="task.document.blockers.length" class="detail-section blocker-section"><h2>阻塞项</h2><p v-for="blocker in task.document.blockers" :key="blocker">{{ blocker }}</p></section>
      <section v-if="task.document.log.length" class="detail-section"><h2>活动日志</h2><ol class="activity-list"><li v-for="(entry, index) in task.document.log" :key="index"><time>{{ entry.ts }}</time><span>{{ entry.msg }}</span></li></ol></section>
      <section v-if="diagnostics.length" class="detail-section task-diagnostics"><h2>诊断信息 <span>{{ diagnostics.length }}</span></h2><p v-for="item in diagnostics" :key="item.message">{{ item.message }}</p></section>
      <section class="detail-section transition-section">
        <h2>状态变更 <span class="optional-tag">非必选</span></h2>
        <p class="transition-hint">Push 只会发送任务指令，不会自动领取或改变任务状态；需要推进流程时再手动选择。</p>
        <select v-model="transition.target"><option value="">选择目标状态</option><option v-for="state in ['backlog','in-progress','in-review','done']" :key="state" :value="state" :disabled="state === task.state">{{ state }}</option></select>
        <div v-if="transition.target === 'in-progress'" class="transition-fields"><label class="field"><span>负责人</span><input v-model="transition.assigned" placeholder="例如 codex"/></label><label class="field"><span>分支</span><input v-model="transition.branch" placeholder="task/TASK-001"/></label></div>
        <p v-if="transition.target === 'done'" class="transition-hint done-hint">完成任务不会创建 Git Commit，也不会自动关联仓库最新提交。已有提交记录仍会保留。</p>
        <button v-if="transition.target" class="button secondary full-width" :disabled="busy" @click="confirmTransition">确认状态变更</button>
        <p v-if="error" class="form-error">{{ error }}</p>
      </section>
    </div>
    <footer class="drawer-footer"><button class="button secondary" @click="$emit('edit')"><UiIcon name="edit"/>编辑</button><button class="button danger" @click="$emit('delete')"><UiIcon name="trash"/>删除任务</button><button v-if="task.state === 'backlog'" class="button primary" @click="$emit('push')"><UiIcon name="send"/>Push</button><button v-else class="button primary" @click="$emit('close')">关闭</button></footer>
  </aside>
</template>
