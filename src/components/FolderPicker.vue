<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "../stores/session";

// T41/T42: dropdown com as pastas JÁ selecionadas na sessão (project_root +
// extra_read_paths) — antes `FileBrowser.vue`/`RepoDiffViewer.vue` iam
// direto pro diálogo nativo do SO, obrigando escolher de novo uma pasta que
// já estava configurada. "Escolher outra pasta..." continua abrindo o
// diálogo nativo pra quem quiser navegar pra algum lugar novo.
const { t } = useI18n();
const props = withDefaults(defineProps<{ current: string; allowBrowse?: boolean }>(), {
  allowBrowse: true,
});
const emit = defineEmits<{ pick: [path: string] }>();

const sessionStore = useSessionStore();

interface FolderOption {
  label: string;
  path: string;
}

const BROWSE_VALUE = "__browse__";

const options = computed<FolderOption[]>(() => {
  const opts: FolderOption[] = [];
  const session = sessionStore.currentSession;
  if (session?.project_root) {
    opts.push({ label: `${t("folderPicker.projectRoot")}: ${session.project_root}`, path: session.project_root });
  }
  for (const entry of session?.extra_read_paths ?? []) {
    if (opts.some((o) => o.path === entry.path)) continue;
    opts.push({ label: entry.path, path: entry.path });
  }
  return opts;
});

async function onChange(value: string) {
  if (value === BROWSE_VALUE) {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") emit("pick", picked);
    return;
  }
  emit("pick", value);
}
</script>

<template>
  <select
    class="folder-picker-select"
    :value="props.current"
    @change="onChange(($event.target as HTMLSelectElement).value)"
  >
    <option v-if="props.current && !options.some((o) => o.path === props.current)" :value="props.current">
      {{ props.current }}
    </option>
    <option v-if="!props.current" value="" disabled>{{ $t("folderPicker.none") }}</option>
    <option v-for="opt in options" :key="opt.path" :value="opt.path">{{ opt.label }}</option>
    <option v-if="allowBrowse" :value="BROWSE_VALUE">{{ $t("folderPicker.browse") }}</option>
  </select>
</template>

<style scoped>
.folder-picker-select {
  flex: 1;
  min-width: 0;
  border: var(--cerne-border);
  border-radius: 6px;
  padding: 6px 8px;
  font-size: 12px;
  font-family: var(--cerne-mono);
  color: #52525b;
  background: #f4f4f5;
}
</style>
