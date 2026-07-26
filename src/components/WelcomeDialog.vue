<template>
  <BaseDialog
    :open="open"
    role="dialog"
    labelled-by="welcome-title"
    described-by="welcome-desc"
    :close-on-overlay="false"
    @close="emit('complete')"
  >
    <div class="dialog-header">
      <span id="welcome-title" class="dialog-title">{{ $t('welcome.title') }}</span>
    </div>
    <div class="dialog-body">
      <ol id="welcome-desc" class="welcome-steps">
        <li>{{ $t('welcome.step1', { shortcut }) }}</li>
        <li>{{ $t('welcome.step2') }}</li>
        <li>{{ $t('welcome.step3') }}</li>
      </ol>
    </div>
    <div class="dialog-footer">
      <button class="btn-confirm" type="button" @click="emit('complete')">
        {{ $t('welcome.start') }}
      </button>
    </div>
  </BaseDialog>
</template>

<script setup lang="ts">
import BaseDialog from "./BaseDialog.vue";

defineProps<{
  open: boolean;
  shortcut: string;
}>();

const emit = defineEmits<{
  (e: "complete"): void;
}>();
</script>

<style scoped>
.welcome-steps {
  margin: 0;
  padding-left: 1.25rem;
  font-size: var(--text-base);
  line-height: 1.6;
  color: var(--text-secondary);
}

.welcome-steps li + li {
  margin-top: 0.5rem;
}

.kbd {
  display: inline-block;
  margin: 0 0.15em;
  padding: 0.1em 0.4em;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: var(--text-sm, 0.85em);
  color: var(--text-primary);
  background: var(--bg-hover);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm, 6px);
}
</style>
