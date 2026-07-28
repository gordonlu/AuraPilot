<script setup lang="ts">
import brandMark from '../assets/aurapilot-mark.webp'
import UiIcon from './UiIcon.vue'
defineEmits<{ add: [] }>()
</script>

<template>
  <section class="empty-state">
    <img :src="brandMark" alt="AuraPilot" class="empty-brand-mark" />
    <h1>AuraPilot</h1>
    <p>把本地项目接入统一任务控制台</p>
    <button class="button primary" @click="$emit('add')"><UiIcon name="plus"/>接入本地项目</button>
    <ol class="onboarding-steps">
      <li><b>1</b><span><strong>选择目录</strong>从系统目录选择器定位代码仓库</span></li>
      <li><b>2</b><span><strong>准备协议</strong>自动验证或创建 <code>.aurapilot/</code></span></li>
      <li><b>3</b><span><strong>开始管理</strong>在看板中创建、编辑和流转任务</span></li>
    </ol>
    <small>尚未初始化的项目可以在接入时直接创建协议。</small>
  </section>
</template>
