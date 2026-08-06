<script setup lang="ts">
import type { CSSProperties } from 'vue'

const SNOW_PARTICLE_COUNT = 26
const SNOW_PARTICLES = Array.from({ length: SNOW_PARTICLE_COUNT }, (_, index) => ({
  id: index,
  source: index % 4 === 0
    ? '/skins/polarscape/snowflake.webp'
    : '/skins/polarscape/snow-grain.webp',
  style: {
    '--polar-snow-left': `${(index * 41 + 7) % 98}%`,
    '--polar-snow-delay': `${-((index * 1.37) % 14).toFixed(2)}s`,
    '--polar-snow-duration': `${10 + (index % 7) * 1.15}s`,
    '--polar-snow-drift': `${-18 + (index % 6) * 8}px`,
    '--polar-snow-scale': `${0.46 + (index % 5) * 0.12}`,
  } as CSSProperties,
}))
</script>

<template>
  <div class="polarscape-board-decor" aria-hidden="true">
    <img
      class="polarscape-landscape"
      src="/skins/polarscape/background.webp"
      alt=""
      draggable="false"
    />
    <div class="polarscape-floe-field">
      <img class="polarscape-floe floe-one" src="/skins/polarscape/floe-1.webp" alt="" draggable="false" />
      <img class="polarscape-floe floe-two" src="/skins/polarscape/floe-2.webp" alt="" draggable="false" />
      <img class="polarscape-floe floe-three" src="/skins/polarscape/floe-3.webp" alt="" draggable="false" />
    </div>
    <div class="polarscape-snowfall">
      <img
        v-for="particle in SNOW_PARTICLES"
        :key="particle.id"
        class="polarscape-snow-particle"
        :src="particle.source"
        :style="particle.style"
        alt=""
        draggable="false"
      />
    </div>
  </div>
</template>
