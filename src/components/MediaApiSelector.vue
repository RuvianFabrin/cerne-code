<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api, type MediaConnection, type MediaModel } from '../api';
import { useProviderStore } from '../stores/provider';
const props = defineProps<{ kind: 'image' | 'video' }>();
const store = useProviderStore();
const { t } = useI18n();
const cfg = computed(() => props.kind === 'image' ? store.config!.image_gen : store.config!.video_gen);
const connections = ref<MediaConnection[]>([]);
const models = ref<MediaModel[]>([]);
const key = ref('');
const hasKey = ref(false);
const notice = ref('');
const busy = ref(false);
const parameters = ref('');
const bases: Record<string, string> = { openrouter: 'https://openrouter.ai/api/v1', openai: 'https://api.openai.com/v1', gemini: 'https://generativelanguage.googleapis.com/v1beta', xai: 'https://api.x.ai/v1', fal: 'https://queue.fal.run' };
const available = computed(() => connections.value.filter(c => c.provider === cfg.value.provider));
const selected = computed(() => available.value.find(c => c.id === cfg.value.connection_id));
const vendor = computed(() => !!cfg.value.provider && cfg.value.provider !== 'compatible');
async function refreshKey() {
  hasKey.value = cfg.value.connection_id ? !!selected.value?.has_key : vendor.value ? await api.hasMediaProviderKey(cfg.value.provider!) : false;
}
onMounted(async () => {
  parameters.value = cfg.value.parameters ? JSON.stringify(cfg.value.parameters, null, 2) : '';
  try { connections.value = await api.listMediaConnections(); await refreshKey(); } catch (e) { notice.value = String(e); }
});
async function chooseProvider() {
  models.value = []; key.value = ''; notice.value = ''; parameters.value = '';
  cfg.value.model = ''; cfg.value.parameters = null;
  cfg.value.base_url = bases[cfg.value.provider ?? ''] ?? '';
  cfg.value.connection_id = available.value.find(c => c.has_key)?.id ?? (cfg.value.provider === 'openrouter' ? 'openrouter' : '');
  // Pixel size belongs to compatible/OpenRouter APIs; other vendors use ratio/resolution.
  if (props.kind === 'video' && cfg.value.provider !== 'openrouter') store.config!.video_gen.size = '';
  try { await refreshKey(); await store.saveConfig(); } catch (e) { notice.value = String(e); }
}
async function chooseConnection() {
  key.value = ''; models.value = [];
  try { await refreshKey(); await store.saveConfig(); } catch (e) { notice.value = String(e); }
}
async function save() {
  try {
    const extra: unknown = parameters.value.trim() ? JSON.parse(parameters.value) : null;
    if (extra !== null && (typeof extra !== 'object' || Array.isArray(extra))) throw new Error(t('mediaApi.invalidJson'));
    cfg.value.parameters = extra as Record<string, unknown> | null;
    if (key.value.trim() && !cfg.value.connection_id) { await api.setMediaProviderKey(cfg.value.provider!, key.value.trim()); key.value = ''; hasKey.value = true; }
    await store.saveConfig(); notice.value = t('mediaApi.saved'); return true;
  } catch (e) { notice.value = String(e); return false; }
}
async function removeKey() {
  try { await api.clearMediaProviderKey(cfg.value.provider!); hasKey.value = false; notice.value = t('mediaApi.keyRemoved'); } catch (e) { notice.value = String(e); }
}
async function loadModels() {
  busy.value = true;
  try { if (!await save()) return; models.value = await api.listMediaModels(props.kind); notice.value = models.value.length ? t('mediaApi.chooseModel') : t('mediaApi.noModels'); }
  catch (e) { notice.value = String(e); }
  finally { busy.value = false; }
}
defineExpose({ save });
</script>
<template>
  <div v-if="store.config" class="media-api-selector">
    <label>{{ t(kind === 'image' ? 'mediaApi.imageApi' : 'mediaApi.videoApi') }}
      <select class="text-input" v-model="cfg.provider" @change="chooseProvider">
        <option value="">{{ t('mediaApi.compatible') }}</option><option value="compatible">{{ t('mediaApi.customCompatible') }}</option>
        <option value="openrouter">{{ t('mediaApi.openrouter') }}</option>
        <option v-if="kind === 'image'" value="openai">{{ t('mediaApi.openai') }}</option>
        <option value="gemini">{{ t('mediaApi.google') }}</option><option value="xai">xAI · Grok Imagine</option><option value="fal">{{ t('mediaApi.fal') }}</option>
      </select>
    </label>
    <template v-if="vendor">
      <label>{{ t('mediaApi.connection') }}
        <select class="text-input" v-model="cfg.connection_id" @change="chooseConnection">
          <option v-if="cfg.provider !== 'openrouter'" value="">{{ t('mediaApi.sharedKey') }}</option>
          <option v-for="c in available" :key="c.id" :value="c.id">{{ c.label }} · {{ t(c.has_key ? 'mediaApi.configured' : 'mediaApi.noKey') }}</option>
        </select>
      </label>
      <p v-if="cfg.connection_id" class="hint">{{ t(hasKey ? 'mediaApi.reuse' : 'mediaApi.configureExisting') }}</p>
      <template v-else>
        <label>{{ t('mediaApi.apiKey') }}<input class="text-input" type="password" autocomplete="new-password" v-model="key" :placeholder="t(hasKey ? 'mediaApi.keyPresent' : 'mediaApi.providerKey')" /></label>
        <button v-if="hasKey" class="btn-secondary" @click="removeKey">{{ t('mediaApi.removeKey') }}</button>
      </template>
      <label>{{ t('mediaApi.model') }}<input class="text-input" v-model="cfg.model" :placeholder="t(cfg.provider === 'fal' ? 'mediaApi.falEndpoint' : 'mediaApi.modelId')" /></label>
      <div class="actions"><button v-if="cfg.provider !== 'fal'" class="btn-secondary" :disabled="busy" @click="loadModels">{{ t(busy ? 'mediaApi.loading' : 'mediaApi.loadModels') }}</button></div>
      <label v-if="models.length">{{ t('mediaApi.models') }}<select class="text-input" v-model="cfg.model" @change="save"><option value="">{{ t('mediaApi.select') }}</option><option v-for="m in models" :key="m.id" :value="m.id">{{ m.label }}</option></select></label>
      <details><summary>{{ t('mediaApi.parameters') }}</summary><p class="hint">{{ t('mediaApi.parametersHint') }}</p><textarea class="text-input" v-model="parameters" rows="4" placeholder='{"quality":"high"}' /></details>
      <p v-if="cfg.provider === 'fal'" class="hint">{{ t('mediaApi.falHint') }}</p>
      <p v-if="kind === 'video'" class="hint">{{ t('mediaApi.videoHint') }}</p>
      <button class="btn-primary" @click="save">{{ t('mediaApi.save') }}</button>
    </template>
    <p aria-live="polite">{{ notice }}</p>
  </div>
</template>
<style scoped>
label { display:flex; flex-direction:column; gap:6px; margin:12px 0; font-size:13px; font-weight:600; }
.text-input { border:var(--cerne-border); border-radius:8px; padding:8px 10px; font:inherit; font-weight:500; min-width:0; width:100%; box-sizing:border-box; }
.hint { color:#71717a; font-size:13px; line-height:1.5; }
.actions { display:flex; gap:10px; margin:10px 0; }
button { border:var(--cerne-border); border-radius:8px; padding:7px 12px; font:inherit; font-size:13px; font-weight:600; cursor:pointer; }
.btn-secondary { background:white; color:#52525b; }.btn-primary { background:#18181b; color:white; margin-top:12px; }
summary { font-size:13px; cursor:pointer; margin:12px 0; }
</style>
