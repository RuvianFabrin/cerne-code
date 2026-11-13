<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

// Editor do prompt da "entrega final" do modo Anything. O texto padrão vem do Cerne; quem quiser escreve o
// próprio e pode voltar ao padrão com "Redefinir". `value` vazio = sem texto próprio (vale o padrão, ou o
// do nível acima). Emite `save` com o texto escrito, ou "" quando ele é igual ao padrão ou foi redefinido.
const props = defineProps<{
  value: string;
  /** O que vale quando `value` está vazio (o padrão do Cerne, ou o da Configuração para a sessão). */
  fallback: string;
  resetLabel?: string;
}>();
const emit = defineEmits<{ save: [value: string] }>();
const { t } = useI18n();

const text = ref("");
const saved = ref(false);
let timer: ReturnType<typeof setTimeout> | undefined;

watch(
  () => [props.value, props.fallback],
  () => {
    text.value = props.value.trim() ? props.value : props.fallback;
  },
  { immediate: true },
);

function flash() {
  saved.value = true;
  clearTimeout(timer);
  timer = setTimeout(() => (saved.value = false), 2000);
}

function save() {
  const igual = text.value.trim() === props.fallback.trim();
  emit("save", igual ? "" : text.value);
  flash();
}

function reset() {
  text.value = props.fallback;
  emit("save", "");
  flash();
}
</script>

<template>
  <div class="afp">
    <textarea v-model="text" class="afp-text" rows="10" spellcheck="false" @blur="save" />
    <p class="afp-hint">{{ t("anythingFinal.placeholders") }}</p>
    <div class="afp-row">
      <button type="button" class="afp-btn" :disabled="!value.trim()" @click="reset">{{ resetLabel || t("anythingFinal.reset") }}</button>
      <span v-if="saved" class="afp-saved">{{ t("anythingFinal.saved") }}</span>
    </div>
  </div>
</template>

<style scoped>
.afp { display: flex; flex-direction: column; gap: 6px; }
.afp-text { width: 100%; box-sizing: border-box; resize: vertical; padding: 8px; border: 1px solid var(--border, #d0d7de); border-radius: 8px; background: transparent; color: inherit; font: inherit; font-size: 12.5px; line-height: 1.45; }
.afp-hint { margin: 0; opacity: 0.7; font-size: 12px; }
.afp-row { display: flex; align-items: center; gap: 10px; }
.afp-btn { padding: 4px 12px; border: 1px solid var(--border, #d0d7de); border-radius: 6px; background: transparent; color: inherit; cursor: pointer; }
.afp-btn:disabled { opacity: 0.45; cursor: default; }
.afp-saved { color: #2e7d32; font-size: 12px; }
</style>
