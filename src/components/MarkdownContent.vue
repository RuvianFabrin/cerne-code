<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { renderMarkdown } from "../markdown";
import { api } from "../api";

const props = defineProps<{ content: string; dark?: boolean }>();

// Durante streaming, chat:token chega token a token (as vezes dezenas de
// vezes por segundo com modelos locais rapidos) e cada delta reatribuia
// `content`, disparando markdown-it + highlight.js + DOMPurify inteiros de
// novo sobre o texto acumulado — trabalho sincrono na thread principal que
// cresce com o tamanho da mensagem e travava a UI (cliques/scroll sem
// resposta) em respostas longas. Aqui o conteudo exibido so e atualizado no
// máximo uma vez por frame (rAF), pegando sempre o valor mais recente.
const displayContent = ref(props.content);
let rafId: number | null = null;
watch(
  () => props.content,
  () => {
    if (rafId !== null) return;
    rafId = requestAnimationFrame(() => {
      displayContent.value = props.content;
      rafId = null;
    });
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  if (rafId !== null) cancelAnimationFrame(rafId);
});

const html = computed(() => renderMarkdown(displayContent.value));
const bodyRef = ref<HTMLElement | null>(null);

// Links de resposta em markdown vao pro navegador padrao do SO em vez de
// navegar a janela do proprio app pra fora - sem isso, clicar num link some
// com a UI inteira do Cerne Code (o WebView navega o app inteiro).
function onClick(e: MouseEvent) {
  const target = e.target as HTMLElement;

  const copyBtn = target.closest(".code-copy-btn");
  if (copyBtn) {
    const pre = copyBtn.closest("pre");
    const code = pre?.querySelector("code")?.textContent ?? pre?.textContent ?? "";
    navigator.clipboard.writeText(code).then(() => {
      const icon = copyBtn.querySelector(".msi");
      if (icon) icon.textContent = "check";
      copyBtn.classList.add("copied");
      setTimeout(() => {
        copyBtn.classList.remove("copied");
        if (icon) icon.textContent = "content_copy";
      }, 1500);
    });
    return;
  }

  const link = target.closest("a");
  if (!link) return;
  const href = link.getAttribute("href");
  if (!href) return;
  e.preventDefault();
  api.openExternalUrl(href);
}

function injectCopyButtons() {
  const el = bodyRef.value;
  if (!el) return;
  el.querySelectorAll("pre").forEach((pre) => {
    if (pre.querySelector(".code-copy-btn")) return;
    const btn = document.createElement("button");
    btn.className = "code-copy-btn";
    btn.title = "Copiar";
    btn.innerHTML = '<span class="msi">content_copy</span>';
    pre.style.position = "relative";
    pre.appendChild(btn);
  });
}

watch(
  html,
  async () => {
    await nextTick();
    injectCopyButtons();
  },
  // Sem isso, o botão nunca aparecia na mensagem final: durante o streaming
  // o texto vem de um <MarkdownContent> "ao vivo" (ChatView.vue liveBlocks),
  // mas ao terminar o turno essa instância é descartada e uma NOVA monta a
  // mensagem persistida (MessageBubble). Nessa instância nova, `html` já
  // nasce com o valor final e nunca "muda" depois - o watcher sem immediate
  // nunca disparava, então injectCopyButtons() nunca rodava.
  { immediate: true },
);
</script>

<template>
  <div ref="bodyRef" class="markdown-body" :class="{ dark: props.dark }" v-html="html" @click="onClick" />
</template>

<style scoped>
.markdown-body {
  font-size: var(--cerne-font-chat, 16px);
  font-weight: 400;
  line-height: 1.55;
  min-width: 0;
  overflow-wrap: anywhere;
  word-break: normal;
}

.markdown-body :deep(p) {
  margin: 0 0 0.75em;
  /* markdown-it já transforma as quebras em <br>; pre-wrap as duplicava. */
  white-space: normal;
}

.markdown-body :deep(> :first-child) {
  margin-top: 0;
}

.markdown-body :deep(> :last-child) {
  margin-bottom: 0;
}

.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4),
.markdown-body :deep(h5),
.markdown-body :deep(h6) {
  margin: 1.25em 0 0.5em;
  font-weight: 700;
  line-height: 1.25;
  text-wrap: pretty;
}

.markdown-body :deep(h1) {
  font-size: 2em;
  letter-spacing: -0.025em;
}
.markdown-body :deep(h2) {
  font-size: 1.5em;
  letter-spacing: -0.015em;
}
.markdown-body :deep(h3) {
  font-size: 1.25em;
}
.markdown-body :deep(h4) {
  font-size: 1.125em;
}
.markdown-body :deep(h5) {
  font-size: 1em;
}
.markdown-body :deep(h6) {
  font-size: 0.9375em;
  color: #52525b;
}

.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  margin: 0 0 0.75em;
  padding-left: 1.5em;
}

.markdown-body :deep(ul) {
  list-style-type: disc;
}
.markdown-body :deep(ul ul) {
  list-style-type: circle;
}
.markdown-body :deep(ul ul ul) {
  list-style-type: square;
}
.markdown-body :deep(ol) {
  list-style-type: decimal;
}

.markdown-body :deep(li) {
  margin: 0;
  padding-left: 0.125em;
}

.markdown-body :deep(li + li) {
  margin-top: 0.25em;
}

.markdown-body :deep(li p) {
  margin: 0 0 0.5em;
}

.markdown-body :deep(li > :last-child) {
  margin-bottom: 0;
}

