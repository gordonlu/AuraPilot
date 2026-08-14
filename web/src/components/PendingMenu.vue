<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { usePendingStore } from '../stores/pending'
import type { PendingItem, PendingTarget } from '../types/protocol'
import UiIcon from './UiIcon.vue'

const emit = defineEmits<{ select: [target: PendingTarget] }>()
const pending = usePendingStore()
const open = ref(false)
const root = ref<HTMLElement | null>(null)
const hasItems = computed(() => pending.count > 0)

const relativeTime = (iso: string) => {
  const timestamp = new Date(iso).getTime()
  if (Number.isNaN(timestamp)) return ''
  const minutes = Math.max(0, Math.round((Date.now() - timestamp) / 60_000))
  if (minutes < 1) return '刚刚'
  if (minutes < 60) return `${minutes} 分钟前`
  if (minutes < 1_440) return `${Math.round(minutes / 60)} 小时前`
  return new Date(iso).toLocaleDateString('zh-CN')
}
const choose = (item: PendingItem) => {
  emit('select', pending.targetFor(item))
  open.value = false
}
const onOutside = (event: MouseEvent) => {
  if (root.value && !root.value.contains(event.target as Node)) open.value = false
}
const onKey = (event: KeyboardEvent) => {
  if (event.key === 'Escape') open.value = false
}
onMounted(() => {
  document.addEventListener('mousedown', onOutside)
  document.addEventListener('keydown', onKey)
})
onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onOutside)
  document.removeEventListener('keydown', onKey)
})
</script>

<template>
  <div ref="root" class="pending-menu">
    <button
      :class="['button', 'pending-button', { active: open, attention: hasItems }]"
      :title="hasItems ? `${pending.count} 项待处理` : '没有待处理事项'"
      aria-haspopup="menu"
      :aria-expanded="open"
      @click="open = !open; if (open) pending.refresh()"
    >
      <UiIcon name="alert"/><span>待处理</span><b>{{ pending.count }}</b>
    </button>
    <div v-if="open" class="pending-popover" role="menu" aria-label="待处理事项">
      <header><strong>待处理</strong><button :disabled="pending.loading" @click="pending.refresh()"><UiIcon name="diagnostic" :size="12"/>{{ pending.loading ? '刷新中' : '刷新' }}</button></header>
      <p v-if="pending.error" class="pending-error" role="alert">{{ pending.error }}</p>
      <div v-if="!hasItems && !pending.loading" class="pending-empty"><UiIcon name="check" :size="20"/><strong>当前没有需要你处理的事项</strong><span>已处理审批和已修复问题不会保留在这里。</span></div>
      <div v-else class="pending-groups">
        <section v-for="group in pending.byProject" :key="group.id" class="pending-group">
          <h3>{{ group.name }}</h3>
          <button v-for="item in group.items" :key="`${item.kind}-${item.approval_id || item.path}-${item.title}`" :class="['pending-item', item.kind]" role="menuitem" @click="choose(item)">
            <span class="pending-item-icon"><UiIcon :name="item.kind === 'approval' ? 'terminal' : 'diagnostic'" :size="14"/></span>
            <span class="pending-item-body">
              <span><em>{{ item.kind === 'approval' ? '等待审批' : item.repair_kind === 'manual' ? '人工处理' : '确认修复' }}</em><code v-if="item.task_id">{{ item.task_id }}</code></span>
              <strong>{{ item.title }}</strong><small v-if="item.detail">{{ item.detail }}</small>
            </span>
            <time v-if="item.kind === 'approval'">{{ relativeTime(item.created_at) }}</time>
          </button>
        </section>
      </div>
      <footer v-if="pending.lastUpdated">更新于 {{ pending.lastUpdated.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }) }}</footer>
    </div>
  </div>
</template>
