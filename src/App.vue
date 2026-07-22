<script setup lang="ts">
import { onMounted, ref } from "vue";
import Sidebar from "./components/Sidebar.vue";
import ChatView from "./components/ChatView.vue";
import Settings from "./components/Settings.vue";
import NewSessionDialog from "./components/NewSessionDialog.vue";
import HelpModal from "./components/HelpModal.vue";
import { useProviderStore } from "./stores/provider";
import { useSessionStore } from "./stores/session";

const view = ref<"chat" | "settings">("chat");
const showNewSession = ref(false);
const showHelp = ref(false);

const providerStore = useProviderStore();
const sessionStore = useSessionStore();

onMounted(async () => {
  try {
    await providerStore.init();
    await sessionStore.initListeners();
    await sessionStore.loadSessions();
    if (sessionStore.sessions.length > 0) {
      await sessionStore.selectSession(sessionStore.sessions[0].id);
    }
  } catch (e) {
    // Outside a Tauri window (e.g. plain browser preview) the IPC bridge
    // isn't present — the UI still renders, just without live data.
    console.warn("Cerne Code: Tauri bridge unavailable", e);
  }
});
</script>

<template>
  <div class="shell">
    <Sidebar v-model:view="view" @new-session="showNewSession = true" @open-help="showHelp = true" />
    <main class="main-panel">
      <ChatView v-if="view === 'chat'" @open-settings="view = 'settings'" />
      <Settings v-else />
    </main>
    <NewSessionDialog v-model:visible="showNewSession" />
    <HelpModal v-model:visible="showHelp" />
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
