<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Dialog from "primevue/dialog";
import { api, type SkillLanguage, type SkillMeta } from "../api";

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
  content.value = `---\nname: ${name.value.trim() || "nome-da-skill"}\ndescription: ${
    description.value.trim() || "Quando usar (uma linha)"
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
        error.value = "Preencha o nome e o \"quando usar\" antes de salvar.";
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
    :header="isNew ? 'Nova skill' : `Skill: ${skill?.name}`"
    modal
    :style="{ width: '640px' }"
  >
    <div class="skill-help">
      <p>
        <strong>O que é uma skill:</strong> um arquivo <code>SKILL.md</code> com instruções que o
        agente carrega sob demanda (via <code>load_skill</code>) quando percebe que são relevantes
        pro pedido atual — é uma forma de ensinar o agente a seguir um passo a passo ou convenção
        específica, sem precisar reescrever isso toda vez na conversa.
      </p>
      <p>
        <strong>Quando vale criar uma:</strong> quando você se pega explicando a mesma coisa pro
        agente em conversas diferentes (um processo do seu time, um formato de saída específico,
        os passos certos pra revisar um PR, etc.). A <code>description</code> é o que o agente lê
        pra decidir se a skill é relevante — capriche nela dizendo claramente QUANDO usar.
      </p>
    </div>
    <div v-if="isNew" class="skill-new-fields">
      <input v-model="name" class="text-input" placeholder="nome-da-skill" />
      <input v-model="description" class="text-input" placeholder="Quando usar (uma linha)" />
      <select v-model="language" class="text-input skill-lang-select">
        <option value="pt-br">Português</option>
        <option value="en">English</option>
      </select>
    </div>
    <p v-else-if="skill" class="skill-desc-line">{{ skill.description }}</p>
    <p v-if="loading" class="hint">Carregando...</p>
    <textarea v-else v-model="content" @input="bodyEdited = true" class="skill-modal-textarea" rows="14" />
    <p v-if="error" class="error-text">{{ error }}</p>
    <template #footer>
      <button class="btn-secondary" @click="cancel">Cancelar</button>
      <button class="btn-primary" :disabled="loading" @click="save">Salvar</button>
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
