<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { api, type Persona, type PythonTool, type SkillMeta } from "../api";
import { useSessionStore } from "../stores/session";
import SkillEditorModal from "./SkillEditorModal.vue";
import SkillImportModal from "./SkillImportModal.vue";
import PersonaEditorModal from "./PersonaEditorModal.vue";
import PythonToolEditorModal from "./PythonToolEditorModal.vue";

// Fase B1 do roteiro de Agentes/Skills: painel unificado pra ver o que está
// disponível (skills + agentes + personas) e usar rapidamente sem precisar
// ir em Configurações. "Agente nomeado" (Fase A3) já é exatamente o que
// `Persona` faz — nome + system_prompt_override + filtro de tools/skills.
//
// Abas "Agentes" e "Personas" são o MESMO dado (`Persona`) filtrado por
// `kind` — pedido explícito do usuário (2026-08-18): mesmo mecanismo por
// baixo, mas separados na UI porque o USO é diferente — "Agente" é
// orquestrador (passos explícitos: "use a ferramenta X, depois Y, leia o
// arquivo da pasta W e mova pra pasta R"), "Persona" é especialista/professor
// (tom e conhecimento: "Especialista em inglês, ensina pra profissional de
// TI, com muitos anos de experiência"). Achado testando ao vivo antes disso
// (2026-08-17): a primeira versão só tinha uma aba "Agentes" cobrindo os
// dois usos, e o usuário sentiu falta da distinção.
//
// Gap conhecido: o badge "em uso agora" só é possível pra agentes/personas
// (dá pra comparar com `session.persona_id`, que é exatamente essa
// informação). Pra skills não tem sinal equivalente hoje — `load_skill` é
// uma leitura síncrona dentro do turno, não cria um `AgentExecution`
// rastreável (só `task`/`verify_completion` criam, e esses não têm nome de
// skill associado).

const { t } = useI18n();
const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const sessionStore = useSessionStore();

const tab = ref<"skills" | "agents" | "personas" | "pythonTools">("skills");
const search = ref("");

// Skills e Personas agora vivem em `sessionStore` (compartilhado com
// `ComposerBar.vue` — menu `/` e seletor de persona) desde que um bug real
// foi encontrado testando ao vivo (2026-08-16): cada componente tinha sua
// PRÓPRIA cópia local carregada só uma vez, então criar uma persona aqui
// nunca aparecia no composer sem recarregar a sessão inteira. Python tools
// continuam locais — nada mais no app precisa dessa lista hoje.
const pythonTools = ref<PythonTool[]>([]);
const loading = ref(false);

async function refresh() {
  loading.value = true;
  try {
    const projectRoot = sessionStore.currentSession?.project_root ?? null;
    const [, , pythonToolList] = await Promise.all([
      sessionStore.loadSkills(projectRoot),
      sessionStore.loadPersonas(),
      api.listPythonTools(),
    ]);
    pythonTools.value = pythonToolList;
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.visible,
  (v) => {
    if (v) refresh();
  },
);

const filteredSkills = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return sessionStore.skills;
  return sessionStore.skills.filter(
    (s) => s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q),
  );
});

function filterPersonas(kind: "agent" | "persona") {
  const q = search.value.trim().toLowerCase();
  return sessionStore.personas.filter((p) => {
    if (p.kind !== kind) return false;
    if (!q) return true;
    return p.name.toLowerCase().includes(q) || p.content.toLowerCase().includes(q);
  });
}

const filteredAgents = computed(() => filterPersonas("agent"));
const filteredPersonas = computed(() => filterPersonas("persona"));
// Contagem do badge da aba não usa a busca (mesmo padrão de Skills/Ferramentas
// Python, que mostram o total sempre) — só o kind importa aqui.
const agentsCount = computed(() => sessionStore.personas.filter((p) => p.kind === "agent").length);
const personasCount = computed(() => sessionStore.personas.filter((p) => p.kind === "persona").length);

const filteredPythonTools = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return pythonTools.value;
  return pythonTools.value.filter(
    (pt) => pt.name.toLowerCase().includes(q) || pt.description.toLowerCase().includes(q),
  );
});

const skillModalVisible = ref(false);
const skillModalTarget = ref<SkillMeta | null>(null);
function openSkillModal(skill: SkillMeta | null) {
  skillModalTarget.value = skill;
  skillModalVisible.value = true;
}

const skillImportModalVisible = ref(false);

const personaModalVisible = ref(false);
const personaModalTarget = ref<Persona | null>(null);
// Só usado quando `persona` é null (criando do zero) — decide se nasce como
// "agent" ou "persona" dependendo de qual aba/botão abriu o modal.
const personaModalDefaultKind = ref<"agent" | "persona">("persona");
function openPersonaModal(persona: Persona | null, defaultKind: "agent" | "persona" = "persona") {
  personaModalTarget.value = persona;
  personaModalDefaultKind.value = defaultKind;
  personaModalVisible.value = true;
}

const pythonToolModalVisible = ref(false);
const pythonToolModalTarget = ref<PythonTool | null>(null);
function openPythonToolModal(tool: PythonTool) {
  pythonToolModalTarget.value = tool;
  pythonToolModalVisible.value = true;
}

