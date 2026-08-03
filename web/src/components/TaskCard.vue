<script setup lang="ts">
import type { LocatedTask } from '../types/protocol'

const props = defineProps<{ task: LocatedTask; projectName: string; selected?: boolean }>()
defineEmits<{ open: [] }>()

const progress = () => {
  const value = props.task.document.progress
  return typeof value === 'number' ? Math.max(0, Math.min(100, value)) : null
}
</script>

<template>
  <button
    :class="['task-card', { blocked: task.document.blockers.length, selected }]"
    :data-task-state="task.state"
    :aria-label="`打开 ${task.document.id} ${task.document.title}`"
    @click="$emit('open')"
  >
    <div class="task-meta-line">
      <span class="task-id">{{ task.document.id ?? '未知 ID' }}</span>
      <span :class="['priority', (task.document.priority ?? 'P3').toLowerCase()]">{{ task.document.priority ?? '—' }}</span>
      <span v-if="Object.keys(task.document).some((key) => !['id','title','priority','type','created','assigned','branch','started','pr','waiting','completed','commit','desc','accept','log','blockers','progress'].includes(key))" class="extension-mark" title="含扩展元数据">i</span>
    </div>
    <strong>{{ task.document.title ?? '未命名任务' }}</strong>
    <span class="project-caption">{{ projectName }}</span>
    <div v-if="task.document.assigned || task.document.branch" class="task-runtime">
      <span v-if="task.document.assigned">◉ {{ task.document.assigned }}</span>
      <span v-if="task.document.branch" class="truncate">{{ task.document.branch }}</span>
      <span v-if="progress() !== null" class="progress-number">{{ progress() }}%</span>
    </div>
    <div v-if="progress() !== null" class="progress-track"><i :style="{ width: `${progress()}%` }" /></div>
    <div v-if="task.document.blockers.length" class="blocker-caption">{{ task.document.blockers[0] }}</div>
  </button>
</template>
