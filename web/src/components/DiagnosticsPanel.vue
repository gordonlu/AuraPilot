<script setup lang="ts">
import type { ProjectSnapshot } from '../types/protocol'
import UiIcon from './UiIcon.vue'

defineProps<{ snapshots: ProjectSnapshot[] }>()
defineEmits<{ close: [] }>()
</script>

<template>
  <aside class="diagnostics-panel" aria-label="协议诊断">
    <header><div><UiIcon name="diagnostic"/><h2>协议诊断</h2></div><button class="icon-button" aria-label="关闭诊断" @click="$emit('close')"><UiIcon name="x"/></button></header>
    <div class="diagnostics-content">
      <template v-for="snapshot in snapshots" :key="snapshot.registration.id">
        <section v-if="snapshot.diagnostics.length" class="diagnostic-group">
          <h3>{{ snapshot.project?.name ?? snapshot.registration.path }}</h3>
          <article v-for="(item, index) in snapshot.diagnostics" :key="`${item.path}-${index}`" :class="['diagnostic-item', item.severity]">
            <div><strong>{{ item.code.replaceAll('_', ' ') }}</strong><span>{{ item.severity }}</span></div>
            <p>{{ item.message }}</p><code v-if="item.path">{{ item.path }}</code>
          </article>
        </section>
      </template>
      <div v-if="!snapshots.some((item) => item.diagnostics.length)" class="panel-empty"><span>✓</span><h3>没有发现协议问题</h3><p>所有已扫描任务均可正常读取。</p></div>
    </div>
  </aside>
</template>
