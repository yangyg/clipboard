<template>
  <div class="settings-overlay" tabindex="-1" @keydown.esc="onOverlayEsc">
    <div class="settings-window panel-surface">
      <!-- Header -->
      <div class="settings-header" :class="{ 'with-chrome': isWindowMode }" data-tauri-drag-region>
        <span class="settings-title"><AppIcon name="settings" :size="15" /> {{ $t('settings.title') }}</span>
        <div v-if="isWindowMode" class="settings-header-right">
          <WindowControls />
        </div>
      </div>

      <div class="settings-main">
        <!-- Nav -->
        <nav class="settings-nav">
          <button type="button" class="nav-item nav-back" :title="$t('settings.back')" :aria-label="$t('settings.back')" @click="emit('close')">
            <span class="nav-icon"><AppIcon name="back" :size="15" /></span>
            <span class="nav-label">{{ $t('settings.back') }}</span>
          </button>
          <div class="nav-divider" aria-hidden="true"></div>
          <button
            v-for="section in SECTIONS"
            :key="section.key"
            type="button"
            class="nav-item"
            :class="{ active: activeSection === section.key }"
            :title="$t(section.labelKey)"
            :aria-label="$t(section.labelKey)"
            @click="activeSection = section.key"
          >
            <span class="nav-icon"><AppIcon :name="section.icon" :size="15" /></span>
            <span class="nav-label">{{ $t(section.labelKey) }}</span>
          </button>
        </nav>

        <!-- Body -->
        <div class="settings-body">
          <!-- Shortcuts -->
          <template v-if="activeSection === 'shortcuts'">
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.shortcuts.title') }}</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.shortcuts.globalShortcut') }}</div>
                  <div class="setting-desc">{{ $t('settings.shortcuts.globalShortcutDesc') }}</div>
                </div>
                <button
                  class="shortcut-btn"
                  :class="{ recording: isRecordingShortcut }"
                  type="button"
                  @click="startShortcutRecording"
                  @keydown="onShortcutKeydown"
                >
                  {{ isRecordingShortcut ? $t('settings.shortcuts.pressShortcut') : settings.global_shortcut }}
                </button>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.shortcuts.searchFocus') }}</div>
                  <div class="setting-desc">{{ $t('settings.shortcuts.searchFocusDesc') }}</div>
                </div>
                <span class="kbd-display">/ {{ $t('common.or') }} Ctrl+K</span>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.shortcuts.plainPaste') }}</div>
                  <div class="setting-desc">{{ $t('settings.shortcuts.plainPasteDesc') }}</div>
                </div>
                <span class="kbd-display">Alt + V</span>
              </div>
            </div>
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.shortcuts.behavior') }}</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.shortcuts.autoHide') }}</div>
                  <div class="setting-desc">{{ $t('settings.shortcuts.autoHideDesc') }}</div>
                </div>
                <div
                  class="toggle"
                  :class="{ on: settings.auto_close_on_paste }"
                  role="switch"
                  :aria-checked="settings.auto_close_on_paste"
                  tabindex="0"
                  @click="update('auto_close_on_paste', !settings.auto_close_on_paste)"
                  @keydown.enter.prevent="update('auto_close_on_paste', !settings.auto_close_on_paste)"
                  @keydown.space.prevent="update('auto_close_on_paste', !settings.auto_close_on_paste)"
                ></div>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.shortcuts.defaultPasteMode') }}</div>
                  <div class="setting-desc">{{ $t('settings.shortcuts.defaultPasteModeDesc') }}</div>
                </div>
                <div class="segmented">
                  <button
                    v-for="mode in PASTE_MODES"
                    :key="mode.key"
                    type="button"
                    class="segment-btn"
                    :class="{ selected: settings.default_paste_mode === mode.key }"
                    @click="update('default_paste_mode', mode.key)"
                  >
                    {{ $t(mode.labelKey) }}
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- Appearance -->
          <template v-else-if="activeSection === 'appearance'">
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.appearance.theme') }}</div>
              <div class="theme-cards" role="radiogroup" :aria-label="$t('settings.appearance.theme')">
                <div
                  v-for="(t, idx) in THEMES"
                  :key="t.key"
                  class="theme-card"
                  role="radio"
                  :data-theme="t.key"
                  :aria-checked="settings.theme === t.key"
                  :aria-label="$t(t.labelKey)"
                  :tabindex="settings.theme === t.key ? 0 : -1"
                  :class="{ selected: settings.theme === t.key }"
                  @click="update('theme', t.key)"
                  @keydown.enter.prevent="update('theme', t.key)"
                  @keydown.space.prevent="update('theme', t.key)"
                  @keydown.arrowright.prevent="focusTheme(idx + 1)"
                  @keydown.arrowleft.prevent="focusTheme(idx - 1)"
                  @keydown.arrowdown.prevent="focusTheme(idx + 1)"
                  @keydown.arrowup.prevent="focusTheme(idx - 1)"
                >
                  <div class="theme-preview" :class="`theme-${t.key}`" aria-hidden="true"></div>
                  <div class="theme-name"><AppIcon :name="t.icon" :size="13" /> {{ $t(t.labelKey) }}</div>
                </div>
              </div>
            </div>
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.appearance.appMode') }}</div>
              <div class="mode-grid" role="radiogroup" :aria-label="$t('settings.appearance.appMode')">
                <button
                  v-for="mode in APP_MODES"
                  :key="mode.key"
                  type="button"
                  class="mode-card"
                  role="radio"
                  :aria-checked="settings.app_mode === mode.key"
                  :class="{ selected: settings.app_mode === mode.key }"
                  @click="update('app_mode', mode.key)"
                >
                  <span class="mode-icon"><AppIcon :name="mode.icon" :size="18" /></span>
                  <span class="mode-title">{{ $t(mode.labelKey) }}</span>
                  <span class="mode-desc">{{ $t(mode.descKey) }}</span>
                </button>
              </div>
            </div>
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.appearance.panelAppearance') }}</div>
              <div class="setting-row">
                <div class="setting-label">{{ $t('settings.appearance.cornerRadius') }}</div>
                <div class="slider-row">
                  <input type="range" min="0" max="40" :aria-label="$t('settings.appearance.cornerRadius')" :aria-valuetext="`${settings.panel_radius}px`" :value="settings.panel_radius" @input="(e) => update('panel_radius', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.panel_radius }}px</span>
                </div>
              </div>
              <div class="setting-row">
                  <div class="setting-label">{{ $t('settings.appearance.opacity') }}</div>
                <div class="slider-row">
                  <input type="range" min="60" max="100" :aria-label="$t('settings.appearance.opacity')" :aria-valuetext="`${settings.panel_opacity}%`" :value="settings.panel_opacity" @input="(e) => update('panel_opacity', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.panel_opacity }}%</span>
                </div>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.appearance.blur') }}</div>
                  <div class="setting-desc">{{ $t('settings.appearance.blurDesc') }}</div>
                </div>
                <div
                  class="toggle"
                  :class="{ on: settings.enable_blur }"
                  role="switch"
                  :aria-checked="settings.enable_blur"
                  tabindex="0"
                  @click="update('enable_blur', !settings.enable_blur)"
                  @keydown.enter.prevent="update('enable_blur', !settings.enable_blur)"
                  @keydown.space.prevent="update('enable_blur', !settings.enable_blur)"
                ></div>
              </div>
              <div class="setting-row">
                <div class="setting-label">{{ $t('settings.appearance.animation') }}</div>
                <div
                  class="toggle"
                  :class="{ on: settings.enable_animation }"
                  role="switch"
                  :aria-checked="settings.enable_animation"
                  tabindex="0"
                  @click="update('enable_animation', !settings.enable_animation)"
                  @keydown.enter.prevent="update('enable_animation', !settings.enable_animation)"
                  @keydown.space.prevent="update('enable_animation', !settings.enable_animation)"
                ></div>
              </div>
              <div class="setting-row">
                <div class="setting-label">{{ $t('settings.appearance.fontSize') }}</div>
                <div class="slider-row">
                  <input type="range" min="11" max="18" :aria-label="$t('settings.appearance.fontSize')" :aria-valuetext="`${settings.font_size}px`" :value="settings.font_size" @input="(e) => update('font_size', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.font_size }}px</span>
                </div>
              </div>
            </div>
          </template>

          <!-- History -->
          <template v-else-if="activeSection === 'history'">
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.history.title') }}</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.history.maxRecords') }}</div>
                  <div class="setting-desc">{{ $t('settings.history.maxRecordsDesc') }}</div>
                </div>
                <div class="slider-row">
                  <input type="range" min="100" max="10000" step="100" :aria-label="$t('settings.history.maxRecords')" :value="settings.max_records" @input="(e) => update('max_records', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.max_records }}</span>
                </div>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.history.retentionDays') }}</div>
                  <div class="setting-desc">{{ $t('settings.history.retentionDaysDesc') }}</div>
                </div>
                <div class="slider-row">
                  <input type="range" min="7" max="365" step="1" :aria-label="$t('settings.history.retentionDays')" :value="settings.retention_days" @input="(e) => update('retention_days', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.retention_days }} {{ $t('common.days') }}</span>
                </div>
              </div>
            </div>
          </template>

          <!-- Tags -->
          <template v-else-if="activeSection === 'tags'">
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.tags.title') }}</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.tags.autoTag') }}</div>
                  <div class="setting-desc">{{ $t('settings.tags.autoTagDesc') }}</div>
                </div>
                <div
                  class="toggle"
                  :class="{ on: settings.enable_auto_tag }"
                  role="switch"
                  :aria-checked="settings.enable_auto_tag"
                  tabindex="0"
                  @click="update('enable_auto_tag', !settings.enable_auto_tag)"
                  @keydown.enter.prevent="update('enable_auto_tag', !settings.enable_auto_tag)"
                  @keydown.space.prevent="update('enable_auto_tag', !settings.enable_auto_tag)"
                ></div>
              </div>

              <div v-if="settings.enable_auto_tag" class="auto-tag-panel">
                <div class="auto-tag-panel-head">
                  <div class="auto-tag-panel-title">{{ $t('settings.tags.matchRules') }}</div>
                  <div class="auto-tag-panel-meta">{{ $t('settings.tags.rulesCount', { count: rulesDraft.length }) }}</div>
                </div>

                <div v-if="rulesDraft.length === 0" class="auto-tag-empty">
                  <AppIcon name="tag" :size="18" />
                  <p>{{ $t('settings.tags.noRules') }}</p>
                </div>

                <div v-else class="auto-tag-rules">
                  <article
                    v-for="(rule, index) in rulesDraft"
                    :key="index"
                    class="auto-tag-rule"
                  >
                    <header class="auto-tag-rule-top">
                      <span
                        class="auto-tag-rule-dot"
                        :style="{ background: ruleAccentColor(rule.tag_name, index) }"
                        aria-hidden="true"
                      ></span>
                      <span class="auto-tag-rule-index">{{ $t('settings.tags.rule', { index: index + 1 }) }}</span>
                      <button
                        type="button"
                        class="auto-tag-remove"
                        :title="$t('settings.tags.deleteRule')"
                        :aria-label="$t('settings.tags.deleteRule')"
                        @click="removeAutoTagRule(index)"
                      >
                        <AppIcon name="close" :size="12" />
                      </button>
                    </header>

                    <label class="auto-tag-field">
                      <span class="auto-tag-field-label">{{ $t('settings.tags.tagName') }}</span>
                      <input
                        class="auto-tag-input"
                        :value="rule.tag_name"
                        :placeholder="$t('settings.tags.tagNamePlaceholder')"
                        @input="updateRuleField(index, 'tag_name', (($event.target as HTMLInputElement).value))"
                      />
                    </label>

                    <label class="auto-tag-field">
                      <span class="auto-tag-field-label">{{ $t('settings.tags.keywords') }}</span>
                      <input
                        class="auto-tag-input auto-tag-input-mono"
                        :value="rule.keywords.join(', ')"
                        :placeholder="$t('settings.tags.keywordsPlaceholder')"
                        @change="updateRuleKeywords(index, ($event.target as HTMLInputElement).value)"
                      />
                      <div v-if="rule.keywords.length" class="auto-tag-keyword-chips" aria-hidden="true">
                        <span
                          v-for="kw in rule.keywords"
                          :key="kw"
                          class="auto-tag-chip auto-tag-chip-kw"
                        >{{ kw }}</span>
                      </div>
                    </label>

                    <div class="auto-tag-field">
                      <span class="auto-tag-field-label">{{ $t('settings.tags.contentTypes') }}</span>
                      <div class="auto-tag-type-chips" role="group" :aria-label="$t('settings.tags.contentTypes')">
                        <button
                          v-for="ct in CONTENT_TYPE_OPTIONS"
                          :key="ct.value"
                          type="button"
                          class="auto-tag-type-chip"
                          :class="{ active: rule.content_types.includes(ct.value) }"
                          :style="rule.content_types.includes(ct.value) ? { '--chip-accent': ct.color } : undefined"
                          :aria-pressed="rule.content_types.includes(ct.value)"
                          @click="toggleRuleContentType(index, ct.value)"
                        >
                          <AppIcon :name="ct.icon" :size="12" />
                          {{ $t(ct.labelKey) }}
                        </button>
                      </div>
                    </div>
                  </article>
                </div>

                <div class="auto-tag-actions">
                  <button type="button" class="btn btn-secondary" @click="addAutoTagRule">
                    <AppIcon name="plus" :size="13" /> {{ $t('settings.tags.addRule') }}
                  </button>
                  <button type="button" class="btn btn-secondary" @click="restoreDefaultAutoTagRules">
                    <AppIcon name="restore" :size="13" /> {{ $t('settings.tags.restoreDefaults') }}
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- Privacy -->
          <template v-else-if="activeSection === 'privacy'">
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.privacy.sensitiveTitle') }}</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.privacy.autoDetect') }}</div>
                  <div class="setting-desc">{{ $t('settings.privacy.autoDetectDesc') }}</div>
                </div>
                <div
                  class="toggle"
                  :class="{ on: settings.enable_sensitive_detection }"
                  role="switch"
                  :aria-checked="settings.enable_sensitive_detection"
                  tabindex="0"
                  @click="update('enable_sensitive_detection', !settings.enable_sensitive_detection)"
                  @keydown.enter.prevent="update('enable_sensitive_detection', !settings.enable_sensitive_detection)"
                  @keydown.space.prevent="update('enable_sensitive_detection', !settings.enable_sensitive_detection)"
                ></div>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.privacy.autoExpire') }}</div>
                  <div class="setting-desc">{{ $t('settings.privacy.autoExpireDesc') }}</div>
                </div>
                <div class="slider-row">
                  <input type="range" min="10" max="3600" step="10" :aria-label="$t('settings.privacy.autoExpire')" :aria-valuetext="$t('settings.privacy.autoExpireUnit', { minutes: Math.floor(settings.sensitive_auto_expire_seconds / 60) })" :value="settings.sensitive_auto_expire_seconds" @input="(e) => update('sensitive_auto_expire_seconds', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ $t('settings.privacy.autoExpireUnit', { minutes: Math.floor(settings.sensitive_auto_expire_seconds / 60) }) }}</span>
                </div>
              </div>
            </div>
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.privacy.ignoreTitle') }}</div>
              <div class="ignore-list">
                <div v-for="app in settings.ignored_apps" :key="app" class="ignore-item">
                  <span class="ignore-icon"><AppIcon name="monitor" :size="14" /></span>
                  <span class="ignore-name">{{ app }}</span>
                  <button type="button" class="ignore-remove" :aria-label="$t('settings.privacy.removeApp', { app })" @click="removeIgnoredApp(app)"><AppIcon name="close" :size="12" /></button>
                </div>
              </div>
              <div class="ignore-add-row">
                <input class="ignore-input" :aria-label="$t('settings.privacy.ignoreTitle')" :placeholder="$t('settings.privacy.ignorePlaceholder')" v-model="newIgnoredApp" @keydown.enter="addIgnoredApp" />
                <button type="button" class="ignore-add-btn" @click="addIgnoredApp"><AppIcon name="plus" :size="13" /> {{ $t('settings.privacy.ignoreAdd') }}</button>
              </div>
            </div>
          </template>

          <!-- Data -->
          <template v-else-if="activeSection === 'stats'">
            <div class="stats-dashboard">
              <div class="stats-card">
                <div class="stats-value accent">{{ stats?.total_records ?? 0 }}</div>
                <div class="stats-label">{{ $t('settings.stats.totalRecords') }}</div>
              </div>
              <div class="stats-card">
                <div class="stats-value success">{{ stats?.total_copies ?? 0 }}</div>
                <div class="stats-label">{{ $t('settings.stats.totalCopies') }}</div>
              </div>
              <div class="stats-card">
                <div class="stats-value warning">{{ stats?.favorites_count ?? 0 }}</div>
                <div class="stats-label">{{ $t('settings.stats.favorites') }}</div>
              </div>
              <div class="stats-card">
                <div class="stats-value sensitive">{{ stats?.sensitive_count ?? 0 }}</div>
                <div class="stats-label">{{ $t('settings.stats.sensitive') }}</div>
              </div>
            </div>

            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.stats.typeDistribution') }}</div>
              <div class="type-bars">
                <div v-for="item in typeDistribution" :key="item.key" class="type-row">
                  <div class="type-row-label">
                    <span>{{ item.label }}</span>
                    <span>{{ item.count }}</span>
                  </div>
                  <div class="type-track">
                    <div class="type-fill" :style="{ width: item.percent + '%' }"></div>
                  </div>
                </div>
              </div>
            </div>

            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.stats.storage') }}</div>
              <div class="data-card storage-card">
                <div class="storage-card-main">
                  <div class="setting-label">{{ $t('settings.stats.localStorage') }}</div>
                  <div class="setting-desc">
                    {{ $t('settings.stats.storageDesc') }}
                  </div>
                  <div
                    v-if="stats?.data_path"
                    class="storage-path"
                    :title="stats.data_path"
                  >
                    {{ stats.data_path }}
                  </div>
                </div>
                <span class="kbd-display">{{ formatBytes(stats?.storage_bytes ?? 0) }}</span>
              </div>
            </div>
          </template>

          <!-- Data -->
          <template v-else-if="activeSection === 'data'">
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.data.title') }}</div>
              <div class="data-card">
                <div>
                  <div class="setting-label">{{ $t('settings.data.exportTitle') }}</div>
                  <div class="setting-desc">{{ $t('settings.data.exportDesc') }}</div>
                </div>
                <button class="btn btn-secondary" :disabled="isExporting" @click="exportData">
                  <AppIcon v-if="!isExporting" name="package" :size="13" />
                  {{ isExporting ? $t('settings.data.exporting') : $t('settings.data.exportBtn') }}
                </button>
              </div>
              <div v-if="exportStatus" class="status-line" :class="exportStatusKind">{{ exportStatus }}</div>

              <div class="data-card">
                <div>
                  <div class="setting-label">{{ $t('settings.data.importTitle') }}</div>
                  <div class="setting-desc">{{ $t('settings.data.importDesc') }}</div>
                </div>
                <button class="btn btn-secondary" :disabled="isImporting" @click="importData">
                  <AppIcon v-if="!isImporting" name="history" :size="13" />
                  {{ isImporting ? $t('settings.data.importing') : $t('settings.data.importBtn') }}
                </button>
              </div>
              <div v-if="importStatus" class="status-line" :class="importStatusKind">{{ importStatus }}</div>

              <div class="settings-section-title" style="margin-top: 1.25rem">{{ $t('settings.data.webdavTitle') }}</div>
              <p class="setting-desc" style="margin: 0 0 0.75rem">
                {{ $t('settings.data.webdavDesc') }}
              </p>
              <label class="webdav-field">
                <span class="setting-label">{{ $t('settings.data.webdavUrl') }}</span>
                <input
                  class="auto-tag-input"
                  type="url"
                  placeholder="https://dav.jianguoyun.com/dav/"
                  :value="settings.webdav_url"
                  @input="update('webdav_url', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <label class="webdav-field">
                <span class="setting-label">{{ $t('settings.data.webdavUsername') }}</span>
                <input
                  class="auto-tag-input"
                  type="text"
                  autocomplete="username"
                  :value="settings.webdav_username"
                  @input="update('webdav_username', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <label class="webdav-field">
                <span class="setting-label">{{ $t('settings.data.webdavPassword') }}</span>
                <input
                  class="auto-tag-input"
                  type="password"
                  autocomplete="current-password"
                  :value="settings.webdav_password"
                  @input="update('webdav_password', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <label class="webdav-field">
                <span class="setting-label">{{ $t('settings.data.webdavRemotePath') }}</span>
                <input
                  class="auto-tag-input"
                  type="text"
                  placeholder="ClipVaultSync"
                  :value="settings.webdav_remote_path"
                  @input="update('webdav_remote_path', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.data.webdavSyncSensitive') }}</div>
                  <div class="setting-desc">{{ $t('settings.data.webdavSyncSensitiveDesc') }}</div>
                </div>
                <div
                  class="toggle"
                  :class="{ on: settings.webdav_sync_sensitive }"
                  role="switch"
                  :aria-checked="settings.webdav_sync_sensitive"
                  tabindex="0"
                  @click="update('webdav_sync_sensitive', !settings.webdav_sync_sensitive)"
                  @keydown.enter.prevent="update('webdav_sync_sensitive', !settings.webdav_sync_sensitive)"
                  @keydown.space.prevent="update('webdav_sync_sensitive', !settings.webdav_sync_sensitive)"
                >
                  <div class="toggle-knob" />
                </div>
              </div>
              <div class="data-card webdav-actions">
                <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavTest">
                  <AppIcon name="cloud" :size="13" />
                  {{ webdavAction === 'test' ? $t('settings.data.webdavTesting') : $t('settings.data.webdavTest') }}
                </button>
                <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavPull">
                  <AppIcon name="cloudDownload" :size="13" />
                  {{ webdavAction === 'pull' ? $t('settings.data.webdavPulling') : $t('settings.data.webdavPull') }}
                </button>
                <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavPush">
                  <AppIcon name="cloudUpload" :size="13" />
                  {{ webdavAction === 'push' ? $t('settings.data.webdavPushing') : $t('settings.data.webdavPush') }}
                </button>
                <button class="btn btn-primary" :disabled="webdavBusy" @click="webdavSyncNow">
                  <AppIcon name="refresh" :size="13" />
                  {{ webdavAction === 'sync' ? $t('settings.data.webdavSyncing') : $t('settings.data.webdavSync') }}
                </button>
              </div>
              <div v-if="settings.webdav_last_sync_at" class="setting-desc">
                {{ $t('settings.data.lastSync', { time: formatSyncTime(settings.webdav_last_sync_at) }) }}
              </div>
              <div v-if="webdavStatus" class="status-line" :class="webdavStatusKind">{{ webdavStatus }}</div>

              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.data.clearHistory') }}</div>
                  <div class="setting-desc">{{ $t('settings.data.clearHistoryDesc') }}</div>
                </div>
                <button class="btn btn-danger" @click="clearHistory"><AppIcon name="trash" :size="13" /> {{ $t('settings.data.clearHistoryBtn') }}</button>
              </div>
            </div>
          </template>

          <!-- System -->
          <template v-else-if="activeSection === 'system'">
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.system.title') }}</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.system.autoStart') }}</div>
                  <div class="setting-desc">{{ $t('settings.system.autoStartDesc') }}</div>
                </div>
                <div
                  class="toggle"
                  :class="{ on: settings.auto_start }"
                  role="switch"
                  :aria-checked="settings.auto_start"
                  tabindex="0"
                  @click="update('auto_start', !settings.auto_start)"
                  @keydown.enter.prevent="update('auto_start', !settings.auto_start)"
                  @keydown.space.prevent="update('auto_start', !settings.auto_start)"
                ></div>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.system.minimizeToTray') }}</div>
                  <div class="setting-desc">{{ $t('settings.system.minimizeToTrayDesc') }}</div>
                </div>
                <div
                  class="toggle"
                  :class="{ on: settings.minimize_to_tray }"
                  role="switch"
                  :aria-checked="settings.minimize_to_tray"
                  tabindex="0"
                  @click="update('minimize_to_tray', !settings.minimize_to_tray)"
                  @keydown.enter.prevent="update('minimize_to_tray', !settings.minimize_to_tray)"
                  @keydown.space.prevent="update('minimize_to_tray', !settings.minimize_to_tray)"
                ></div>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">{{ $t('settings.system.language') }}</div>
                  <div class="setting-desc">{{ $t('settings.system.languageDesc') }}</div>
                </div>
                <div class="segmented">
                  <button
                    type="button"
                    class="segment-btn"
                    :class="{ selected: settings.language === 'zh-CN' }"
                    @click="updateLanguage('zh-CN')"
                  >
                    {{ $t('settings.system.langZhCN') }}
                  </button>
                  <button
                    type="button"
                    class="segment-btn"
                    :class="{ selected: settings.language === 'en-US' }"
                    @click="updateLanguage('en-US')"
                  >
                    {{ $t('settings.system.langEnUS') }}
                  </button>
                  <button
                    type="button"
                    class="segment-btn"
                    :class="{ selected: settings.language === 'system' }"
                    @click="updateLanguage('system')"
                  >
                    {{ $t('settings.system.langSystem') }}
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- Help -->
          <template v-else-if="activeSection === 'help'">
            <div class="settings-section">
              <div class="settings-section-title">{{ $t('settings.help.title') }}</div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="keyboard" :size="14" /> {{ $t('settings.help.invokePanel') }}</div>
                <div class="guide-text">{{ $t('settings.help.invokePanelText', { shortcut: settings.global_shortcut }) }}</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="clipboard" :size="14" /> {{ $t('settings.help.autoRecord') }}</div>
                <div class="guide-text">{{ $t('settings.help.autoRecordText') }}</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="paste" :size="14" /> {{ $t('settings.help.pasteToApp') }}</div>
                <div class="guide-text">{{ $t('settings.help.pasteToAppText') }}</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="search" :size="14" /> {{ $t('settings.help.searchFilter') }}</div>
                <div class="guide-text">{{ $t('settings.help.searchFilterText') }}</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="star" :size="14" /> {{ $t('settings.help.favoritePinTag') }}</div>
                <div class="guide-text">{{ $t('settings.help.favoritePinTagText') }}</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="shield" :size="14" /> {{ $t('settings.help.privacyProtection') }}</div>
                <div class="guide-text">{{ $t('settings.help.privacyProtectionText') }}</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="trash" :size="14" /> {{ $t('settings.help.trashCleanup') }}</div>
                <div class="guide-text">{{ $t('settings.help.trashCleanupText') }}</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="panel" :size="14" /> {{ $t('settings.help.appModes') }}</div>
                <div class="guide-text">{{ $t('settings.help.appModesText') }}</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="stats" :size="14" /> {{ $t('settings.help.dataUsage') }}</div>
                <div class="guide-text">{{ $t('settings.help.dataUsageText') }}</div>
              </div>
            </div>
          </template>

          <!-- About -->
          <template v-else-if="activeSection === 'about'">
            <div class="settings-section">
              <div class="about-content">
                <div class="about-logo">
                  <img :src="appIconUrl" alt="ClipVault" width="48" height="48" draggable="false" />
                </div>
                <div class="about-name">ClipVault</div>
                <div class="about-version">{{ $t('settings.about.version') }}</div>
                <div class="about-desc">{{ $t('settings.about.desc') }}</div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../stores/settings";
