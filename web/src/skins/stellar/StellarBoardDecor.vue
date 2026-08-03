<template>
  <div class="stellar-board-decor" aria-hidden="true">
    <div class="stellar-grid" />
    <div class="stellar-stars stellar-stars-near" />
    <div class="stellar-stars stellar-stars-far" />
    <div class="stellar-nebula stellar-nebula-a" />
    <div class="stellar-nebula stellar-nebula-b" />

    <svg class="stellar-route" viewBox="0 0 1200 150" preserveAspectRatio="none">
      <path
        id="stellar-flight-path"
        d="M-40 98 C 150 30, 310 132, 505 72 C 690 16, 850 122, 1030 58 C 1115 28, 1190 68, 1240 42"
      />
      <g class="stellar-rocket">
        <path class="stellar-rocket-body" d="M8 0 L-7 5 L-3 0 L-7-5 Z" />
        <circle class="stellar-rocket-flame" cx="-9" cy="0" r="2.2" />
        <animateMotion dur="24s" repeatCount="indefinite" rotate="auto">
          <mpath href="#stellar-flight-path" />
        </animateMotion>
      </g>
    </svg>

    <span class="stellar-spark spark-a" />
    <span class="stellar-spark spark-b" />
    <span class="stellar-spark spark-c" />
    <span class="stellar-spark spark-d" />
    <span class="stellar-destination" />
    <span class="stellar-comet" />

    <svg class="stellar-orbits" viewBox="0 0 420 260">
      <ellipse cx="210" cy="132" rx="158" ry="63" />
      <ellipse cx="210" cy="132" rx="112" ry="42" />
      <circle class="stellar-orbit-core" cx="210" cy="132" r="18" />
      <circle class="stellar-satellite" cx="61" cy="111" r="5" />
      <circle class="stellar-satellite satellite-b" cx="313" cy="171" r="4" />
    </svg>
  </div>
</template>
