<script setup lang="ts">
import { computed } from 'vue'
import type { LocatedTask, ProjectSnapshot } from '../types/protocol'
import TaskCard from './TaskCard.vue'

const props = defineProps<{ snapshots: ProjectSnapshot[]; search: string }>()
defineEmits<{ open: [projectId: string, task: LocatedTask]; back: [] }>()

const blocked = computed(() => {
  const query = props.search.trim().toLocaleLowerCase()
  return props.snapshots.flatMap((snapshot) => snapshot.tasks
    .filter((task) => task.document.blockers.length)
    .filter((task) => !query || [task.document.id, task.document.title, ...task.document.blockers]
      .some((value) => value?.toLocaleLowerCase().includes(query)))
    .map((task) => ({ snapshot, task })))
})
</script>

<template>
  <section class="blocked-view">
    <header class="blocked-heading">
      <div><span class="warning-symbol">!</span><h1>阻塞聚焦</h1><b>{{ blocked.length }} 个任务需要介入</b></div>
      <button class="button secondary" @click="$emit('back')">返回看板</button>
    </header>
    <div v-if="blocked.length" class="blocked-list">
      <article v-for="item in blocked" :key="item.task.path" class="blocked-row">
        <TaskCard
          :task="item.task"
          :project-name="item.snapshot.project?.name ?? '未命名项目'"
          @open="$emit('open', item.snapshot.registration.id, item.task)"
        />
        <div class="blocker-detail">
          <span>当前阻塞</span>
          <p v-for="blocker in item.task.document.blockers" :key="blocker">{{ blocker }}</p>
        </div>
        <button class="button secondary" @click="$emit('open', item.snapshot.registration.id, item.task)">查看详情</button>
      </article>
    </div>
    <div v-else class="resolved-empty"><span>✓</span><h2>当前没有阻塞任务</h2><p>所有项目都可以继续向前推进。</p></div>
  </section>
</template>