import { useClipboardStore } from "../stores/clipboard";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { Settings, WebDavSyncResult } from "../types";
import { DEFAULT_AUTO_TAG_RULES, type AutoTagRule } from "../types";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";
import WindowControls from "./WindowControls.vue";
import appIconUrl from "../assets/app-icon-128.png";
import { resolveKnownTagColors, resolveTagPalette } from "../utils/themeColors";
import { setLocale, resolveLocale } from "../locales";

const emit = defineEmits<{ close: [] }>();
const props = defineProps<{
  initialSection?: string;
}>();
const settingsStore = useSettingsStore();
const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { t } = useI18n();
const settings = settingsStore.settings;
const isWindowMode = computed(() => settings.app_mode === "window");
const stats = computed(() => clipboardStore.stats);

const activeSection = ref(props.initialSection ?? "appearance");
const newIgnoredApp = ref("");
const exportStatus = ref("");
const exportStatusKind = ref<"success" | "error" | "">("");
const importStatus = ref("");
const importStatusKind = ref<"success" | "error" | "">("");
const isExporting = ref(false);
const isImporting = ref(false);
const isRecordingShortcut = ref(false);
const webdavBusy = ref(false);
const webdavAction = ref<"" | "test" | "pull" | "push" | "sync">("");
const webdavStatus = ref("");
const webdavStatusKind = ref<"success" | "error" | "">("");

