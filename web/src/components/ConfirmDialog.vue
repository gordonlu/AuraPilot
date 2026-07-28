<script setup lang="ts">
import UiIcon from './UiIcon.vue'
defineProps<{ title: string; message: string; busy?: boolean; error?: string | null }>()
defineEmits<{ close: []; confirm: [] }>()
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="$emit('close')">
    <section class="confirm-modal" role="alertdialog" aria-modal="true">
      <span class="danger-icon"><UiIcon name="trash"/></span><h2>{{ title }}</h2><p>{{ message }}</p><p v-if="error" class="form-error">{{ error }}</p>
      <footer><button class="button secondary" @click="$emit('close')">取消</button><button class="button danger-filled" :disabled="busy" @click="$emit('confirm')">{{ busy ? '删除中…' : '确认删除' }}</button></footer>
    </section>
  </div>
</template>
