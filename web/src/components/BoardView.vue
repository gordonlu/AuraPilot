<script setup lang="ts">
import { computed } from 'vue'
import type { LocatedTask, ProjectSnapshot, TaskState } from '../types/protocol'
import SeascapeBoardDecor from '../skins/seascape/SeascapeBoardDecor.vue'
import StellarBoardDecor from '../skins/stellar/StellarBoardDecor.vue'
import PolarscapeBoardDecor from '../skins/polarscape/PolarscapeBoardDecor.vue'
import TaskCard from './TaskCard.vue'

const props = defineProps<{
  snapshots: ProjectSnapshot[]
  search: string
  selectedKey: string | null
}>()

defineEmits<{ open: [projectId: string, task: LocatedTask] }>()

const states: Array<{ key: TaskState; label: string; terrain: string; sector: string; polar: string }> = [
  { key: 'backlog', label: 'Backlog', terrain: '沙滩', sector: '星港', polar: '极地营地' },
  { key: 'in-progress', label: 'In Progress', terrain: '潮间带', sector: '巡航中', polar: '风雪带' },
  { key: 'in-review', label: 'In Review', terrain: '浅海', sector: '轨道校验', polar: '冰原' },
  { key: 'done', label: 'Done', terrain: '深海', sector: '抵达星域', polar: '极光海' },
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
    <SeascapeBoardDecor />
    <StellarBoardDecor />
    <PolarscapeBoardDecor />
    <section
      v-for="column in grouped"
      :key="column.key"
      class="board-column"
      :data-task-state="column.key"
    >
      <header class="column-header">
        <div class="column-title">
          <small class="seascape-column-label">{{ column.terrain }}</small>
          <small class="stellar-column-label">{{ column.sector }}</small>
          <small class="polarscape-column-label">{{ column.polar }}</small>
          <h2>{{ column.label }}</h2>
        </div>
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
