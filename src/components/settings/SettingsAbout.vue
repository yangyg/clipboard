<template>
  <div class="settings-section">
    <div class="about-content">
      <div class="about-logo">
        <img :src="appIconUrl" alt="Clipboard" width="48" height="48" draggable="false" />
      </div>
      <div class="about-name">Clipboard</div>
      <div class="about-version">{{ $t('settings.about.version') }}</div>
      <div class="about-desc">{{ $t('settings.about.desc') }}</div>
      <button class="about-link" type="button" @click="openRepo">
        <Github class="about-link-icon" :size="14" />
        <span>{{ $t('settings.about.repoLink') }}</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { Github } from "lucide-vue-next";
import appIconUrl from "../../assets/app-icon-128.png";

const REPO_URL = "https://github.com/yangyg/clipboard";

async function openRepo() {
  try {
    await invoke("open_url", { url: REPO_URL });
  } catch (e) {
    console.error("Failed to open repo link", e);
  }
}
</script>

<style scoped>
.about-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 20px;
}

.about-logo {
  display: flex;
  justify-content: center;
  margin-bottom: 4px;
}

.about-logo img {
  width: 48px;
  height: 48px;
  border-radius: var(--radius-md);
  object-fit: contain;
  user-select: none;
}

.about-name {
  font-size: 1.375rem;
  font-weight: 600;
}

.about-version {
  font-size: var(--text-md);
  color: var(--accent-text);
  font-family: var(--font-mono);
}

.about-desc {
  font-size: var(--text-md);
  color: var(--text-tertiary);
  text-align: center;
}

.about-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  padding: 6px 14px;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  color: var(--accent-text);
  font-size: var(--text-md);
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast);
}

.about-link:hover {
  background: var(--bg-hover);
  border-color: var(--accent);
}

.about-link-icon {
  color: var(--accent);
}
</style>
