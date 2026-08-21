<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { api, type ChatMessage } from "../api";
import MarkdownContent from "./MarkdownContent.vue";

const props = defineProps<{ message: ChatMessage }>();
const { t } = useI18n();

const isUser = computed(() => props.message.role === "user");
const isTool = computed(() => props.message.role === "tool");
// `display_content` (quando presente) é o que o usuário realmente digitou —
// `content` pode carregar o texto inteiro de um anexo de documento, que só
// precisa ir pro modelo, não pra tela (senão um PDF grande vira um scroll
// gigante na conversa).
const displayText = computed(() => props.message.display_content ?? props.message.content);

// Voz — "ler resposta em voz alta" (TTS via OpenRouter). Escopo reduzido a
// pedido do usuário: um botão por mensagem do assistente (não um botão
// flutuante ao selecionar trecho de texto, que precisaria de lógica de
// posicionamento/seleção mais frágil pra acertar sem poder testar ao vivo).
type ReadState = "idle" | "loading" | "playing" | "error";
const readState = ref<ReadState>("idle");
// Achado testando ao vivo (2026-08-18): erro de TTS caía num estado "error"
// mudo — o ícone virava um "!"/"i" genérico sem dizer O QUE deu errado, nem
// pro usuário nem pra investigar depois. Agora guarda a mensagem real do
// erro e mostra ela no tooltip do próprio ícone.
const readError = ref("");
let currentAudio: HTMLAudioElement | null = null;
// Trava contra tocar duas vezes seguidas — achado testando ao vivo,
// 2026-08-20: em alguns cliques (dev com hot-reload rodando há muitas
// edições, listener duplicado é a suspeita) `readAloud`/`speakSelection`
// disparavam duas vezes, cada um gerando/tocando seu próprio áudio ao
// mesmo tempo — soava como fala duplicada, mesmo o WAV do Voicebox
// (conferido byte a byte) estando correto. Cada chamada carimba um token;
// se outra chamada mais nova começar antes da resposta chegar, a mais
// velha se auto-descarta em vez de tocar por cima.
let playToken = 0;

// Pausa qualquer áudio tocando agora (do botão da mensagem inteira OU do
// popup de seleção abaixo) — os dois compartilham o mesmo `currentAudio`,
// senão dava pra tocar dois ao mesmo tempo clicando um enquanto o outro
// ainda está falando.
function stopCurrentAudio() {
  playToken++;
  currentAudio?.pause();
  currentAudio = null;
  if (readState.value === "playing" || readState.value === "loading") readState.value = "idle";
  if (selectionState.value === "playing" || selectionState.value === "loading") selectionState.value = "idle";
}

async function readAloud() {
  if (readState.value === "playing" || readState.value === "loading") {
    stopCurrentAudio();
    return;
  }
  stopCurrentAudio();
  const myToken = ++playToken;
  readState.value = "loading";
  readError.value = "";
  try {
    const result = await api.ttsSpeak(displayText.value);
    if (myToken !== playToken) return; // outra leitura começou enquanto esperava — descarta esta
    if (!result.play_locally) {
      // Backend Voicebox: ele mesmo já tocou o áudio (autoplay dele) —
      // tocar de novo aqui soaria duplicado/eco. Só reflete que terminou.
      readState.value = "idle";
      return;
    }
    const audio = new Audio(`data:${result.mime};base64,${result.audio_base64}`);
    currentAudio = audio;
    audio.onended = () => {
      readState.value = "idle";
      currentAudio = null;
    };
    audio.onerror = () => {
      readState.value = "error";
      readError.value = t("message.readAloudPlaybackError");
      currentAudio = null;
    };
    readState.value = "playing";
    await audio.play();
  } catch (e) {
    if (myToken !== playToken) return;
    readState.value = "error";
    readError.value = String(e);
  }
}

// Ler só o trecho selecionado — pedido do usuário, 2026-08-19: "selecionar
// um texto, e aparecer um autofalante no final da seleção pra aprender
// inglês". `selectionchange` é global (dispara mesmo com o mouse fora do
// componente), então cada instância de MessageBubble confere se a seleção
// atual está dentro do SEU próprio texto (`contentEl`) antes de mostrar o
// próprio popup — evita 20 popups piscando ao mesmo tempo numa conversa
// longa.
const contentEl = ref<HTMLElement | null>(null);
const selectionPopup = ref<{ x: number; y: number; text: string } | null>(null);
const selectionState = ref<ReadState>("idle");
const selectionError = ref("");

function onSelectionChange() {
  const sel = window.getSelection();
  const text = sel?.toString().trim() ?? "";
  if (!sel || sel.isCollapsed || !text || !contentEl.value || !sel.anchorNode || !contentEl.value.contains(sel.anchorNode)) {
    selectionPopup.value = null;
    return;
  }
  const rects = sel.getRangeAt(0).getClientRects();
  if (rects.length === 0) {
    selectionPopup.value = null;
    return;
  }
  const last = rects[rects.length - 1];
  selectionPopup.value = { x: last.right, y: last.top, text };
  selectionState.value = "idle";
}

onMounted(() => document.addEventListener("selectionchange", onSelectionChange));
onUnmounted(() => document.removeEventListener("selectionchange", onSelectionChange));

