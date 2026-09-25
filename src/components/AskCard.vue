<script setup lang="ts">
import { nextTick, ref } from "vue";
import { useSessionStore } from "../stores/session";
import MarkdownContent from "./MarkdownContent.vue";

const sessionStore = useSessionStore();
const freeText = ref("");
const otherOpen = ref(false);
const otherInput = ref<HTMLInputElement | null>(null);

function answer(option: string) {
  sessionStore.answerQuestion(option);
}

async function openOther() {
  otherOpen.value = true;
  await nextTick();
  otherInput.value?.focus();
}

function submitFreeText() {
  if (!freeText.value.trim()) return;
  sessionStore.answerQuestion(freeText.value.trim());
  freeText.value = "";
  otherOpen.value = false;
}
</script>

<template>
  <div v-if="sessionStore.pendingQuestion" class="ask-card">
    <div class="ask-header">
      <span class="msi">help</span>
      <MarkdownContent :content="sessionStore.pendingQuestion.question" class="ask-question-md" />
    </div>
    <div class="ask-options">
      <button
        v-for="(option, i) in sessionStore.pendingQuestion.options"
        :key="option"
        class="ask-option"
        @click="answer(option)"
      >
        <span class="ask-option-label">{{ option }}</span>
        <span class="ask-option-chip">{{ i + 1 }}</span>
      </button>
      <div class="ask-option ask-option-other" :class="{ 'ask-option-other-open': otherOpen }">
        <template v-if="!otherOpen">
          <button class="ask-option-other-trigger" @click="openOther">
            <span class="ask-option-label">{{ $t("askCard.other") }}</span>
            <span class="msi ask-option-other-icon">edit</span>
          </button>
        </template>
        <template v-else>
          <input
            ref="otherInput"
            v-model="freeText"
            class="ask-other-input"
            :placeholder="$t('askCard.freeTextPlaceholder')"
            @keyup.enter="submitFreeText"
            @keyup.esc="otherOpen = false"
          />
          <button class="ask-other-submit" :disabled="!freeText.trim()" @click="submitFreeText">
            {{ $t("askCard.answer") }}
          </button>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ask-card {
  border: 1px solid #e4e4e7;
  border-radius: 12px;
  overflow: hidden;
  margin-bottom: 10px;
  background: #ffffff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
}

.ask-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-bottom: 1px solid #f4f4f5;
}

.ask-header .msi {
  font-size: 18px;
  color: #71717a;
}

.ask-question-md {
  flex: 1;
  min-width: 0;
}

.ask-question-md :deep(.markdown-body) {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
  line-height: 1.5;
}

.ask-question-md :deep(.markdown-body p) {
  margin: 0;
}

.ask-question-md :deep(.markdown-body p + p) {
  margin-top: 4px;
}

.ask-question-md :deep(.markdown-body code) {
  font-size: 12px;
  background: rgba(0, 0, 0, 0.06);
  padding: 1px 4px;
  border-radius: 3px;
}

.ask-question-md :deep(.markdown-body pre) {
  margin: 6px 0 0;
  font-size: 12px;
}

.ask-question-md :deep(.markdown-body ul),
.ask-question-md :deep(.markdown-body ol) {
  margin: 4px 0 0;
  padding-left: 20px;
}

.ask-question-md :deep(.markdown-body li) {
  margin: 2px 0;
}

.ask-options {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px;
}

.ask-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
  border: 1px solid #e4e4e7;
  background: #fafafa;
  color: #18181b;
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}

.ask-option:hover {
  background: #f4f4f5;
  border-color: #d4d4d8;
}

.ask-option-label {
  flex: 1;
  min-width: 0;
}

.ask-option-chip {
  flex-shrink: 0;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: 5px;
  background: #ffffff;
  border: 1px solid #e4e4e7;
  color: #71717a;
  font-size: 11px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ask-option-other {
  padding: 0;
  cursor: default;
  background: #ffffff;
}

.ask-option-other:not(.ask-option-other-open):hover {
  background: #f4f4f5;
}

.ask-option-other-trigger {
  all: unset;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
  padding: 10px 12px;
  cursor: pointer;
  color: #71717a;
  font-size: 13px;
  font-weight: 500;
}

.ask-option-other-icon {
  font-size: 16px;
}

.ask-option-other-open {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px;
  border-color: #a1a1aa;
}

.ask-other-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  padding: 6px 6px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  color: #18181b;
}

.ask-other-submit {
  flex-shrink: 0;
  border: none;
  background: #18181b;
  color: #ffffff;
  border-radius: 6px;
  padding: 7px 12px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.ask-other-submit:disabled {
  background: #d4d4d8;
  cursor: default;
}
</style>