const CONTENT_TYPE_OPTIONS = [
  { value: "text", labelKey: "settings.tags.typeText", icon: "type" as AppIconName, color: "var(--type-text)" },
  { value: "code", labelKey: "settings.tags.typeCode", icon: "code" as AppIconName, color: "var(--type-code)" },
  { value: "link", labelKey: "settings.tags.typeLink", icon: "link" as AppIconName, color: "var(--type-link)" },
  { value: "image", labelKey: "settings.tags.typeImage", icon: "image" as AppIconName, color: "var(--type-image)" },
  { value: "file", labelKey: "settings.tags.typeFile", icon: "file" as AppIconName, color: "var(--type-file)" },
] as const;

const SECTIONS: { key: string; icon: AppIconName; labelKey: string }[] = [
  { key: "appearance", icon: "palette", labelKey: "settings.nav.appearance" },
  { key: "shortcuts", icon: "keyboard", labelKey: "settings.nav.shortcuts" },
  { key: "history", icon: "history", labelKey: "settings.nav.history" },
  { key: "tags", icon: "tag", labelKey: "settings.nav.tags" },
  { key: "privacy", icon: "shield", labelKey: "settings.nav.privacy" },
  { key: "system", icon: "settings", labelKey: "settings.nav.system" },
  { key: "data", icon: "package", labelKey: "settings.nav.data" },
  { key: "stats", icon: "stats", labelKey: "settings.nav.stats" },
  { key: "help", icon: "help", labelKey: "settings.nav.help" },
  { key: "about", icon: "info", labelKey: "settings.nav.about" },
];

