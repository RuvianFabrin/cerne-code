<script setup lang="ts">
import { ref, watch } from "vue";
import Dialog from "primevue/dialog";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean]; accepted: [] }>();

const countdown = ref(3);
let timer: ReturnType<typeof setInterval> | null = null;

watch(
  () => props.visible,
  (v) => {
    if (v) {
      countdown.value = 3;
      timer = setInterval(() => {
        countdown.value--;
        if (countdown.value <= 0 && timer) {
          clearInterval(timer);
          timer = null;
        }
      }, 1000);
    } else if (timer) {
      clearInterval(timer);
      timer = null;
    }
  },
);

function accept() {
  if (countdown.value > 0) return;
  emit("accepted");
}
</script>

<template>
  <Dialog
    :visible="props.visible"
    modal
    :closable="false"
    :closeOnEscape="false"
    :style="{ width: '480px' }"
    :header="$t('disclaimer.title')"
  >
    <div class="disclaimer-body">
      <div class="disclaimer-icon">
        <span class="msi">shield</span>
      </div>
      <p class="disclaimer-text" v-html="$t('disclaimer.text')"></p>
      <p class="disclaimer-subtext">{{ $t("disclaimer.subtext") }}</p>
    </div>
    <template #footer>
      <button
        class="btn-accept"
        :disabled="countdown > 0"
        @click="accept"
      >
        {{ countdown > 0 ? `${$t('disclaimer.wait')} ${countdown}s` : $t("disclaimer.accept") }}
      </button>
    </template>
  </Dialog>
</template>

<style scoped>
.disclaimer-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 8px 0;
}

.disclaimer-icon {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: #fef3c7;
  display: flex;
  align-items: center;
  justify-content: center;
}

.disclaimer-icon .msi {
  font-size: 28px;
  color: #b45309;
}

.disclaimer-text {
  font-size: 14px;
  line-height: 1.6;
  color: #3f3f46;
  text-align: center;
  margin: 0;
}

.disclaimer-subtext {
  font-size: 12px;
  color: #71717a;
  text-align: center;
  margin: 0;
}

.btn-accept {
  width: 100%;
  border: none;
  background: #18181b;
  color: #ffffff;
  border-radius: 8px;
  padding: 10px 14px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.btn-accept:disabled {
  background: #e4e4e7;
  color: #a1a1aa;
  cursor: default;
}
</style>
