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
      <span id="welcome-title" class="dialog-title">欢迎使用 ClipVault</span>
    </div>
    <div class="dialog-body">
      <ol id="welcome-desc" class="welcome-steps">
        <li>
          用全局快捷键
          <kbd class="kbd">{{ shortcut }}</kbd>
          唤起面板
        </li>
        <li>选一条记录，回车或点粘贴</li>
        <li>托盘图标右键：打开面板 / 设置 / 退出</li>
      </ol>
    </div>
    <div class="dialog-footer">
      <button class="btn-confirm" type="button" @click="emit('complete')">
        开始使用
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
