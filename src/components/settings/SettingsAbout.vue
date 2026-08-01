<template>
  <div class="settings-section">
    <div class="about-content">
      <div class="about-logo">
        <img :src="appIconUrl" alt="Clipboard" width="48" height="48" draggable="false" />
      </div>
      <div class="about-name">Clipboard</div>
      <div class="about-version">{{ $t('settings.about.version') }}</div>
      <div class="about-desc">{{ $t('settings.about.desc') }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import appIconUrl from "../../assets/app-icon-128.png";
</script>

<style scoped>
.about-content {
  text-align: center;
  padding: 20px;
}

.about-logo {
  display: flex;
  justify-content: center;
  margin-bottom: 12px;
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
  margin-bottom: 4px;
}

.about-version {
  font-size: var(--text-md);
  color: var(--accent-text);
  font-family: var(--font-mono);
  margin-bottom: 8px;
}

.about-desc {
  font-size: var(--text-md);
  color: var(--text-tertiary);
}
</style>
