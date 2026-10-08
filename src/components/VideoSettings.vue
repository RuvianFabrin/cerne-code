<script setup lang="ts">
import MediaApiSelector from "./MediaApiSelector.vue";
import { ref, onMounted } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import { api } from "../api";
import { useProviderStore } from "../stores/provider";
const store = useProviderStore();
const { t } = useI18n();
const key = ref("");
const hasKey = ref(false);
const notice = ref("");
const selector = ref<InstanceType<typeof MediaApiSelector> | null>(null);
onMounted(async () => { hasKey.value = await api.hasVideoGenKey(); });
async function save() {
  try {
    if ((!store.config?.video_gen.provider || store.config.video_gen.provider === "compatible") && key.value.trim()) { await api.setVideoGenKey(key.value.trim()); key.value = ""; hasKey.value = true; }
    if (store.config?.video_gen.provider && store.config.video_gen.provider !== "compatible") { if (!await selector.value?.save()) return; }
    else await store.saveConfig();
    notice.value = t('media.saved');
  } catch (e) { notice.value = String(e); }
}
async function removeKey() {
  try { await api.clearVideoGenKey(); hasKey.value = false; } catch (e) { notice.value = String(e); }
}
async function chooseFolder() {
  const folder = await open({ directory: true, multiple: false });
  if (typeof folder === "string" && store.config) { store.config.video_gen.output_dir = folder; await save(); }
}
</script>
<template>
  <section v-if="store.config" class="video-settings">
    <h3>{{ t('media.videoSettings') }}</h3>
    <MediaApiSelector ref="selector" kind="video" />
    <template v-if="!store.config.video_gen.provider || store.config.video_gen.provider === 'compatible'">
    <p class="hint">{{ t('media.videoHint') }}</p>
    <label>{{ t('media.baseUrl') }}<input class="text-input" v-model="store.config.video_gen.base_url" placeholder="http://127.0.0.1:8000/v1" /></label>
    <label>{{ t('media.model') }}<input class="text-input" v-model="store.config.video_gen.model" /></label>
    <label>{{ t('media.protocol') }}<select class="text-input" v-model="store.config.video_gen.protocol">
      <option value="">JSON · POST /videos</option><option value="multipart">Multipart · POST /videos + GET /videos/id</option>
    </select></label>
    </template>
    <div class="fields"><label>{{ t('media.seconds') }}<input class="text-input" v-model="store.config.video_gen.seconds" placeholder="4" /></label>
      <label v-if="!store.config.video_gen.provider || ['compatible', 'openrouter'].includes(store.config.video_gen.provider)">{{ t('media.size') }}<input class="text-input" v-model="store.config.video_gen.size" placeholder="1280x720" /></label></div>
    <div v-if="store.config.video_gen.provider && store.config.video_gen.provider !== 'compatible'" class="fields"><label>{{ t('mediaApi.aspectRatio') }}<input class="text-input" v-model="store.config.video_gen.aspect_ratio" placeholder="16:9" /></label><label>{{ t('mediaApi.resolution') }}<input class="text-input" v-model="store.config.video_gen.resolution" placeholder="720p" /></label></div>
    <label v-if="!store.config.video_gen.provider || store.config.video_gen.provider === 'compatible'">{{ t('media.apiKey') }}<input class="text-input" type="password" autocomplete="new-password" v-model="key" :placeholder="hasKey ? t('media.keyConfigured') : t('media.optional')" /></label>
    <label>{{ t('media.folder') }}<input class="text-input" readonly :value="store.config.video_gen.output_dir" /></label>
    <div class="actions"><button class="btn-secondary" @click="chooseFolder">{{ t('media.chooseFolder') }}</button>
      <button v-if="hasKey && (!store.config.video_gen.provider || store.config.video_gen.provider === 'compatible')" class="btn-secondary" @click="removeKey">{{ t('media.removeKey') }}</button>
      <button class="btn-primary" @click="save">{{ t('media.save') }}</button></div>
    <p aria-live="polite">{{ notice }}</p>
  </section>
</template>
<style scoped>
.video-settings { margin-top: 24px; padding-top: 16px; border-top: 1px solid #dedee7; }
label { display: flex; flex-direction: column; gap: 6px; margin: 12px 0; font-size: 13px; font-weight: 600; }
.fields, .actions { display: flex; gap: 10px; flex-wrap: wrap; }
.fields label { flex: 1; }
.hint { color: #71717a; font-size: 13px; }
.text-input { border: var(--cerne-border); border-radius: 8px; padding: 8px 10px; font: inherit; font-weight: 500; min-width: 0; }
.btn-secondary, .btn-primary { border: var(--cerne-border); border-radius: 8px; padding: 7px 12px; font: inherit; font-size: 13px; font-weight: 600; cursor: pointer; }
.btn-secondary { background: white; color: #52525b; }
.btn-primary { background: #18181b; color: white; }
</style>
