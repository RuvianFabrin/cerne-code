<script setup lang="ts">
import { computed } from "vue";
import type { ChatMessage } from "../api";
import MarkdownContent from "./MarkdownContent.vue";

const props = defineProps<{ message: ChatMessage }>();

const isUser = computed(() => props.message.role === "user");
const isTool = computed(() => props.message.role === "tool");
// `display_content` (quando presente) é o que o usuário realmente digitou —
// `content` pode carregar o texto inteiro de um anexo de documento, que só
// precisa ir pro modelo, não pra tela (senão um PDF grande vira um scroll
// gigante na conversa).
const displayText = computed(() => props.message.display_content ?? props.message.content);
</script>

<template>
  <div v-if="!isTool" class="row" :class="{ user: isUser }">
    <div class="bubble" :class="{ user: isUser }">
      <div v-if="message.images?.length" class="image-row">
        <img v-for="(src, i) in message.images" :key="i" :src="src" class="message-image" />
      </div>
      <MarkdownContent :content="displayText" :dark="isUser" />
    </div>
  </div>
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
</style>
