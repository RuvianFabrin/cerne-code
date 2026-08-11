<script setup lang="ts">
import Dialog from "primevue/dialog";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const credits = [
  { name: "Tauri", desc: "Framework desktop Rust + WebView", url: "https://tauri.app" },
  { name: "Vue.js", desc: "Framework frontend reativo", url: "https://vuejs.org" },
  { name: "PrimeVue", desc: "Componentes UI (Dialog, Select, Popover)", url: "https://primevue.org" },
  { name: "Material Symbols", desc: "Ícones do Google (fonte outlined)", url: "https://fonts.google.com/icons" },
  { name: "Pinia", desc: "Gerenciamento de estado Vue", url: "https://pinia.vuejs.org" },
  { name: "vue-i18n", desc: "Internacionalização (PT-BR, EN, ES, ZH)", url: "https://vue-i18n.intlify.dev" },
  { name: "markdown-it", desc: "Parser Markdown para renderização", url: "https://github.com/markdown-it/markdown-it" },
  { name: "highlight.js", desc: "Syntax highlighting em blocos de código", url: "https://highlightjs.org" },
  { name: "DOMPurify", desc: "Sanitização de HTML contra XSS", url: "https://github.com/cure53/DOMPurify" },
  { name: "rust-xlsxwriter", desc: "Geração de arquivos Excel (.xlsx)", url: "https://github.com/jmcnamara/rust_xlsxwriter" },
  { name: "chardetng", desc: "Detecção de encoding de arquivos", url: "https://github.com/hsivonen/chardetng" },
  { name: "ast-grep", desc: "Busca e reescrita estrutural de código (AST)", url: "https://ast-grep.github.io" },
  { name: "DuckDuckGo / Brave / Mojeek", desc: "Motores de busca web agregados", url: "https://duckduckgo.com" },
  { name: "MetaGPT", desc: "Referência: prompts de PM/Architect/QA", url: "https://github.com/FoundationAgents/MetaGPT" },
  { name: "CrewAI", desc: "Referência: formato declarativo de agentes", url: "https://github.com/crewAIInc/crewAI" },
  { name: "AutoGen", desc: "Referência: termination conditions, GroupChat", url: "https://github.com/microsoft/autogen" },
  { name: "PR-Agent (Qodo)", desc: "Referência: prompt de code review, ticket compliance", url: "https://github.com/qodo-ai/pr-agent" },
  { name: "Open Interpreter", desc: "Referência: AGENTS.md, regras de contexto", url: "https://github.com/openinterpreter/open-interpreter" },
  { name: "Composio", desc: "Referência: formato SKILL.md validado", url: "https://github.com/ComposioHQ/composio" },
];
</script>

<template>
  <Dialog
    :visible="props.visible"
    @update:visible="(v) => emit('update:visible', v)"
    :header="$t('about.title')"
    modal
    :style="{ width: '520px' }"
  >
    <div class="about-body">
      <div class="about-logo">
        <span class="brand-text">Cerne Code</span>
        <span class="version">v0.1.0</span>
      </div>
      <p class="about-desc">{{ $t("about.description") }}</p>

      <h3 class="credits-title">{{ $t("about.creditsTitle") }}</h3>
      <ul class="credits-list">
        <li v-for="c in credits" :key="c.name" class="credit-item">
          <a :href="c.url" target="_blank" rel="noopener" class="credit-name">{{ c.name }}</a>
          <span class="credit-desc">{{ c.desc }}</span>
        </li>
      </ul>
    </div>
  </Dialog>
</template>

<style scoped>
.about-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.about-logo {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.brand-text {
  font-size: 20px;
  font-weight: 700;
  color: #18181b;
}

.version {
  font-size: 12px;
  color: #a1a1aa;
  font-weight: 500;
}

.about-desc {
  font-size: 13px;
  color: #52525b;
  line-height: 1.5;
  margin: 0;
}

.credits-title {
  font-size: 14px;
  font-weight: 600;
  color: #18181b;
  margin: 8px 0 0;
}

.credits-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 320px;
  overflow-y: auto;
}

.credit-item {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 4px 0;
  border-bottom: 1px solid #f4f4f5;
}

.credit-name {
  font-size: 13px;
  font-weight: 600;
  color: #2563eb;
  text-decoration: none;
  white-space: nowrap;
  min-width: 120px;
}

.credit-name:hover {
  text-decoration: underline;
}

.credit-desc {
  font-size: 12px;
  color: #71717a;
}
</style>
