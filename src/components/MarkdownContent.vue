<script setup lang="ts">
import { computed } from "vue";
import { renderMarkdown } from "../markdown";
import { api } from "../api";

const props = defineProps<{ content: string; dark?: boolean }>();

const html = computed(() => renderMarkdown(props.content));

// Links de resposta em markdown vao pro navegador padrao do SO em vez de
// navegar a janela do proprio app pra fora - sem isso, clicar num link some
// com a UI inteira do Cerne Code (o WebView navega o app inteiro).
function onClick(e: MouseEvent) {
  const link = (e.target as HTMLElement).closest("a");
  if (!link) return;
  const href = link.getAttribute("href");
  if (!href) return;
  e.preventDefault();
  api.openExternalUrl(href);
}
</script>

<template>
  <div class="markdown-body" :class="{ dark: props.dark }" v-html="html" @click="onClick" />
</template>

<style scoped>
.markdown-body {
  font-size: 14px;
  font-weight: 400;
  line-height: 1.55;
  word-break: break-word;
}

.markdown-body :deep(p) {
  margin: 0 0 8px;
  white-space: pre-wrap;
}

.markdown-body :deep(p:last-child) {
  margin-bottom: 0;
}

.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4) {
  margin: 12px 0 6px;
  font-weight: 600;
  line-height: 1.3;
}

.markdown-body :deep(h1) {
  font-size: 18px;
}
.markdown-body :deep(h2) {
  font-size: 16px;
}
.markdown-body :deep(h3),
.markdown-body :deep(h4) {
  font-size: 14px;
}

.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  margin: 0 0 8px;
  padding-left: 20px;
}

.markdown-body :deep(li) {
  margin: 2px 0;
}

.markdown-body :deep(a) {
  color: inherit;
  text-decoration: underline;
  text-decoration-color: #a1a1aa;
  text-underline-offset: 2px;
}

.markdown-body :deep(blockquote) {
  margin: 0 0 8px;
  padding: 2px 12px;
  border-left: 3px solid #d4d4d8;
  color: #52525b;
}

.markdown-body :deep(hr) {
  border: none;
  border-top: 1px solid #e4e4e7;
  margin: 10px 0;
}

.markdown-body :deep(code) {
  font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
  font-size: 12.5px;
}

.markdown-body :deep(:not(pre) > code) {
  background: #f4f4f5;
  border-radius: 4px;
  padding: 1px 5px;
}

.markdown-body :deep(pre) {
  margin: 0 0 8px;
  padding: 10px 12px;
  background: #fafafa;
  border: 1px solid #e4e4e7;
  border-radius: 8px;
  overflow-x: auto;
}

.markdown-body :deep(pre code) {
  background: none;
  padding: 0;
  color: #27272a;
}

.markdown-body :deep(table) {
  border-collapse: collapse;
  margin: 0 0 8px;
  font-size: 13px;
  display: block;
  overflow-x: auto;
}

.markdown-body :deep(th),
.markdown-body :deep(td) {
  border: 1px solid #e4e4e7;
  padding: 5px 10px;
  text-align: left;
  word-break: normal;
  overflow-wrap: break-word;
}

.markdown-body :deep(th) {
  background: #f4f4f5;
  font-weight: 500;
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
}
.markdown-body.dark :deep(pre) {
  background: #27272a;
  border-color: #3f3f46;
}
.markdown-body.dark :deep(pre code) {
  color: #e4e4e7;
}
.markdown-body.dark :deep(th),
.markdown-body.dark :deep(td) {
  border-color: #52525b;
}
.markdown-body.dark :deep(th) {
  background: #27272a;
}
</style>