async function speakSelection() {
  if (!selectionPopup.value) return;
  const text = selectionPopup.value.text;
  if (selectionState.value === "playing" || selectionState.value === "loading") {
    stopCurrentAudio();
    return;
  }
  stopCurrentAudio();
  const myToken = ++playToken;
  selectionState.value = "loading";
  selectionError.value = "";
  try {
    const result = await api.ttsSpeak(text);
    if (myToken !== playToken) return; // outra leitura começou enquanto esperava — descarta esta
    if (!result.play_locally) {
      selectionState.value = "idle";
      return;
    }
    const audio = new Audio(`data:${result.mime};base64,${result.audio_base64}`);
    currentAudio = audio;
    audio.onended = () => {
      selectionState.value = "idle";
      currentAudio = null;
    };
    audio.onerror = () => {
      selectionState.value = "error";
      selectionError.value = t("message.readAloudPlaybackError");
      currentAudio = null;
    };
    selectionState.value = "playing";
    await audio.play();
  } catch (e) {
    if (myToken !== playToken) return;
    selectionState.value = "error";
    selectionError.value = String(e);
  }
}

// Mensagem longa (ex: colar um log de erro inteiro) tomava a tela inteira
// de altura — mesmo padrão de "mostrar mais N linhas" já usado no IN/OUT de
// `TaskStepGroup.vue`, aplicado aqui SÓ pra mensagens que o usuário mandou
// (colar um log gigante). A resposta do LLM deve continuar sempre inteira,
// sem truncar (correção pedida ao vivo, 2026-08-16).
const PREVIEW_LINES = 3;
const expanded = ref(false);
const lines = computed(() => displayText.value.split("\n"));
const hasMore = computed(() => isUser.value && lines.value.length > PREVIEW_LINES);
const shownText = computed(() =>
  !hasMore.value || expanded.value ? displayText.value : lines.value.slice(0, PREVIEW_LINES).join("\n"),
);
</script>

<template>
  <div v-if="!isTool" class="row" :class="{ user: isUser }">
    <div class="bubble" :class="{ user: isUser }">
      <div v-if="message.images?.length" class="image-row">
        <img v-for="(src, i) in message.images" :key="i" :src="src" class="message-image" />
      </div>
      <div ref="contentEl">
        <MarkdownContent :content="shownText" :dark="isUser" />
      </div>
      <button v-if="hasMore" class="show-more-btn" :class="{ dark: isUser }" @click="expanded = !expanded">
        <span class="msi">{{ expanded ? "expand_less" : "expand_more" }}</span>
        {{ expanded ? t("taskStep.showLess") : t("taskStep.showMoreLines", { count: lines.length - PREVIEW_LINES }) }}
      </button>
      <button
        v-if="!isUser"
        class="read-aloud-btn"
        :class="{ active: readState === 'playing' || readState === 'loading' }"
        v-tooltip.top="readState === 'error' ? readError : $t(readState === 'playing' ? 'message.stopReading' : 'message.readAloud')"
        @click="readAloud"
      >
        <span class="msi">{{ readState === "loading" ? "hourglass_top" : readState === "playing" ? "stop_circle" : readState === "error" ? "error" : "volume_up" }}</span>
      </button>
    </div>
  </div>

  <Teleport to="body">
    <button
      v-if="selectionPopup"
      class="selection-speak-btn"
      :class="{ active: selectionState === 'playing' || selectionState === 'loading' }"
      :style="{ left: `${selectionPopup.x + 4}px`, top: `${selectionPopup.y - 6}px` }"
      v-tooltip.top="selectionState === 'error' ? selectionError : $t(selectionState === 'playing' ? 'message.stopReading' : 'message.readSelection')"
      @mousedown.prevent
      @click="speakSelection"
    >
      <span class="msi">{{ selectionState === "loading" ? "hourglass_top" : selectionState === "playing" ? "stop_circle" : selectionState === "error" ? "error" : "volume_up" }}</span>
    </button>
  </Teleport>
</template>

<style scoped>
.row {
  display: flex;
  padding: 4px 0;
}

.row.user {
  justify-content: flex-end;
}

.bubble {
  max-width: 72ch;
  padding: 4px 0;
}

.bubble.user {
  background: #18181b;
  color: #fafafa;
  padding: 10px 14px;
  border-radius: 12px;
}

.image-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 8px;
}

.message-image {
  max-width: 220px;
  max-height: 220px;
  border-radius: 8px;
  object-fit: cover;
}

.show-more-btn {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-top: 2px;
  padding: 0;
  background: none;
  border: none;
  font-size: 11px;
  font-weight: 600;
  color: #6366f1;
  cursor: pointer;
}

.show-more-btn.dark {
  color: #a5b4fc;
}

.show-more-btn:hover {
  text-decoration: underline;
}

.show-more-btn .msi {
  font-size: 15px;
}

.read-aloud-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  margin-top: 4px;
  padding: 3px;
  background: none;
  border: none;
  border-radius: 4px;
  color: #a1a1aa;
  cursor: pointer;
}

.read-aloud-btn:hover {
  background: #f4f4f5;
  color: #52525b;
}

.read-aloud-btn.active {
  color: #6366f1;
}

.read-aloud-btn .msi {
  font-size: 17px;
}

.selection-speak-btn {
  position: fixed;
  z-index: 500;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  background: #18181b;
  color: #fafafa;
  border: none;
  border-radius: 50%;
  cursor: pointer;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  transform: translateY(-100%);
}

.selection-speak-btn:hover {
  background: #27272a;
}

.selection-speak-btn.active {
  background: #6366f1;
}

.selection-speak-btn .msi {
  font-size: 15px;
}
</style>