const THEMES: { key: Settings["theme"]; icon: AppIconName; labelKey: string }[] = [
  { key: "dark", icon: "moon", labelKey: "settings.appearance.themeDark" },
  { key: "light", icon: "sun", labelKey: "settings.appearance.themeLight" },
  { key: "oled", icon: "circle", labelKey: "settings.appearance.themeOled" },
  { key: "system", icon: "monitor", labelKey: "settings.appearance.themeSystem" },
];

function focusTheme(index: number) {
  const len = THEMES.length;
  const next = ((index % len) + len) % len;
  const key = THEMES[next].key;
  update("theme", key);
  requestAnimationFrame(() => {
    const el = document.querySelector<HTMLElement>(`.theme-card[data-theme="${key}"]`);
    el?.focus();
  });
}

const APP_MODES = [
  {
    key: "floating",
    icon: "panel" as AppIconName,
    labelKey: "settings.appearance.modeFloating",
    descKey: "settings.appearance.modeFloatingDesc",
  },
  {
    key: "window",
    icon: "window" as AppIconName,
    labelKey: "settings.appearance.modeWindow",
    descKey: "settings.appearance.modeWindowDesc",
  },
] as const;

const PASTE_MODES = [
  { key: "original", labelKey: "settings.shortcuts.pasteOriginal" },
  { key: "plain", labelKey: "settings.shortcuts.pastePlain" },
] as const;

