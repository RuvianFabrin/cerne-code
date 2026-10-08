<script setup lang="ts">
defineProps<{ up: boolean | null; loading?: boolean }>();
</script>

<template>
  <span
    class="status-dot"
    :class="{ loading: loading, up: !loading && up === true, down: !loading && up === false, unknown: !loading && up === null }"
    v-tooltip.top="loading ? $t('statusDot.loading') : up === null ? $t('statusDot.checking') : up ? $t('statusDot.serverUp') : $t('statusDot.serverDown')"
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

.status-dot.loading { background: #f59e0b; }

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
