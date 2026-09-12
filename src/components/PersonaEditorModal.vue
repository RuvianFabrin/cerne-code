<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { api, type Persona, type PersonaKind, type SkillMeta } from "../api";
import { useSessionStore } from "../stores/session";

const { t } = useI18n();
// `defaultKind` só é usado quando `persona` é null (criando do zero) — decide
// se o registro nasce como "agent" ou "persona" dependendo de qual aba do
// painel Agentes & Skills abriu o modal. Editando um existente, o kind vem
// sempre de `persona.kind` (não muda o "tipo" de um registro já criado).
const props = defineProps<{ visible: boolean; persona: Persona | null; defaultKind?: PersonaKind }>();
const emit = defineEmits<{ "update:visible": [value: boolean]; saved: [] }>();
const sessionStore = useSessionStore();

const isNew = computed(() => props.persona === null);

const name = ref("");
const content = ref("");
const kind = ref<PersonaKind>("persona");
const error = ref("");

// Fase A3: allowlist de ferramentas opcional — lista vazia = sem restrição
// (comportamento default, igual antes desse campo existir). Curada pra um
// subconjunto que faz sentido restringir por persona (fora computer_use_*/
// MCP, dinâmicos demais pra um checkbox fixo).
const SELECTABLE_TOOLS = [
  "read_file",
  "write_file",
  "edit_file",
  "ast_edit",
  "ast_grep",
  "list_dir",
  "grep",
  "run_command",
  "web_search",
  "web_fetch",
  "load_skill",
  "task",
  "verify_completion",
  "todo_list",
];
const selectedTools = ref<string[]>([]);

function toggleTool(name: string) {
  const idx = selectedTools.value.indexOf(name);
  if (idx === -1) selectedTools.value.push(name);
  else selectedTools.value.splice(idx, 1);
}

// B3/Fase A3: allowlist de skills — mesmo padrão de tools acima, mas a
// lista de opções vem do catálogo de skills de verdade (global + do
// projeto da sessão atual), não de uma lista curada fixa como as tools.
const availableSkills = ref<SkillMeta[]>([]);
const selectedSkills = ref<string[]>([]);

function toggleSkill(name: string) {
  const idx = selectedSkills.value.indexOf(name);
  if (idx === -1) selectedSkills.value.push(name);
  else selectedSkills.value.splice(idx, 1);
}

watch(
  () => [props.visible, props.persona?.id] as const,
  async ([visible]) => {
    if (!visible) return;
    error.value = "";
    name.value = props.persona?.name ?? "";
    content.value = props.persona?.content ?? "";
    kind.value = props.persona?.kind ?? props.defaultKind ?? "persona";
    selectedTools.value = [...(props.persona?.tools ?? [])];
    selectedSkills.value = [...(props.persona?.skills ?? [])];
    const projectRoot = sessionStore.currentSession?.project_root ?? null;
    availableSkills.value = await api.listSkills(projectRoot).catch(() => []);
  },
);

async function save() {
  error.value = "";
  if (!name.value.trim() || !content.value.trim()) {
    error.value = t("personaEditor.fillNameAndContent");
    return;
  }
  try {
    if (isNew.value) {
      await api.createPersona(name.value.trim(), content.value.trim(), selectedTools.value, selectedSkills.value, kind.value);
    } else if (props.persona) {
      await api.updatePersona(props.persona.id, name.value.trim(), content.value.trim(), selectedTools.value, selectedSkills.value, kind.value);
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
    :header="isNew ? $t(kind === 'agent' ? 'personaEditor.newAgent' : 'personaEditor.newPersona') : $t('personaEditor.personaHeader', { name: persona?.name })"
    modal
    :style="{ width: '640px' }"
  >
    <div class="persona-help">
      <p>{{ $t(kind === "agent" ? "personaEditor.whatIsAnAgent" : "personaEditor.whatIsAPersona") }}</p>
    </div>
    <input v-model="name" class="text-input persona-name-input" :placeholder="$t('personaEditor.namePlaceholder')" />
    <textarea
      v-model="content"
      class="persona-modal-textarea"
      rows="10"
      :placeholder="$t(kind === 'agent' ? 'personaEditor.agentContentPlaceholder' : 'personaEditor.contentPlaceholder')"
    />
    <div class="persona-tools-section">
      <p class="persona-tools-hint">{{ $t("personaEditor.toolsHint") }}</p>
      <div class="persona-tools-grid">
        <label v-for="tool in SELECTABLE_TOOLS" :key="tool" class="persona-tool-checkbox">
          <input
            type="checkbox"
            :checked="selectedTools.includes(tool)"
            @change="toggleTool(tool)"
          />
          {{ $t(`toolLabels.${tool}`) }}
        </label>
      </div>
    </div>
    <div class="persona-tools-section">
      <p class="persona-tools-hint">{{ $t("personaEditor.skillsHint") }}</p>
      <div v-if="availableSkills.length === 0" class="persona-skills-empty">
        {{ $t("personaEditor.noSkillsAvailable") }}
      </div>
      <div v-else class="persona-tools-grid">
        <label v-for="skill in availableSkills" :key="skill.dir" class="persona-tool-checkbox">
          <input
            type="checkbox"
            :checked="selectedSkills.includes(skill.name)"
            @change="toggleSkill(skill.name)"
          />
          {{ skill.name }}
        </label>
      </div>
    </div>
    <p v-if="error" class="error-text">{{ error }}</p>
    <template #footer>
      <button class="btn-secondary" @click="cancel">{{ $t("newSession.cancel") }}</button>
      <button class="btn-primary" @click="save">{{ $t("sidebar.save") }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.persona-help {
  background: #f4f4f5;
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 12px;
}

.persona-help p {
  margin: 0;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
  line-height: 1.5;
}

.text-input {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  outline: none;
  width: 100%;
  box-sizing: border-box;
}

.persona-name-input {
  margin-bottom: 10px;
}

.persona-modal-textarea {
  width: 100%;
  box-sizing: border-box;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 12px;
  font-family: var(--cerne-mono);
  resize: vertical;
  outline: none;
}

.error-text {
  font-size: 12px;
  font-weight: 500;
  color: #dc2626;
  margin-top: 8px;
}

.persona-tools-section {
  margin-top: 12px;
  padding-top: 12px;
  border-top: var(--cerne-border);
}

.persona-tools-hint {
  margin: 0 0 8px;
  font-size: 11px;
  font-weight: 500;
  color: #71717a;
}

.persona-tools-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px 10px;
}

.persona-tool-checkbox {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
  cursor: pointer;
}

.persona-skills-empty {
  font-size: 12px;
  font-weight: 500;
  color: #a1a1aa;
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