const pythonToolDeleteTarget = ref<PythonTool | null>(null);
async function confirmDeletePythonTool() {
  if (!pythonToolDeleteTarget.value) return;
  await api.deletePythonTool(pythonToolDeleteTarget.value.name);
  pythonToolDeleteTarget.value = null;
  await refresh();
}

// "Usar agora" de uma skill: não dá pra forçar o load_skill diretamente (é
// o LLM que decide carregar dentro do turno) — em vez disso, prepara uma
// mensagem no composer pedindo pra usar aquela skill, pro usuário revisar
// e enviar.
function useSkillNow(skill: SkillMeta) {
  sessionStore.setDraft(t("agentsSkillsPanel.useSkillDraft", { name: skill.name }));
  emit("update:visible", false);
}

// "Usar agora" de uma persona: efeito imediato, sem precisar do LLM — é só
// trocar `Session.persona_id`.
async function usePersonaNow(persona: Persona) {
  await sessionStore.updatePersona(persona.id);
  emit("update:visible", false);
}

// Igual "usar agora" de skill: prepara o pedido no composer (a ferramenta
// python já tem uma skill companheira 'python-tool-<nome>' no catálogo).
function usePythonToolNow(tool: PythonTool) {
  sessionStore.setDraft(t("agentsSkillsPanel.usePythonToolDraft", { name: tool.name }));
  emit("update:visible", false);
}
</script>

