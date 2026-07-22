<script setup lang="ts">
defineProps<{ up: boolean | null }>();
</script>

<template>
  <span
    class="status-dot"
    :class="{ up: up === true, down: up === false, unknown: up === null }"
    v-tooltip.top="up === null ? 'Verificando...' : up ? 'Servidor rodando' : 'Servidor parado'"
  />
</template>

<style scoped>
.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 999px;
  flex-shrink: 0;
}

.status-dot.up {
  background: #22c55e;
  box-shadow: 0 0 0 2px rgba(34, 197, 94, 0.18);
}

.status-dot.down {
  background: #dc2626;
}

.status-dot.unknown {
  background: #d4d4d8;
}
</style>