const KEY_ALIASES: Record<string, string> = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Escape: "Esc",
};

function update<K extends keyof Settings>(key: K, value: Settings[K]) {
  settingsStore.updateSetting(key, value);
}

function updateLanguage(lang: 'zh-CN' | 'en-US' | 'system') {
  update('language', lang);
  setLocale(resolveLocale(lang));
}

function cloneRules(rules: AutoTagRule[]): AutoTagRule[] {
  return rules.map((r) => ({
    tag_name: r.tag_name,
    keywords: [...r.keywords],
    content_types: [...r.content_types],
  }));
}

/** Local draft so typing rules doesn't deep-watch/save settings on every keystroke. */
const rulesDraft = ref<AutoTagRule[]>(cloneRules(settings.auto_tag_rules));
let rulesCommitTimer: ReturnType<typeof setTimeout> | null = null;
let ignoreRulesSettingsEcho = false;

watch(
  () => settings.auto_tag_rules,
  (rules) => {
    if (ignoreRulesSettingsEcho) return;
    rulesDraft.value = cloneRules(rules);
  },
  { deep: true },
);

function flushAutoTagRules() {
  if (rulesCommitTimer) {
    clearTimeout(rulesCommitTimer);
    rulesCommitTimer = null;
  }
  ignoreRulesSettingsEcho = true;
  update("auto_tag_rules", cloneRules(rulesDraft.value));
  void nextTick(() => {
    ignoreRulesSettingsEcho = false;
  });
}

function scheduleCommitRules() {
  if (rulesCommitTimer) clearTimeout(rulesCommitTimer);
  rulesCommitTimer = setTimeout(() => {
    rulesCommitTimer = null;
    flushAutoTagRules();
  }, 400);
}

function updateRuleField(index: number, field: "tag_name", value: string) {
  const next = cloneRules(rulesDraft.value);
  if (!next[index]) return;
  next[index][field] = value;
  rulesDraft.value = next;
  scheduleCommitRules();
}

function updateRuleKeywords(index: number, raw: string) {
  const next = cloneRules(rulesDraft.value);
  if (!next[index]) return;
  next[index].keywords = raw
    .split(/[,，]/)
    .map((s) => s.trim())
    .filter(Boolean);
  rulesDraft.value = next;
  scheduleCommitRules();
}