<template>
  <Dialog
    :visible="visible"
    @update:visible="(v) => emit('update:visible', v)"
    :header="$t('agentsSkillsPanel.title')"
    modal
    :style="{ width: '620px' }"
  >
    <div class="asp-tabs">
      <button class="asp-tab" :class="{ active: tab === 'skills' }" @click="tab = 'skills'">
        {{ $t("agentsSkillsPanel.skillsTab", { count: sessionStore.skills.length }) }}
      </button>
      <button class="asp-tab" :class="{ active: tab === 'agents' }" @click="tab = 'agents'">
        {{ $t("agentsSkillsPanel.agentsTab", { count: agentsCount }) }}
      </button>
      <button class="asp-tab" :class="{ active: tab === 'personas' }" @click="tab = 'personas'">
        {{ $t("agentsSkillsPanel.personasTab", { count: personasCount }) }}
      </button>
      <button class="asp-tab" :class="{ active: tab === 'pythonTools' }" @click="tab = 'pythonTools'">
        {{ $t("agentsSkillsPanel.pythonToolsTab", { count: pythonTools.length }) }}
      </button>
    </div>

    <input v-model="search" class="asp-search" :placeholder="$t('agentsSkillsPanel.searchPlaceholder')" />

    <div v-if="tab === 'skills'" class="asp-list">
      <div v-for="skill in filteredSkills" :key="skill.dir" class="asp-row">
        <div class="asp-info">
          <span class="asp-name">{{ skill.name }}</span>
          <span class="asp-scope-badge">{{ $t(`agentsSkillsPanel.scope.${skill.scope}`) }}</span>
          <span class="asp-desc">{{ skill.description }}</span>
        </div>
        <div class="asp-actions">
          <button class="btn-secondary" @click="useSkillNow(skill)">{{ $t("agentsSkillsPanel.useNow") }}</button>
          <button class="btn-secondary" @click="openSkillModal(skill)">{{ $t("settings.edit") }}</button>
        </div>
      </div>
      <p v-if="!loading && filteredSkills.length === 0" class="hint">{{ $t("agentsSkillsPanel.noSkills") }}</p>
      <div class="asp-new-row">
        <button class="btn-primary asp-new" @click="openSkillModal(null)">{{ $t("settings.createSkill") }}</button>
        <button class="btn-secondary asp-new" @click="skillImportModalVisible = true">{{ $t("agentsSkillsPanel.importSkill") }}</button>
      </div>
    </div>

    <div v-else-if="tab === 'agents'" class="asp-list">
      <div v-for="agent in filteredAgents" :key="agent.id" class="asp-row">
        <div class="asp-info">
          <span class="asp-name">
            {{ agent.name }}
            <span v-if="sessionStore.currentSession?.persona_id === agent.id" class="asp-active-badge">
              {{ $t("agentsSkillsPanel.activeNow") }}
            </span>
          </span>
          <span class="asp-desc asp-persona-preview">{{ agent.content }}</span>
        </div>
        <div class="asp-actions">
          <button class="btn-secondary" @click="usePersonaNow(agent)">{{ $t("agentsSkillsPanel.useNow") }}</button>
          <button class="btn-secondary" @click="openPersonaModal(agent)">{{ $t("settings.edit") }}</button>
        </div>
      </div>
      <p v-if="!loading && filteredAgents.length === 0" class="hint">{{ $t("agentsSkillsPanel.noAgents") }}</p>
      <button class="btn-primary asp-new" @click="openPersonaModal(null, 'agent')">{{ $t("agentsSkillsPanel.createAgent") }}</button>
    </div>

    <div v-else-if="tab === 'personas'" class="asp-list">
      <div v-for="persona in filteredPersonas" :key="persona.id" class="asp-row">
        <div class="asp-info">
          <span class="asp-name">
            {{ persona.name }}
            <span v-if="sessionStore.currentSession?.persona_id === persona.id" class="asp-active-badge">
              {{ $t("agentsSkillsPanel.activeNow") }}
            </span>
          </span>
          <span class="asp-desc asp-persona-preview">{{ persona.content }}</span>
        </div>
        <div class="asp-actions">
          <button class="btn-secondary" @click="usePersonaNow(persona)">{{ $t("agentsSkillsPanel.useNow") }}</button>
          <button class="btn-secondary" @click="openPersonaModal(persona)">{{ $t("settings.edit") }}</button>
        </div>
      </div>
      <p v-if="!loading && filteredPersonas.length === 0" class="hint">{{ $t("agentsSkillsPanel.noPersonas") }}</p>
      <button class="btn-primary asp-new" @click="openPersonaModal(null, 'persona')">{{ $t("settings.createPersona") }}</button>
    </div>

    <div v-else-if="tab === 'pythonTools'" class="asp-list">
      <div v-for="pt in filteredPythonTools" :key="pt.name" class="asp-row">
        <div class="asp-info">
          <span class="asp-name">{{ pt.name }}</span>
          <span class="asp-desc">{{ pt.description }}</span>
          <span v-if="pt.dependencies.length" class="asp-scope-badge">{{ pt.dependencies.join(", ") }}</span>
        </div>
        <div class="asp-actions">
          <button class="btn-secondary" @click="usePythonToolNow(pt)">{{ $t("agentsSkillsPanel.useNow") }}</button>
          <button class="btn-secondary" @click="openPythonToolModal(pt)">{{ $t("settings.edit") }}</button>
          <button class="btn-secondary" @click="pythonToolDeleteTarget = pt">{{ $t("sidebar.delete") }}</button>
        </div>
      </div>
      <p v-if="!loading && filteredPythonTools.length === 0" class="hint">{{ $t("agentsSkillsPanel.noPythonTools") }}</p>
    </div>

    <SkillEditorModal v-model:visible="skillModalVisible" :skill="skillModalTarget" @saved="refresh" />
    <SkillImportModal v-model:visible="skillImportModalVisible" @saved="refresh" />
    <PersonaEditorModal v-model:visible="personaModalVisible" :persona="personaModalTarget" :default-kind="personaModalDefaultKind" @saved="refresh" />
    <PythonToolEditorModal v-model:visible="pythonToolModalVisible" :tool="pythonToolModalTarget" @saved="refresh" />
  </Dialog>

  <Dialog
    :visible="!!pythonToolDeleteTarget"
    @update:visible="(v) => { if (!v) pythonToolDeleteTarget = null; }"
    modal
    :header="t('agentsSkillsPanel.deletePythonToolConfirmTitle')"
    :style="{ width: 'min(420px, 92vw)' }"
  >
    <p class="delete-confirm-text">{{ t("agentsSkillsPanel.deletePythonToolConfirmBody", { name: pythonToolDeleteTarget?.name ?? "" }) }}</p>
    <template #footer>
      <button class="btn-secondary" @click="pythonToolDeleteTarget = null">{{ t("newSession.cancel") }}</button>
      <button class="btn-danger" @click="confirmDeletePythonTool">{{ t("sidebar.delete") }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.asp-tabs {
  display: flex;
  gap: 6px;
  margin-bottom: 10px;
}

.asp-tab {
  border: var(--cerne-border);
  background: #ffffff;
  color: #52525b;
  border-radius: 8px;
  padding: 6px 12px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.asp-tab.active {
  background: #18181b;
  color: #ffffff;
  border-color: #18181b;
}

.asp-search {
  width: 100%;
  box-sizing: border-box;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-family: inherit;
  outline: none;
  margin-bottom: 10px;
}

.asp-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 420px;
  overflow-y: auto;
}

.asp-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
}

.asp-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.asp-name {
  font-size: 13px;
  font-weight: 600;
  font-family: var(--cerne-mono);
  display: flex;
  align-items: center;
  gap: 6px;
}

.asp-scope-badge {
  display: inline-block;
  font-size: 10px;
  font-weight: 600;
  color: #71717a;
  background: #f4f4f5;
  border-radius: 4px;
  padding: 1px 6px;
  width: fit-content;
}

.asp-active-badge {
  display: inline-block;
  font-size: 10px;
  font-weight: 600;
  color: #16a34a;
  background: #dcfce7;
  border-radius: 4px;
  padding: 1px 6px;
}

.asp-desc {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
}

.asp-persona-preview {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  white-space: pre-line;
}

.asp-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.asp-new {
  margin-top: 4px;
  align-self: flex-start;
}

.asp-new-row {
  display: flex;
  gap: 8px;
}

.hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
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
  padding: 6px 10px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.btn-danger {
  border: none;
  background: #dc2626;
  color: #ffffff;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.btn-danger:hover {
  background: #b91c1c;
}

.delete-confirm-text {
  font-size: 13px;
  color: #3f3f46;
  line-height: 1.5;
}
</style>
