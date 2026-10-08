// Test-only browser harness: real Vue components, simulated Tauri bridge.
import { createApp, h } from "vue";
import { createPinia } from "pinia";
import PrimeVue from "primevue/config";
import Tooltip from "primevue/tooltip";
import ChatView from "../../src/components/ChatView.vue";
import MediaApiSelector from "../../src/components/MediaApiSelector.vue";
import VideoSettings from "../../src/components/VideoSettings.vue";
import { useSessionStore } from "../../src/stores/session";
import { useProviderStore } from "../../src/stores/provider";
import { i18n } from "../../src/i18n";
import { cerneThemeOptions } from "../../src/theme";
import "../../src/style.css";
const win = window as any;
win.calls = [];
win.dialogResult = null;
win.records = [];
let session: any;
let provider: any;
win.__TAURI_INTERNALS__ = {
  transformCallback: () => 1,
  convertFileSrc: (file: string) => file,
  invoke: async (cmd: string, args: any) => {
    win.calls.push({ cmd, args });
    if (cmd === "set_config" && win.failSave) throw new Error('Simulated save failure');
    if (cmd === "list_media_connections") return [{ id: 'openrouter', provider: 'openrouter', label: 'OpenRouter', has_key: true }, { id: 'custom:existing-google', provider: 'gemini', label: 'Google já salvo', has_key: true }];
    if (cmd === "list_media_models") return [{ id: args.kind + '-available-model', label: args.kind + ' available model' }];
    if (cmd === "has_media_provider_key") return !!win.vendorKeySet?.[args.provider];
    if (cmd === "set_media_provider_key") { (win.vendorKeySet ??= {})[args.provider] = true; return; }
    if (cmd === "clear_media_provider_key") { (win.vendorKeySet ??= {})[args.provider] = false; return; }
    if (cmd === "has_video_gen_key") return !!win.videoKeySet;
    if (cmd === "set_video_gen_key") { win.videoKeySet = true; return; }
    if (cmd === "clear_video_gen_key") { win.videoKeySet = false; return; }
    if (cmd === "plugin:dialog|open") return win.dialogResult;
    if (cmd === "send_media_message") {
      const record = { id: crypto.randomUUID(), kind: args.kind, prompt: args.prompt, model: args.kind + '-model', created_at: new Date().toISOString(), after_text_message: session.messages.length, files: [args.kind === 'video' ? '/scripts/media-tests/fixture.mp4' : '/scripts/media-tests/fixture.svg'], error: null };
      win.records.push(record); return record;
    }
    if (cmd === "get_config") return provider.config;
    if (cmd === "get_session") return session.currentSession;
    if (cmd === "get_session_messages_since") return session.messages;
    if (cmd === "list_session_media") return win.records;
    if (cmd === "get_session_messages_page") return { messages: session.messages, next_before: 0, has_more: false };
    if (cmd === "get_session_context_usage") return null;
    if (cmd === "check_vision_support") return false;
    if (cmd.includes('models')) return [{ id: 'text-model', label: 'Text model', supports_vision: false }];
    return [];
  }
};
const pinia = createPinia();
session = useSessionStore(pinia);
provider = useProviderStore(pinia);
provider.config = { image_gen: { base_url: 'http://fake-image/v1', model: 'image-model' }, video_gen: { base_url: 'http://fake-video/v1', model: 'video-model', protocol: '', seconds: '', size: '', output_dir: '' }, active_provider: 'custom', active_model: 'text-model', long_horizon: {}, external_cli: {} } as any;
provider.models.custom = [{ id: 'text-model', label: 'Text model' }];
session.currentId = '12345678-1234-4234-8234-123456789abc';
session.currentSession = { id: session.currentId, title: 'Teste direto', provider: 'custom', model: 'text-model', project_root: null, extra_read_paths: [], long_horizon: { enabled: false, iteracao_atual: 0, ultimo_desfecho: null }, execution_mode: 'yolo', reasoning_effort: 'off', enabled_mcp_servers: null } as any;
session.messages = [{ role: 'user', content: 'Texto antigo que não deve ir para mídia.' }, { role: 'assistant', content: 'Resposta antiga de texto.' }];
session.messagesOldestIndex = 0;
win.testSession = session;
win.testProvider = provider;
const app = createApp({ render: () => h(ChatView) });
app.use(pinia).use(PrimeVue, { theme: cerneThemeOptions }).use(i18n).directive('tooltip', Tooltip).mount('#app');
win.mountVideoSettings = () => {
  const host = document.createElement('div'); host.id = 'test-video-settings'; document.body.append(host);
  createApp(VideoSettings).use(pinia).use(i18n).mount(host);
};

win.mountImageSettings = () => {
  const host = document.createElement('div'); host.id = 'test-image-settings'; document.body.append(host);
  createApp({ render: () => h(MediaApiSelector, { kind: 'image' }) }).use(pinia).use(i18n).mount(host);
};