function toggleRuleContentType(index: number, contentType: string) {
  const next = cloneRules(rulesDraft.value);
  if (!next[index]) return;
  const types = next[index].content_types;
  const i = types.indexOf(contentType);
  if (i >= 0) types.splice(i, 1);
  else types.push(contentType);
  rulesDraft.value = next;
  flushAutoTagRules();
}

function addAutoTagRule() {
  const next = cloneRules(rulesDraft.value);
  next.push({ tag_name: "", keywords: [], content_types: [] });
  rulesDraft.value = next;
  flushAutoTagRules();
}

function removeAutoTagRule(index: number) {
  const next = cloneRules(rulesDraft.value);
  next.splice(index, 1);
  rulesDraft.value = next;
  flushAutoTagRules();
}

function restoreDefaultAutoTagRules() {
  rulesDraft.value = cloneRules(DEFAULT_AUTO_TAG_RULES);
  flushAutoTagRules();
}

function ruleAccentColor(tagName: string, index: number): string {
  const name = tagName.trim();
  const known = resolveKnownTagColors();
  if (name && known[name]) return known[name];
  const fromStore = clipboardStore.tags.find((t) => t.name === name)?.color;
  if (fromStore) return fromStore;
  const palette = resolveTagPalette();
  return palette[index % palette.length] ?? cssFallbackAccent();
}

function cssFallbackAccent(): string {
  return resolveTagPalette()[0] ?? "#0078d4";
}

function addIgnoredApp() {
  const name = newIgnoredApp.value.trim();
  if (!name) {
    toast(t('settings.privacy.ignoreEmpty'), "warning");
    return;
  }
  if (settings.ignored_apps.includes(name)) {
    toast(t('settings.privacy.ignoreDuplicate'), "warning");
    return;
  }
  const updated = [...settings.ignored_apps, name];
  settingsStore.updateSetting("ignored_apps", updated);
  newIgnoredApp.value = "";
}

function removeIgnoredApp(app: string) {
  const updated = settings.ignored_apps.filter((a) => a !== app);
  settingsStore.updateSetting("ignored_apps", updated);
}

function startShortcutRecording() {
  isRecordingShortcut.value = true;
}

function stopShortcutRecording() {
  isRecordingShortcut.value = false;
}

function onOverlayEsc() {
  if (isRecordingShortcut.value) {
    stopShortcutRecording();
    return;
  }
  emit("close");
}

function normalizeKey(key: string): string | null {
  if (["Control", "Shift", "Alt", "Meta", "OS"].includes(key)) return null;
  if (KEY_ALIASES[key]) return KEY_ALIASES[key];
  if (key.length === 1) return key.toUpperCase();
  if (key.startsWith("Key") && key.length === 4) return key.slice(3);
  if (key.startsWith("Digit") && key.length === 6) return key.slice(5);
  return key;
}

function onShortcutKeydown(e: KeyboardEvent) {
  if (!isRecordingShortcut.value) return;
  e.preventDefault();
  e.stopPropagation();

  if (e.key === "Escape") {
    stopShortcutRecording();
    return;
  }

  // Enter / Escape alone must not become the shortcut
  if (e.key === "Enter" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
    return;
  }

  const key = normalizeKey(e.key);
  if (!key) return;

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(key);

  // Require at least one modifier for a global shortcut
  if (parts.length < 2) return;

  const combo = parts.join("+");
  settingsStore.updateSetting("global_shortcut", combo);
  stopShortcutRecording();
}

const TYPE_LABELS: Record<string, string> = {
  text: "settings.tags.typeText",
  code: "settings.tags.typeCode",
  link: "settings.tags.typeLink",
  image: "settings.tags.typeImage",
  file: "settings.tags.typeFile",
  sensitive: "settings.stats.sensitive",
};

const typeDistribution = computed(() => {
  const distribution = stats.value?.type_distribution ?? {};
  const total = Math.max(stats.value?.total_records ?? 0, 1);
  return Object.entries(distribution).map(([key, rawCount]) => {
    const count = Number(rawCount) || 0;
    return {
      key,
      count,
      label: TYPE_LABELS[key] ? t(TYPE_LABELS[key]) : key,
      percent: Math.max(4, Math.round((count / total) * 100)),
    };
  });
});

