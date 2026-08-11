<script setup lang="ts">
import { onMounted, ref } from "vue";
import Sidebar from "./components/Sidebar.vue";
import ChatView from "./components/ChatView.vue";
import Settings from "./components/Settings.vue";
import HelpModal from "./components/HelpModal.vue";
import AboutModal from "./components/AboutModal.vue";
import DisclaimerModal from "./components/DisclaimerModal.vue";
import { useI18n } from "vue-i18n";
import { useProviderStore } from "./stores/provider";
import { useSessionStore } from "./stores/session";
import { api } from "./api";

const { t } = useI18n();
const view = ref<"chat" | "settings">("chat");
const showHelp = ref(false);
const showAbout = ref(false);
const showDisclaimer = ref(false);

const providerStore = useProviderStore();
const sessionStore = useSessionStore();

async function createNewSession() {
  const cfg = providerStore.config;
  if (!cfg) return;
  const provider = cfg.active_provider;
  const model = cfg.active_model ?? "";
  const forkId = provider === "llama_cpp" ? (cfg.active_llama_fork ?? null) : null;
  const customId = provider === "custom" ? (cfg.active_custom_provider_id ?? null) : null;
  // Título default traduzido — o backend reconhece esses mesmos textos pra
  // auto-nomear a sessão na primeira mensagem (agent::run_turn).
  await sessionStore.createSession(t("newSession.defaultTitle"), provider, model, null, forkId, customId);
  view.value = "chat";
}

onMounted(async () => {
  try {
    await providerStore.init();
    await sessionStore.initListeners();
    await sessionStore.loadSessions();
    if (sessionStore.sessions.length > 0) {
      await sessionStore.selectSession(sessionStore.sessions[0].id);
    }
    // Disclaimer na primeira abertura
    const accepted = await api.getDisclaimerAccepted().catch(() => false);
    if (!accepted) {
      showDisclaimer.value = true;
    }
  } catch (e) {
    console.warn("Cerne Code: Tauri bridge unavailable", e);
  }
});

async function acceptDisclaimer() {
  await api.setDisclaimerAccepted(true).catch(() => {});
  showDisclaimer.value = false;
}
</script>

<template>
  <div class="shell">
    <Sidebar v-model:view="view" @new-session="createNewSession" @open-help="showHelp = true" @open-about="showAbout = true" />
    <main class="main-panel">
      <ChatView v-if="view === 'chat'" @open-settings="view = 'settings'" />
      <Settings v-else />
    </main>
    <HelpModal v-model:visible="showHelp" />
    <AboutModal v-model:visible="showAbout" />
    <DisclaimerModal v-model:visible="showDisclaimer" @accepted="acceptDisclaimer" />
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  height: 100vh;
  width: 100vw;
  background: #ffffff;
  overflow: hidden;
}

.main-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
</style>
