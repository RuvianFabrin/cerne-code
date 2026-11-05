<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface OverlayAction {
  x: number | null;
  y: number | null;
  stamp: number;
}

// Posicao do anel, em pixels CSS da janela (o backend manda pixels fisicos).
const ring = ref<{ x: number; y: number; key: number } | null>(null);
let unlisten: UnlistenFn | null = null;

onMounted(async () => {
  unlisten = await listen<OverlayAction>("overlay:action", (event) => {
    const { x, y, stamp } = event.payload;
    if (x == null || y == null) {
      ring.value = null;
      return;
    }
    const scale = window.devicePixelRatio || 1;
    ring.value = { x: x / scale, y: y / scale, key: stamp };
  });
});

onUnmounted(() => unlisten?.());
</script>

<template>
  <div class="cu-frame">
    <div class="cu-banner">Computer Use</div>
    <div
      v-if="ring"
      :key="ring.key"
      class="cu-ring"
      :style="{ left: `${ring.x}px`, top: `${ring.y}px` }"
    ></div>
  </div>
</template>

<style>
/* Janela transparente: zera o fundo do documento inteiro. */
html,
body,
#app {
  background: transparent !important;
  margin: 0;
  height: 100%;
  overflow: hidden;
}
</style>

<style scoped>
.cu-frame {
  position: fixed;
  inset: 0;
  box-sizing: border-box;
  border: 4px solid #2563eb;
  pointer-events: none;
}

.cu-banner {
  position: absolute;
  top: 10px;
  left: 50%;
  transform: translateX(-50%);
  padding: 6px 14px;
  border-radius: 999px;
  background: #2563eb;
  color: #fff;
  font: 500 13px/1 system-ui, -apple-system, "Segoe UI", sans-serif;
  white-space: nowrap;
}

.cu-ring {
  position: absolute;
  width: 28px;
  height: 28px;
  margin: -14px 0 0 -14px;
  border: 3px solid #2563eb;
  border-radius: 50%;
  animation: cu-pulse 0.9s ease-out 2;
}

@keyframes cu-pulse {
  from {
    transform: scale(0.6);
    opacity: 1;
  }
  to {
    transform: scale(1.8);
    opacity: 0;
  }
}
</style>
