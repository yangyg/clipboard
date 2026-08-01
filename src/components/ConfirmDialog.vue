<template>
  <BaseDialog
    :open="!!current"
    role="alertdialog"
    :labelled-by="titleId"
    :described-by="messageId"
    @close="settle(false)"
  >
    <div class="dialog-header">
      <span :id="titleId" class="dialog-title">{{ current?.title }}</span>
    </div>
    <div class="dialog-body">
      <p :id="messageId" class="dialog-message">{{ current?.message }}</p>
    </div>
    <div class="dialog-footer">
      <button class="btn btn-secondary btn-lg" type="button" @click="settle(false)">
        {{ current?.cancelText }}
      </button>
      <button
        class="btn btn-primary btn-lg"
        :class="{ danger: current?.danger }"
        type="button"
        @click="settle(true)"
      >
        {{ current?.confirmText }}
      </button>
    </div>
  </BaseDialog>
</template>

<script setup lang="ts">
import BaseDialog from "./BaseDialog.vue";
import { useConfirm } from "../composables/useConfirm";

const { current, settle } = useConfirm();

const titleId = "confirm-dialog-title";
const messageId = "confirm-dialog-message";
</script>

<style scoped>
.dialog-message {
  margin: 0;
  font-size: var(--text-base);
  line-height: 1.5;
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