async function exportData() {
  exportStatus.value = "";
  exportStatusKind.value = "";
  isExporting.value = true;
  try {
    const path = await save({
      defaultPath: `clipvault-export-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "ClipVault JSON", extensions: ["json"] }],
    });
    if (!path) return;
    // Backend streams JSON to disk — avoids holding the full export in JS/Rust heap.
    await invoke("export_data", { path });
    exportStatus.value = t('settings.data.exportDone');
    exportStatusKind.value = "success";
  } catch (e) {
    console.error("Export failed:", e);
    exportStatus.value = t('settings.data.exportFailed', { error: String(e) });
    exportStatusKind.value = "error";
  } finally {
    isExporting.value = false;
  }
}

async function importData() {
  importStatus.value = "";
  importStatusKind.value = "";
  isImporting.value = true;
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: "ClipVault JSON", extensions: ["json"] }],
    });
    if (!path || Array.isArray(path)) return;
    const imported = await invoke<number>("import_data_from_path", { path });
    await clipboardStore.loadRecords();
    importStatus.value = t('settings.data.importDone', { count: imported });
    importStatusKind.value = "success";
  } catch (e) {
    console.error("Import failed:", e);
    importStatus.value = t('settings.data.importFailed', { error: String(e) });
    importStatusKind.value = "error";
  } finally {
    isImporting.value = false;
  }
}

function formatSyncTime(iso: string) {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

async function flushSettings() {
  await settingsStore.saveSettings();
}

async function webdavTest() {
  webdavStatus.value = "";
  webdavStatusKind.value = "";
  webdavBusy.value = true;
  webdavAction.value = "test";
  try {
    await flushSettings();
    await invoke("webdav_test_connection");
    webdavStatus.value = t('settings.data.webdavConnected');
    webdavStatusKind.value = "success";
  } catch (e) {
    webdavStatus.value = t('settings.data.webdavConnectFailed', { error: String(e) });
    webdavStatusKind.value = "error";
  } finally {
    webdavBusy.value = false;
    webdavAction.value = "";
  }
}

async function runWebDav(
  action: "pull" | "push" | "sync",
  command: "webdav_pull" | "webdav_push" | "webdav_sync",
) {
  webdavStatus.value = "";
  webdavStatusKind.value = "";
  webdavBusy.value = true;
  webdavAction.value = action;
  try {
    await flushSettings();
    const result = await invoke<WebDavSyncResult>(command);
    webdavStatus.value = result.message;
    webdavStatusKind.value = "success";
    await settingsStore.loadSettings();
    if (action === "pull" || action === "sync") {
      await clipboardStore.loadRecords();
      await clipboardStore.loadStats();
    }
  } catch (e) {
    webdavStatus.value = `${action === "pull" ? t('settings.data.webdavPullFailed', { error: String(e) }) : action === "push" ? t('settings.data.webdavPushFailed', { error: String(e) }) : t('settings.data.webdavSyncFailed', { error: String(e) })}`;
    webdavStatusKind.value = "error";
  } finally {
    webdavBusy.value = false;
    webdavAction.value = "";
  }
}

async function webdavPull() {
  await runWebDav("pull", "webdav_pull");
}

async function webdavPush() {
  await runWebDav("push", "webdav_push");
}

async function webdavSyncNow() {
  await runWebDav("sync", "webdav_sync");
}

async function clearHistory() {
  const ok = await confirm({
    title: t('confirm.clearHistoryTitle'),
    message: t('confirm.clearHistoryMsg'),
    confirmText: t('confirm.clearHistoryConfirm'),
    cancelText: t('common.cancel'),
    danger: true,
  });
  if (!ok) return;
  try {
    await invoke("clear_history");
    await clipboardStore.loadRecords();
    toast(t('confirm.historyCleared'), "success");
  } catch (e) {
    console.error("Clear history failed:", e);
    toast(t('confirm.clearHistoryFailed'), "error");
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function onWindowKeydown(e: KeyboardEvent) {
  if (!isRecordingShortcut.value) return;
  onShortcutKeydown(e);
}

onMounted(() => {
  clipboardStore.loadStats();
  window.addEventListener("keydown", onWindowKeydown, true);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onWindowKeydown, true);
  if (rulesCommitTimer) flushAutoTagRules();
});
</script>

<style scoped>
.settings-overlay {
  position: fixed;
  inset: 0;
  background: transparent;
  display: flex;
  z-index: 200;
}

.settings-window {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.settings-header.with-chrome {
  padding: 0 0 0 16px;
  height: 38px;
  min-height: 38px;
}

.settings-header-right {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 100%;
  -webkit-app-region: no-drag;
}

.settings-header.with-chrome .settings-header-right {
  margin-right: 0;
}

.settings-title {
  font-size: var(--text-base);
  font-weight: 600;
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.settings-main {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.settings-nav {
  width: 180px;
  background: var(--bg-elevated);
  border-right: 1px solid var(--border-subtle);
  padding: 12px 0 16px;
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow-y: auto;
  transition: background var(--transition-smooth), border-color var(--transition-smooth);
}

.nav-back {
  color: var(--text-secondary);
  font-weight: 500;
}

.nav-back:hover {
  color: var(--text-primary);
}

.nav-divider {
  height: 1px;
  margin: 8px 16px;
  background: var(--border-subtle);
  flex-shrink: 0;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  margin: 0;
  padding: 8px 16px;
  border: none;
  border-left: 2px solid transparent;
  background: transparent;
  font: inherit;
  font-size: var(--text-md);
  line-height: 1;
  text-align: left;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
}

.nav-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent);
  border-left-color: var(--accent);
}

.nav-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  color: inherit;
  line-height: 0;
}

.nav-label {
  line-height: 1.2;
}

.settings-body {
  flex: 1;
  padding: 20px 24px;
  overflow-y: auto;
  min-width: 0;
}

.settings-section {
  margin-bottom: 24px;
}

.settings-section-title {
  font-size: var(--text-lg);
  font-weight: 600;
  margin-bottom: 14px;
  color: var(--text-primary);
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 0;
  border-bottom: 1px solid var(--border-subtle);
  gap: 16px;
  transition: border-color var(--transition-smooth);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-label {
  font-size: var(--text-md);
  color: var(--text-primary);
  flex: 1;
}

.setting-desc {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  margin-top: 2px;
}

.shortcut-btn {
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 5px 12px;
  font-size: var(--text-sm);
  font-family: var(--font-mono);
  color: var(--text-secondary);
  min-width: 140px;
  text-align: center;
  cursor: pointer;
  transition: background var(--transition-smooth), border-color var(--transition-smooth), color var(--transition-fast);
}

.shortcut-btn:hover {
  border-color: var(--accent);
  color: var(--text-primary);
}

.shortcut-btn.recording {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
  animation: pulse-border 1.2s ease-in-out infinite;
}

@keyframes pulse-border {
  50% { opacity: 0.75; }
}

.kbd-display {
  background: var(--bg-active);
  padding: 3px 8px;
  border-radius: 4px;
  font-size: var(--text-sm);
  font-family: var(--font-mono);
  color: var(--text-secondary);
  white-space: nowrap;
}

.segmented {
  display: flex;
  background: var(--bg-active);
  border-radius: var(--radius-sm);
  padding: 2px;
  gap: 2px;
  flex-shrink: 0;
}

.segment-btn {
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  padding: 4px 10px;
  border-radius: calc(var(--radius-sm) - 1px);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.segment-btn:hover {
  color: var(--text-primary);
}

.segment-btn.selected {
  background: var(--bg-elevated);
  color: var(--accent);
  box-shadow: 0 0 0 1px var(--border-subtle);
}

.slider-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.slider-value {
  font-size: var(--text-md);
  font-family: var(--font-mono);
  color: var(--accent);
  min-width: 50px;
  text-align: right;
}

input[type="range"] {
  -webkit-appearance: none;
  width: 140px;
  height: 4px;
  background: var(--bg-active);
  border-radius: 2px;
  outline: none;
  cursor: pointer;
  transition: background var(--transition-smooth);
}

input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 14px;
  height: 14px;
  background: var(--accent);
  border-radius: 50%;
  cursor: pointer;
}

/* Theme cards */
.theme-cards {
  display: flex;
  gap: 10px;
  margin-bottom: 16px;
}

.theme-card {
  flex: 1;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px;
  cursor: pointer;
  text-align: center;
  transition: all var(--transition-fast);
}

.theme-card:hover {
  border-color: var(--accent);
}

.theme-card.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.theme-card:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.theme-preview {
  width: 100%;
  height: 36px;
  border-radius: var(--radius-sm);
  margin-bottom: var(--space-2);
}

.theme-dark { background: linear-gradient(135deg, #181a22, #1e2130); }
.theme-light { background: linear-gradient(135deg, #ffffff, #f0f2f8); border: 1px solid rgba(0,0,0,0.06); }
.theme-oled { background: #000000; }
.theme-system { background: linear-gradient(135deg, #181a22 50%, #ffffff 50%); }

.theme-name {
  font-size: var(--text-sm);
  font-weight: 500;
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

/* Mode cards */
.mode-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.mode-card {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 14px;
  cursor: pointer;
  text-align: center;
  transition: all var(--transition-fast);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
}

.mode-card:hover {
  border-color: var(--accent);
}

.mode-card.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.mode-icon {
  display: flex;
  color: var(--accent);
  line-height: 1;
}

.mode-title {
  font-size: var(--text-base);
  font-weight: 600;
  color: var(--text-primary);
}

.mode-desc {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  line-height: 1.4;
}

/* Auto-tag rules */
.auto-tag-panel {
  margin-top: 4px;
  padding: 12px;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.auto-tag-panel-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 12px;
}

.auto-tag-panel-title {
  font-size: var(--text-md);
  font-weight: 600;
  color: var(--text-primary);
}

.auto-tag-panel-meta {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.auto-tag-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 28px 16px;
  text-align: center;
  color: var(--text-tertiary);
  border: 1px dashed var(--border-default);
  border-radius: var(--radius-md);
  background: var(--accent-softer);
}

.auto-tag-empty p {
  margin: 0;
  font-size: var(--text-md);
  line-height: 1.5;
  max-width: 260px;
}

.auto-tag-rules {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.auto-tag-rule {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
}

.auto-tag-rule:hover {
  border-color: var(--border-default);
}

.auto-tag-rule:focus-within {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border-default));
  box-shadow: 0 0 0 3px var(--accent-softer);
}

.auto-tag-rule-top {
  display: flex;
  align-items: center;
  gap: 8px;
}

.auto-tag-rule-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.auto-tag-rule-index {
  flex: 1;
  min-width: 0;
  font-size: var(--text-sm);
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--text-secondary);
}

.auto-tag-remove {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  flex-shrink: 0;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.auto-tag-remove:hover {
  background: var(--danger-soft);
  color: var(--danger);
}

.auto-tag-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.auto-tag-field-label {
  font-size: var(--text-xs);
  font-weight: 500;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-tertiary);
}

.auto-tag-input {
  width: 100%;
  height: var(--btn-height-lg);
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: var(--text-md);
  transition: border-color var(--transition-fast), background var(--transition-fast);
}

.auto-tag-input::placeholder {
  color: var(--text-tertiary);
}

.auto-tag-input:hover {
  border-color: var(--border-default);
}

.auto-tag-input:focus {
  outline: none;
  border-color: var(--accent);
  background: var(--bg-base);
}

.auto-tag-input-mono {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.auto-tag-keyword-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.auto-tag-chip {
  display: inline-flex;
  align-items: center;
  max-width: 100%;
  padding: 2px 7px;
  border-radius: var(--radius-pill);
  font-size: var(--text-xs);
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.auto-tag-chip-kw {
  background: var(--bg-active);
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.auto-tag-type-chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.auto-tag-type-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 28px;
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-pill);
  background: var(--bg-input);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition:
    background var(--transition-fast),
    border-color var(--transition-fast),
    color var(--transition-fast),
    box-shadow var(--transition-fast);
}

.auto-tag-type-chip:hover {
  border-color: var(--border-default);
  color: var(--text-primary);
}

.auto-tag-type-chip.active {
  --chip-accent: var(--accent);
  background: color-mix(in srgb, var(--chip-accent) 14%, transparent);
  border-color: color-mix(in srgb, var(--chip-accent) 45%, transparent);
  color: var(--chip-accent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--chip-accent) 20%, transparent);
}

.auto-tag-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border-subtle);
}

/* Ignore apps */
.ignore-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 8px;
}

.ignore-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
}

.ignore-item:hover {
  background: var(--bg-hover);
}

.ignore-icon {
  display: flex;
  color: var(--text-tertiary);
}

.ignore-name {
  flex: 1;
  font-size: var(--text-md);
  color: var(--text-secondary);
}

.ignore-remove {
  font-size: var(--text-md);
  color: var(--text-tertiary);
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 3px;
  transition: all var(--transition-fast);
}

.ignore-remove:hover {
  background: var(--danger-soft);
  color: var(--danger);
}

.ignore-add-row {
  display: flex;
  gap: var(--space-2);
}

.ignore-input {
  flex: 1;
  height: var(--btn-height-lg);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 0 var(--space-3);
  font-size: var(--text-md);
  color: var(--text-primary);
  transition: border-color var(--transition-fast), background var(--transition-smooth);
}

.ignore-input:focus {
  border-color: var(--border-focus);
}

.ignore-add-btn {
  height: var(--btn-height-lg);
  padding: 0 var(--space-4);
  background: var(--accent);
  color: white;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  font-weight: 500;
  cursor: pointer;
  transition: background var(--transition-fast);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.ignore-add-btn:hover {
  background: var(--accent-hover);
}

.btn:disabled {
  cursor: default;
  opacity: 0.55;
}

.stats-dashboard {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 22px;
}

.stats-card {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 14px;
}

.stats-value {
  font-family: var(--font-mono);
  font-size: 1.5rem;
  font-weight: 700;
  line-height: 1;
}

.stats-value.accent { color: var(--accent); }
.stats-value.success { color: var(--success); }
.stats-value.warning { color: var(--warning); }
.stats-value.sensitive { color: var(--sensitive); }

.stats-label {
  margin-top: 6px;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.type-bars {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.type-row-label {
  display: flex;
  justify-content: space-between;
  margin-bottom: 5px;
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.type-track {
  height: 6px;
  overflow: hidden;
  border-radius: var(--radius-pill);
  background: var(--bg-active);
}

.type-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
}

.data-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 10px;
  padding: 12px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
}

.webdav-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 10px;
}

.webdav-actions {
  flex-wrap: wrap;
  justify-content: flex-start;
}

.webdav-actions .btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.storage-card {
  align-items: flex-start;
}

.storage-card-main {
  min-width: 0;
  flex: 1;
}

.storage-path {
  margin-top: 8px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  background: var(--bg-active);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-family: var(--font-mono);
  line-height: 1.4;
  word-break: break-all;
  user-select: text;
}

.status-line {
  margin: 0 0 12px;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  background: var(--accent-soft);
  color: var(--accent);
  font-size: var(--text-sm);
}

.status-line.success {
  background: var(--success-soft);
  color: var(--success);
}

.status-line.error {
  background: var(--danger-soft);
  color: var(--danger);
}

/* Help / Guide */
.guide-block {
  padding: 12px 14px;
  margin-bottom: 10px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
}

.guide-heading {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: var(--text-md);
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 6px;
}

.guide-heading .app-icon {
  color: var(--accent);
}

.guide-text {
  font-size: var(--text-md);
  line-height: 1.7;
  color: var(--text-secondary);
}

.guide-kbd {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--bg-active);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--text-primary);
}

/* About */
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
  font-weight: 700;
  margin-bottom: 4px;
}

.about-version {
  font-size: var(--text-md);
  color: var(--accent);
  font-family: var(--font-mono);
  margin-bottom: 8px;
}

.about-desc {
  font-size: var(--text-md);
  color: var(--text-tertiary);
}

@media (max-width: 720px) {
  .settings-nav {
    width: 56px;
    padding: 8px 0 12px;
  }

  .settings-nav .nav-item {
    justify-content: center;
    padding: 10px 8px;
  }

  .settings-nav .nav-label {
    display: none;
  }

  .theme-cards {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
}
</style>