.markdown-body :deep(li ul),
.markdown-body :deep(li ol) {
  margin: 0.25em 0 0;
}

.markdown-body :deep(a) {
  color: inherit;
  text-decoration: underline;
  text-decoration-color: #a1a1aa;
  text-underline-offset: 2px;
}

.markdown-body :deep(a:hover) {
  text-decoration-color: currentColor;
}

.markdown-body :deep(strong) {
  font-weight: 700;
}

.markdown-body :deep(blockquote) {
  margin: 0 0 0.75em;
  padding: 0.25em 0.75em;
  border-left: 3px solid #d4d4d8;
  color: #52525b;
}

.markdown-body :deep(blockquote > :last-child) {
  margin-bottom: 0;
}

.markdown-body :deep(hr) {
  border: none;
  border-top: 1px solid var(--cerne-markdown-border, #e4e4e7);
  margin: 1em 0;
}

.markdown-body :deep(code) {
  font-family: var(--cerne-mono);
  font-size: 0.9em;
}

.markdown-body :deep(:not(pre) > code) {
  background: #f4f4f5;
  border-radius: 4px;
  padding: 1px 5px;
  border: 1px solid var(--cerne-markdown-border, #e4e4e7);
}

.markdown-body :deep(pre) {
  margin: 0.75em 0;
  padding: 14px 48px 14px 16px;
  background: #fafafa;
  border: 1px solid var(--cerne-markdown-border, #e4e4e7);
  border-radius: 8px;
  overflow-x: auto;
  position: relative;
  max-width: 100%;
  line-height: 1.6;
  white-space: pre;
  overflow-wrap: normal;
  word-break: normal;
  tab-size: 4;
}

.markdown-body :deep(.code-copy-btn) {
  position: absolute;
  top: 6px;
  right: 6px;
  border: none;
  background: rgba(0, 0, 0, 0.05);
  border-radius: 6px;
  padding: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #52525b;
}

.markdown-body :deep(.code-copy-btn:hover) {
  background: rgba(0, 0, 0, 0.1);
}

.markdown-body :deep(.code-copy-btn .msi) {
  font-size: 14px;
}

.markdown-body :deep(.code-copy-btn.copied) {
  color: #16a34a;
}

.markdown-body :deep(pre code) {
  background: none;
  padding: 0;
  color: #27272a;
  white-space: pre;
  overflow-wrap: normal;
  word-break: normal;
}

.markdown-body :deep(table) {
  border-collapse: collapse;
  margin: 0 0 0.75em;
  font-size: 0.93em;
  display: block;
  overflow-x: auto;
  max-width: 100%;
  width: fit-content;
  line-height: 1.5;
  font-variant-numeric: tabular-nums;
  background: #ffffff;
}

.markdown-body :deep(th),
.markdown-body :deep(td) {
  border: 1px solid var(--cerne-markdown-border, #e4e4e7);
  padding: 8px 12px;
  text-align: left;
  word-break: normal;
  overflow-wrap: normal;
  vertical-align: top;
}

.markdown-body :deep(th) {
  background: #f4f4f5;
  font-weight: 600;
}

.markdown-body :deep(tbody tr:nth-child(even)) {
  background: #fafafa;
}

.markdown-body :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: 8px;
}

/* highlight.js token colors — kept minimal, tuned to the app's neutral zinc
   palette instead of pulling in a full prebuilt hljs theme stylesheet. */
.markdown-body :deep(.hljs-keyword),
.markdown-body :deep(.hljs-built_in) {
  color: #7c3aed;
}
.markdown-body :deep(.hljs-string) {
  color: #16a34a;
}
.markdown-body :deep(.hljs-comment) {
  color: #a1a1aa;
  font-style: italic;
}
.markdown-body :deep(.hljs-number),
.markdown-body :deep(.hljs-literal) {
  color: #ea580c;
}
.markdown-body :deep(.hljs-function),
.markdown-body :deep(.hljs-title) {
  color: #2563eb;
}
.markdown-body :deep(.hljs-attr),
.markdown-body :deep(.hljs-attribute) {
  color: #0891b2;
}

/* Dark variant used inside the user's own (dark-background) bubble. */
.markdown-body.dark :deep(a) {
  text-decoration-color: #71717a;
}
.markdown-body.dark :deep(h6) {
  color: #d4d4d8;
}
.markdown-body.dark :deep(blockquote) {
  border-left-color: #52525b;
  color: #d4d4d8;
}
.markdown-body.dark :deep(hr) {
  border-top-color: #3f3f46;
}
.markdown-body.dark :deep(:not(pre) > code) {
  background: #27272a;
  color: #fafafa;
  border-color: #3f3f46;
}
.markdown-body.dark :deep(pre) {
  background: #27272a;
  border-color: #3f3f46;
}
.markdown-body.dark :deep(.code-copy-btn) {
  background: rgba(255, 255, 255, 0.1);
  color: #a1a1aa;
}
.markdown-body.dark :deep(.code-copy-btn:hover) {
  background: rgba(255, 255, 255, 0.2);
}
.markdown-body.dark :deep(pre code) {
  color: #e4e4e7;
}
.markdown-body.dark :deep(th),
.markdown-body.dark :deep(td) {
  border-color: #52525b;
}
.markdown-body.dark :deep(table) {
  background: #18181b;
}
.markdown-body.dark :deep(th) {
  background: #27272a;
}
.markdown-body.dark :deep(tbody tr:nth-child(even)) {
  background: #27272a;
}
</style>
