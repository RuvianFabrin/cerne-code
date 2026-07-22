<script setup lang="ts">
import Dialog from "primevue/dialog";
import MarkdownContent from "./MarkdownContent.vue";
import helpContent from "../content/help.md?raw";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();
</script>

<template>
  <Dialog
    :visible="props.visible"
    @update:visible="(v) => emit('update:visible', v)"
    header="Ajuda"
    modal
    :style="{ width: '900px' }"
  >
    <div class="help-body">
      <MarkdownContent :content="helpContent" />
    </div>
  </Dialog>
</template>

<style scoped>
.help-body {
  max-height: 70vh;
  overflow-y: auto;
  padding-right: 4px;
}
</style>
