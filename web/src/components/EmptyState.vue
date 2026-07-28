<script setup lang="ts">
import UiIcon from './UiIcon.vue'
defineEmits<{ add: [] }>()
</script>

<template>
  <section class="empty-state">
    <UiIcon name="compass" :size="62" />
    <h1>AuraPilot</h1>
    <p>把本地项目接入统一任务控制台</p>
    <button class="button primary" @click="$emit('add')"><UiIcon name="plus"/>添加已有项目</button>
    <ol class="onboarding-steps">
      <li><b>1</b><span><strong>选择项目</strong>选择包含 <code>.aurapilot/</code> 的仓库</span></li>
      <li><b>2</b><span><strong>验证协议</strong>自动扫描项目和任务文件</span></li>
      <li><b>3</b><span><strong>开始管理</strong>在看板中创建、编辑和流转任务</span></li>
    </ol>
    <small>项目尚未初始化？Phase 5 将提供完整初始化流程。</small>
  </section>
</template>
