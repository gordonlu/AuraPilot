<script setup lang="ts">
import { computed } from 'vue'
import type { LocatedTask, ProjectSnapshot, TaskState } from '../types/protocol'
import TaskCard from './TaskCard.vue'

const props = defineProps<{
  snapshots: ProjectSnapshot[]
  search: string
  selectedKey: string | null
}>()

defineEmits<{ open: [projectId: string, task: LocatedTask] }>()

const states: Array<{ key: TaskState; label: string }> = [
  { key: 'backlog', label: 'Backlog' },
  { key: 'in-progress', label: 'In Progress' },
  { key: 'in-review', label: 'In Review' },
  { key: 'done', label: 'Done' },
]

const grouped = computed(() => {
  const query = props.search.trim().toLocaleLowerCase()
  return states.map((state) => ({
    ...state,
    groups: props.snapshots
      .map((snapshot) => ({
        snapshot,
        tasks: snapshot.tasks.filter((task) => {
          if (task.state !== state.key) return false
          if (!query) return true
          return [task.document.id, task.document.title, task.document.assigned, snapshot.project?.name]
            .some((value) => value?.toLocaleLowerCase().includes(query))
        }),
      }))
      .filter((group) => group.tasks.length),
  }))
})
</script>

<template>
  <div class="board-grid" aria-label="跨项目任务看板">
    <section v-for="column in grouped" :key="column.key" class="board-column">
      <header class="column-header">
        <h2>{{ column.label }}</h2>
        <span>{{ column.groups.reduce((sum, group) => sum + group.tasks.length, 0) }}</span>
      </header>
      <div class="column-content">
        <div v-if="!column.groups.length" class="column-empty">暂无任务</div>
        <section v-for="group in column.groups" :key="group.snapshot.registration.id" class="project-group">
          <header class="group-header">
            <span :class="['health', group.snapshot.project?.health ?? 'unknown']" />
            <strong>{{ group.snapshot.project?.name ?? group.snapshot.registration.path.split('/').at(-1) }}</strong>
            <span>{{ group.tasks.length }}</span>
          </header>
          <TaskCard
            v-for="task in group.tasks"
            :key="task.path"
            :task="task"
            :project-name="group.snapshot.project?.name ?? '未命名项目'"
            :selected="selectedKey === `${group.snapshot.registration.id}:${task.document.id}`"
            @open="$emit('open', group.snapshot.registration.id, task)"
          />
        </section>
      </div>
    </section>
  </div>
</template>
