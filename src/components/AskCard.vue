<script setup lang="ts">
import { ref } from "vue";
import { useSessionStore } from "../stores/session";

const sessionStore = useSessionStore();
const freeText = ref("");

function answer(option: string) {
  sessionStore.answerQuestion(option);
}

function submitFreeText() {
  if (!freeText.value.trim()) return;
  sessionStore.answerQuestion(freeText.value.trim());
  freeText.value = "";
}
</script>

<template>
  <div v-if="sessionStore.pendingQuestion" class="ask-card">
    <div class="ask-header">
      <span class="msi">help</span>
      <span class="ask-question">{{ sessionStore.pendingQuestion.question }}</span>
    </div>
    <div v-if="sessionStore.pendingQuestion.options.length > 0" class="ask-options">
      <button v-for="option in sessionStore.pendingQuestion.options" :key="option" class="ask-option" @click="answer(option)">
        {{ option }}
      </button>
    </div>
    <div class="ask-free-text">
      <input
        v-model="freeText"
        class="text-input"
        placeholder="Ou responda com texto livre..."
        @keyup.enter="submitFreeText"
      />
      <button class="btn-primary" @click="submitFreeText">Responder</button>
    </div>
  </div>
</template>

<style scoped>
.ask-card {
  border: var(--cerne-border);
  border-radius: 12px;
  overflow: hidden;
  margin-bottom: 10px;
  background: #fffbeb;
  border-color: #fde68a;
}

.ask-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
}

.ask-header .msi {
  font-size: 18px;
  color: #b45309;
}

.ask-question {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
}

.ask-options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 0 12px 10px;
}

.ask-option {
  border: 1px solid #fde68a;
  background: #ffffff;
  color: #18181b;
  border-radius: 8px;
  padding: 6px 12px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.ask-option:hover {
  background: #fef3c7;
}

.ask-free-text {
  display: flex;
  gap: 8px;
  padding: 0 12px 12px;
}

.text-input {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  outline: none;
  flex: 1;
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
</style>
