<script setup lang="ts">
import { ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api } from "../api";
import { useSessionStore } from "../stores/session";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import type { MediaRecord } from "../api";
defineProps<{ record: MediaRecord }>();
const { t } = useI18n();
const session = useSessionStore();
const enlarged = ref("");
const fileError = ref("");
async function openFile(path: string) {
  try { if (session.currentId) await api.openGeneratedMedia(session.currentId, path); } catch (e) { fileError.value = String(e); }
}
</script>
<template>
  <section class="media-card">
    <div class="media-prompt"><span class="msi">{{ record.kind === 'image' ? 'image' : 'movie' }}</span>
      <span>{{ t(`media.${record.kind}`) }} · {{ record.model || t('media.defaultModel') }}</span>
      <p>{{ record.prompt }}</p>
    </div>
    <p v-if="record.error" class="error-text">{{ record.error }}</p>
    <div class="media-results">
      <figure v-for="file in record.files" :key="file">
        <button v-if="record.kind === 'image'" class="image-preview" :aria-label="t('media.enlarge')" @click="enlarged = file">
          <img :src="convertFileSrc(file)" :alt="record.prompt" loading="lazy" />
        </button>
        <video v-else :src="convertFileSrc(file)" controls preload="metadata" playsinline @error="fileError = t('media.playbackError')" />
        <figcaption><button class="btn-secondary" @click="openFile(file)">{{ t('media.openFile') }}</button></figcaption>
      </figure>
    </div>
    <p v-if="fileError" class="error-text">{{ fileError }}</p>
    <Dialog :visible="!!enlarged" modal :header="t('media.image')" @update:visible="enlarged = ''" :style="{ maxWidth: '95vw' }">
      <img v-if="enlarged" :src="convertFileSrc(enlarged)" class="full-image" :alt="record.prompt" />
    </Dialog>
  </section>
</template>
<style scoped>
.media-card { margin: 14px 0; padding: 12px; border: 1px solid #dedee7; border-radius: 12px; }
.media-prompt { color: #71717a; font-size: 13px; }
.media-prompt .msi { vertical-align: middle; margin-right: 6px; }
.media-prompt p { white-space: pre-wrap; color: #18181b; font-size: var(--cerne-chat-font-size, 16px); margin: 8px 0; }
.media-results { display: flex; flex-wrap: wrap; gap: 12px; }
figure { margin: 0; }
.image-preview { border: 0; background: transparent; padding: 0; cursor: zoom-in; }
.image-preview img { max-width: 240px; max-height: 220px; object-fit: contain; border-radius: 8px; }
video { width: 480px; max-width: 100%; max-height: 320px; border-radius: 8px; background: #18181b; }
figcaption { margin-top: 5px; }
.full-image { max-width: 85vw; max-height: 80vh; object-fit: contain; }
.error-text { color: #dc2626; white-space: pre-wrap; }
.btn-secondary { border: var(--cerne-border); background: white; color: #52525b; border-radius: 8px; padding: 6px 10px; font-family: inherit; font-size: 12px; font-weight: 600; cursor: pointer; }
</style>
