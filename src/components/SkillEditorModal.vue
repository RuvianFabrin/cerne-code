<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { api, type SkillLanguage, type SkillMeta } from "../api";

const { t } = useI18n();
const props = defineProps<{ visible: boolean; skill: SkillMeta | null }>();
const emit = defineEmits<{ "update:visible": [value: boolean]; saved: [] }>();

const isNew = computed(() => props.skill === null);

const name = ref("");
const description = ref("");
const language = ref<SkillLanguage>("pt-br");
const bodyTemplate = ref("");
const bodyEdited = ref(false);

const content = ref("");
const loading = ref(false);
const error = ref("");

function buildPreview() {
  content.value = `---\nname: ${name.value.trim() || t("skillEditor.namePlaceholder")}\ndescription: ${
    description.value.trim() || t("skillEditor.descriptionPlaceholder")
  }\n---\n${bodyTemplate.value}`;
}

watch([name, description], () => {
  if (isNew.value && !bodyEdited.value) buildPreview();
});

watch(language, async (lang) => {
  if (!isNew.value) return;
  bodyTemplate.value = await api.skillTemplateBody(lang);
  if (!bodyEdited.value) buildPreview();
});

watch(
  () => [props.visible, props.skill?.dir] as const,
  async ([visible]) => {
    if (!visible) return;
    error.value = "";
    if (props.skill) {
      loading.value = true;
      try {
        content.value = await api.readSkill(props.skill.dir);
      } catch (e) {
        error.value = String(e);
      } finally {
        loading.value = false;
      }
    } else {
      name.value = "";
      description.value = "";
      language.value = "pt-br";
      bodyEdited.value = false;
      loading.value = true;
      try {
        bodyTemplate.value = await api.skillTemplateBody("pt-br");
        buildPreview();
      } catch (e) {
        error.value = String(e);
      } finally {
        loading.value = false;
      }
    }
  },
);

async function save() {
  error.value = "";
  try {
    if (isNew.value) {
      if (!name.value.trim() || !description.value.trim()) {
        error.value = t("skillEditor.fillNameAndDescription");
        return;
      }
      const dir = await api.createSkill(name.value.trim(), description.value.trim(), language.value);
      await api.saveSkill(dir, content.value);
    } else if (props.skill) {
      await api.saveSkill(props.skill.dir, content.value);
    }
    emit("saved");
    emit("update:visible", false);
  } catch (e) {
    error.value = String(e);
  }
}

function cancel() {
  emit("update:visible", false);
}
</script>

<template>
  <Dialog
    :visible="props.visible"
    @update:visible="(v) => emit('update:visible', v)"
    :header="isNew ? $t('skillEditor.newSkill') : $t('skillEditor.skillHeader', { name: skill?.name })"
    modal
    :style="{ width: '640px' }"
  >
    <div class="skill-help">
      <p v-html="$t('skillEditor.whatIsASkill')"></p>
      <p v-html="$t('skillEditor.whenToCreateOne')"></p>
    </div>
    <div v-if="isNew" class="skill-new-fields">
      <input v-model="name" class="text-input" :placeholder="$t('skillEditor.namePlaceholder')" />
      <input v-model="description" class="text-input" :placeholder="$t('skillEditor.descriptionPlaceholder')" />
      <select v-model="language" class="text-input skill-lang-select">
        <option value="pt-br">Português</option>
        <option value="en">English</option>
      </select>
    </div>
    <p v-else-if="skill" class="skill-desc-line">{{ skill.description }}</p>
    <p v-if="loading" class="hint">{{ $t("providerPicker.loading") }}</p>
    <textarea v-else v-model="content" @input="bodyEdited = true" class="skill-modal-textarea" rows="14" />
    <p v-if="error" class="error-text">{{ error }}</p>
    <template #footer>
      <button class="btn-secondary" @click="cancel">{{ $t("newSession.cancel") }}</button>
      <button class="btn-primary" :disabled="loading" @click="save">{{ $t("sidebar.save") }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.skill-help {
  background: #f4f4f5;
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.skill-help p {
  margin: 0;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
  line-height: 1.5;
}

.skill-help code {
  font-family: ui-monospace, monospace;
  background: #e4e4e7;
  border-radius: 4px;
  padding: 1px 5px;
}

.skill-new-fields {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}

.text-input {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  outline: none;
  flex: 1;
}

.skill-lang-select {
  flex: 0 0 auto;
  width: auto;
}

.skill-desc-line {
  font-size: 12px;
  font-weight: 600;
  color: #52525b;
  margin: 0 0 8px;
}

.skill-modal-textarea {
  width: 100%;
  box-sizing: border-box;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 12px;
  font-family: ui-monospace, monospace;
  resize: vertical;
  outline: none;
}

.hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
}

.error-text {
  font-size: 12px;
  font-weight: 500;
  color: #dc2626;
  margin-top: 8px;
}

.btn-primary {
  border: none;
  background: #18181b;
  color: #ffffff;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.btn-primary:disabled {
  background: #e4e4e7;
  color: #a1a1aa;
  cursor: default;
}

.btn-secondary {
  border: var(--cerne-border);
  background: #ffffff;
  color: #52525b;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  margin-right: 8px;
}
</style>
