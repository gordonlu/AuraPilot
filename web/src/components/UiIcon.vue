<script setup lang="ts">
defineProps<{ name: string; size?: number }>()
</script>

<template>
  <svg
    class="ui-icon"
    :width="size ?? 18"
    :height="size ?? 18"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-if="name === 'compass'">
      <circle cx="12" cy="12" r="9"/><path d="m15.3 7.7-1.8 5.8-5.8 2.8 2.8-5.8 4.8-2.8Z"/>
    </template>
    <template v-else-if="name === 'folder'">
      <path d="M3 6.8a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8.5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/>
    </template>
    <template v-else-if="name === 'board'">
      <rect x="3" y="4" width="7" height="16" rx="1.5"/><rect x="14" y="4" width="7" height="9" rx="1.5"/>
    </template>
    <template v-else-if="name === 'alert'">
      <path d="M10.3 4.2 2.8 17.4A1.7 1.7 0 0 0 4.3 20h15.4a1.7 1.7 0 0 0 1.5-2.6L13.7 4.2a2 2 0 0 0-3.4 0Z"/><path d="M12 9v4"/><path d="M12 17h.01"/>
    </template>
    <template v-else-if="name === 'search'">
      <circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/>
    </template>
    <template v-else-if="name === 'plus'">
      <path d="M12 5v14M5 12h14"/>
    </template>
    <template v-else-if="name === 'sun'">
      <circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>
    </template>
    <template v-else-if="name === 'moon'">
      <path d="M20.5 15.5A8.7 8.7 0 0 1 8.5 3.5a8.7 8.7 0 1 0 12 12Z"/>
    </template>
    <template v-else-if="name === 'palette'">
      <path d="M12 3a9 9 0 0 0 0 18h1.5a1.8 1.8 0 0 0 1.2-3.2 1.8 1.8 0 0 1 1.2-3.2H18A3 3 0 0 0 21 12a9 9 0 0 0-9-9Z"/>
      <circle cx="7.5" cy="10" r=".8" fill="currentColor" stroke="none"/><circle cx="10" cy="6.8" r=".8" fill="currentColor" stroke="none"/><circle cx="14" cy="6.8" r=".8" fill="currentColor" stroke="none"/>
    </template>
    <template v-else-if="name === 'diagnostic'">
      <path d="M12 3v3M12 18v3M3 12h3M18 12h3"/><circle cx="12" cy="12" r="6"/><path d="m9.5 12 1.6 1.6 3.5-3.7"/>
    </template>
    <template v-else-if="name === 'x'">
      <path d="m6 6 12 12M18 6 6 18"/>
    </template>
    <template v-else-if="name === 'edit'">
      <path d="M13.5 6.5 17.5 10.5M4 20l4.5-1 10-10a2.8 2.8 0 0 0-4-4l-10 10L4 20Z"/>
    </template>
    <template v-else-if="name === 'trash'">
      <path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5"/>
    </template>
    <template v-else-if="name === 'check'">
      <path d="m5 12 4 4L19 6"/>
    </template>
    <template v-else-if="name === 'send'">
      <path d="m3 11 18-8-7 18-3-7-8-3Z"/><path d="m11 14 4-5"/>
    </template>
    <template v-else-if="name === 'terminal'">
      <rect x="3" y="5" width="18" height="14" rx="2"/><path d="m7 9 3 3-3 3M13 15h4"/>
    </template>
    <template v-else-if="name === 'git-branch'">
      <circle cx="6" cy="5" r="2"/><circle cx="18" cy="7" r="2"/><circle cx="6" cy="19" r="2"/><path d="M6 7v10M8 12h3a7 7 0 0 0 7-3"/>
    </template>
    <template v-else-if="name === 'chevron'">
      <path d="m9 18 6-6-6-6"/>
    </template>
    <template v-else>
      <circle cx="12" cy="12" r="8"/>
    </template>
  </svg>
</template>
