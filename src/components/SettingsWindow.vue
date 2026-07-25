<template>
  <div class="settings-overlay" tabindex="-1" @keydown.esc="onOverlayEsc">
    <div class="settings-window panel-surface">
      <!-- Header -->
      <div class="settings-header" :class="{ 'with-chrome': isWindowMode }" data-tauri-drag-region>
        <span class="settings-title"><AppIcon name="settings" :size="15" /> 设置</span>
        <div v-if="isWindowMode" class="settings-header-right">
          <WindowControls />
        </div>
      </div>

      <div class="settings-main">
        <!-- Nav -->
        <nav class="settings-nav">
          <button type="button" class="nav-item nav-back" title="返回" aria-label="返回" @click="emit('close')">
            <span class="nav-icon"><AppIcon name="back" :size="15" /></span>
            <span class="nav-label">返回</span>
          </button>
          <div class="nav-divider" aria-hidden="true"></div>
          <button
            v-for="section in SECTIONS"
            :key="section.key"
            type="button"
            class="nav-item"
            :class="{ active: activeSection === section.key }"
            :title="section.label"
            :aria-label="section.label"
            @click="activeSection = section.key"
          >
            <span class="nav-icon"><AppIcon :name="section.icon" :size="15" /></span>
            <span class="nav-label">{{ section.label }}</span>
          </button>
        </nav>

        <!-- Body -->
        <div class="settings-body">
          <!-- Shortcuts -->
          <template v-if="activeSection === 'shortcuts'">
            <div class="settings-section">
              <div class="settings-section-title">快捷键</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">全局快捷键</div>
                  <div class="setting-desc">唤起悬浮面板</div>
                </div>
                <button
                  class="shortcut-btn"
                  :class="{ recording: isRecordingShortcut }"
                  type="button"
                  @click="startShortcutRecording"
                  @keydown="onShortcutKeydown"
                >
                  {{ isRecordingShortcut ? "按下快捷键…" : settings.global_shortcut }}
                </button>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">搜索聚焦</div>
                  <div class="setting-desc">在面板内快速聚焦搜索框</div>
                </div>
                <span class="kbd-display">/ 或 Ctrl+K</span>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">纯文本粘贴</div>
                  <div class="setting-desc">面板内快捷切换</div>
                </div>
                <span class="kbd-display">Alt + V</span>
              </div>
            </div>
            <div class="settings-section">
              <div class="settings-section-title">行为</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">粘贴后自动隐藏</div>
                  <div class="setting-desc">悬浮模式隐藏到托盘；窗口模式最小化到任务栏（关闭则粘贴后保持打开）</div>
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
                  <div class="setting-label">默认粘贴模式</div>
                  <div class="setting-desc">从面板粘贴时使用的格式</div>
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
                    {{ mode.label }}
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- Appearance -->
          <template v-else-if="activeSection === 'appearance'">
            <div class="settings-section">
              <div class="settings-section-title">主题</div>
              <div class="theme-cards" role="radiogroup" aria-label="主题">
                <div
                  v-for="(t, idx) in THEMES"
                  :key="t.key"
                  class="theme-card"
                  role="radio"
                  :data-theme="t.key"
                  :aria-checked="settings.theme === t.key"
                  :aria-label="t.label"
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
                  <div class="theme-name"><AppIcon :name="t.icon" :size="13" /> {{ t.label }}</div>
                </div>
              </div>
            </div>
            <div class="settings-section">
              <div class="settings-section-title">应用模式</div>
              <div class="mode-grid" role="radiogroup" aria-label="应用模式">
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
                  <span class="mode-title">{{ mode.label }}</span>
                  <span class="mode-desc">{{ mode.desc }}</span>
                </button>
              </div>
            </div>
            <div class="settings-section">
              <div class="settings-section-title">面板外观</div>
              <div class="setting-row">
                <div class="setting-label">圆角大小</div>
                <div class="slider-row">
                  <input type="range" min="0" max="40" aria-label="圆角大小" :value="settings.panel_radius" @input="(e) => update('panel_radius', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.panel_radius }}px</span>
                </div>
              </div>
              <div class="setting-row">
                  <div class="setting-label">不透明度</div>
                <div class="slider-row">
                  <input type="range" min="60" max="100" aria-label="不透明度" :value="settings.panel_opacity" @input="(e) => update('panel_opacity', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.panel_opacity }}%</span>
                </div>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">毛玻璃效果</div>
                  <div class="setting-desc">默认关闭以降低开销；仅悬浮模式生效，窗口模式始终关闭</div>
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
                <div class="setting-label">动画效果</div>
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
                <div class="setting-label">字体大小</div>
                <div class="slider-row">
                  <input type="range" min="11" max="18" aria-label="字体大小" :value="settings.font_size" @input="(e) => update('font_size', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.font_size }}px</span>
                </div>
              </div>
            </div>
          </template>

          <!-- History -->
          <template v-else-if="activeSection === 'history'">
            <div class="settings-section">
              <div class="settings-section-title">历史记录</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">最大记录数</div>
                  <div class="setting-desc">超出后自动清理旧记录</div>
                </div>
                <div class="slider-row">
                  <input type="range" min="100" max="10000" step="100" aria-label="最大记录数" :value="settings.max_records" @input="(e) => update('max_records', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.max_records }}</span>
                </div>
              </div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">回收站保留天数</div>
                  <div class="setting-desc">回收站内超过天数后永久删除（收藏、置顶除外）</div>
                </div>
                <div class="slider-row">
                  <input type="range" min="7" max="365" step="1" aria-label="回收站保留天数" :value="settings.retention_days" @input="(e) => update('retention_days', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ settings.retention_days }} 天</span>
                </div>
              </div>
            </div>
          </template>

          <!-- Tags -->
          <template v-else-if="activeSection === 'tags'">
            <div class="settings-section">
              <div class="settings-section-title">自动打标</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">根据内容自动打标签</div>
                  <div class="setting-desc">新记录匹配规则时自动附加标签；类型或关键词命中其一即可</div>
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
                  <div class="auto-tag-panel-title">匹配规则</div>
                  <div class="auto-tag-panel-meta">{{ rulesDraft.length }} 条</div>
                </div>

                <div v-if="rulesDraft.length === 0" class="auto-tag-empty">
                  <AppIcon name="tag" :size="18" />
                  <p>暂无规则。添加后，新复制的内容会按规则自动打标。</p>
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
                      <span class="auto-tag-rule-index">规则 {{ index + 1 }}</span>
                      <button
                        type="button"
                        class="auto-tag-remove"
                        title="删除规则"
                        aria-label="删除规则"
                        @click="removeAutoTagRule(index)"
                      >
                        <AppIcon name="close" :size="12" />
                      </button>
                    </header>

                    <label class="auto-tag-field">
                      <span class="auto-tag-field-label">标签名</span>
                      <input
                        class="auto-tag-input"
                        :value="rule.tag_name"
                        placeholder="例如：部署"
                        @input="updateRuleField(index, 'tag_name', (($event.target as HTMLInputElement).value))"
                      />
                    </label>

                    <label class="auto-tag-field">
                      <span class="auto-tag-field-label">关键词</span>
                      <input
                        class="auto-tag-input auto-tag-input-mono"
                        :value="rule.keywords.join(', ')"
                        placeholder="逗号分隔，如 deploy, docker"
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
                      <span class="auto-tag-field-label">内容类型</span>
                      <div class="auto-tag-type-chips" role="group" aria-label="内容类型">
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
                          {{ ct.label }}
                        </button>
                      </div>
                    </div>
                  </article>
                </div>

                <div class="auto-tag-actions">
                  <button type="button" class="btn btn-secondary" @click="addAutoTagRule">
                    <AppIcon name="plus" :size="13" /> 添加规则
                  </button>
                  <button type="button" class="btn btn-secondary" @click="restoreDefaultAutoTagRules">
                    <AppIcon name="restore" :size="13" /> 恢复默认
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- Privacy -->
          <template v-else-if="activeSection === 'privacy'">
            <div class="settings-section">
              <div class="settings-section-title">敏感内容</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">自动检测敏感内容</div>
                  <div class="setting-desc">检测密码、验证码等</div>
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
                  <div class="setting-label">自动过期时间</div>
                  <div class="setting-desc">敏感内容自动删除</div>
                </div>
                <div class="slider-row">
                  <input type="range" min="10" max="3600" step="10" aria-label="敏感内容自动过期秒数" :value="settings.sensitive_auto_expire_seconds" @input="(e) => update('sensitive_auto_expire_seconds', Number((e.target as HTMLInputElement).value))" />
                  <span class="slider-value">{{ Math.floor(settings.sensitive_auto_expire_seconds / 60) }} 分钟</span>
                </div>
              </div>
            </div>
            <div class="settings-section">
              <div class="settings-section-title">忽略应用</div>
              <div class="ignore-list">
                <div v-for="app in settings.ignored_apps" :key="app" class="ignore-item">
                  <span class="ignore-icon"><AppIcon name="monitor" :size="14" /></span>
                  <span class="ignore-name">{{ app }}</span>
                  <button type="button" class="ignore-remove" :aria-label="`移除 ${app}`" @click="removeIgnoredApp(app)"><AppIcon name="close" :size="12" /></button>
                </div>
              </div>
              <div class="ignore-add-row">
                <input class="ignore-input" aria-label="忽略应用进程名" placeholder="输入应用进程名…" v-model="newIgnoredApp" @keydown.enter="addIgnoredApp" />
                <button type="button" class="ignore-add-btn" @click="addIgnoredApp"><AppIcon name="plus" :size="13" /> 添加</button>
              </div>
            </div>
          </template>

          <!-- Data -->
          <template v-else-if="activeSection === 'stats'">
            <div class="stats-dashboard">
              <div class="stats-card">
                <div class="stats-value accent">{{ stats?.total_records ?? 0 }}</div>
                <div class="stats-label">总记录</div>
              </div>
              <div class="stats-card">
                <div class="stats-value success">{{ stats?.total_copies ?? 0 }}</div>
                <div class="stats-label">复制次数</div>
              </div>
              <div class="stats-card">
                <div class="stats-value warning">{{ stats?.favorites_count ?? 0 }}</div>
                <div class="stats-label">收藏</div>
              </div>
              <div class="stats-card">
                <div class="stats-value sensitive">{{ stats?.sensitive_count ?? 0 }}</div>
                <div class="stats-label">敏感</div>
              </div>
            </div>

            <div class="settings-section">
              <div class="settings-section-title">类型分布</div>
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
              <div class="settings-section-title">存储</div>
              <div class="data-card storage-card">
                <div class="storage-card-main">
                  <div class="setting-label">本地存储占用</div>
                  <div class="setting-desc">
                    文本内容估算 + media 图片目录（不含 SQLite 索引开销）
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
              <div class="settings-section-title">数据管理</div>
              <div class="data-card">
                <div>
                  <div class="setting-label">导出记录</div>
                  <div class="setting-desc">保存为 ClipVault JSON 备份文件，可再次导入</div>
                </div>
                <button class="btn btn-secondary" :disabled="isExporting" @click="exportData">
                  <AppIcon v-if="!isExporting" name="package" :size="13" />
                  {{ isExporting ? '导出中…' : '选择保存位置' }}
                </button>
              </div>
              <div v-if="exportStatus" class="status-line" :class="exportStatusKind">{{ exportStatus }}</div>

              <div class="data-card">
                <div>
                  <div class="setting-label">导入记录</div>
                  <div class="setting-desc">读取 JSON 备份，按内容 hash 自动跳过重复记录</div>
                </div>
                <button class="btn btn-secondary" :disabled="isImporting" @click="importData">
                  <AppIcon v-if="!isImporting" name="history" :size="13" />
                  {{ isImporting ? '导入中…' : '选择备份文件' }}
                </button>
              </div>
              <div v-if="importStatus" class="status-line" :class="importStatusKind">{{ importStatus }}</div>

              <div class="setting-row">
                <div>
                  <div class="setting-label">清理历史</div>
                  <div class="setting-desc">手动清理所有记录</div>
                </div>
                <button class="btn btn-danger" @click="clearHistory"><AppIcon name="trash" :size="13" /> 清空历史</button>
              </div>
            </div>
          </template>

          <!-- System -->
          <template v-else-if="activeSection === 'system'">
            <div class="settings-section">
              <div class="settings-section-title">系统</div>
              <div class="setting-row">
                <div>
                  <div class="setting-label">开机自启动</div>
                  <div class="setting-desc">Windows 启动时自动运行</div>
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
                  <div class="setting-label">最小化到托盘</div>
                  <div class="setting-desc">关闭按钮最小化而非退出</div>
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
            </div>
          </template>

          <!-- Help -->
          <template v-else-if="activeSection === 'help'">
            <div class="settings-section">
              <div class="settings-section-title">使用指南</div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="keyboard" :size="14" /> 唤起面板</div>
                <div class="guide-text">在任意应用中按下全局快捷键 <span class="guide-kbd">{{ settings.global_shortcut }}</span> 即可唤起剪贴板面板；可在“快捷键”中自定义。</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="clipboard" :size="14" /> 自动记录</div>
                <div class="guide-text">复制任意文本、链接、代码、图片或文件，内容会自动进入历史列表，无需手动保存。</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="paste" :size="14" /> 粘贴到当前应用</div>
                <div class="guide-text">双击条目、按 <span class="guide-kbd">Enter</span> 或右键菜单选“粘贴”，会把内容写回系统剪贴板（图片优先以 PNG 格式写入），并把焦点还给唤出面板前的应用，再模拟 Ctrl+V；按 <span class="guide-kbd">Alt + V</span> 或选“纯文本粘贴”则去除格式。</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="search" :size="14" /> 搜索与筛选</div>
                <div class="guide-text">面板内按 <span class="guide-kbd">/</span> 或 <span class="guide-kbd">Ctrl + K</span> 快速聚焦搜索框（支持正文、来源与标签；短关键词也可搜）。左侧导航可按类型、收藏、标签筛选。独立窗口模式下，列表工具栏可切换排序（最新 / 最早 / 最近创建 / 粘贴最多；置顶仍优先）。</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="star" :size="14" /> 收藏、置顶与标签</div>
                <div class="guide-text">常用内容可收藏或置顶，不会被自动清理；也可手动为条目添加标签。开启「自动打标」（设置 → 标签，默认开）后，新记录会按内容类型或关键词规则打上标签（如链接、部署、前端）；可自定义规则。同一内容再次复制不会重复打标。</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="shield" :size="14" /> 隐私保护</div>
                <div class="guide-text">开启“自动检测敏感内容”后，密码、验证码等会被标记并在设定时间后自动删除；可在“隐私”中将密码管理器等应用加入忽略列表，不记录其剪贴板。</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="trash" :size="14" /> 回收站与清理</div>
                <div class="guide-text">删除的条目会进入回收站，超过“保留天数”后自动清除（收藏、置顶除外）；历史超过最大记录数时也会自动淘汰最旧的普通记录。</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="panel" :size="14" /> 两种应用模式</div>
                <div class="guide-text">“悬浮面板”无边框置顶、失焦自动隐藏，适合快速粘贴（毛玻璃仅在此模式生效）；“独立窗口”带侧边栏与任务栏入口，适合长期管理（为降低合成开销自动关闭毛玻璃）。两种模式都会记住你上次调整的窗口大小。可在“外观”中切换。</div>
              </div>

              <div class="guide-block">
                <div class="guide-heading"><AppIcon name="stats" :size="14" /> 数据与占用</div>
                <div class="guide-text">设置 → 统计可查看记录概览、类型分布、本地存储占用估算，以及数据目录绝对路径（默认在 %LOCALAPPDATA%\ClipVault）。</div>
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
                <div class="about-version">版本 0.1.0</div>
                <div class="about-desc">Windows 剪贴板管理工具 · Tauri + Vue 3 + Rust</div>
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
import { useSettingsStore } from "../stores/settings";
import { useClipboardStore } from "../stores/clipboard";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { Settings } from "../types";
import { DEFAULT_AUTO_TAG_RULES, type AutoTagRule } from "../types";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";
import WindowControls from "./WindowControls.vue";
import appIconUrl from "../assets/app-icon-128.png";
import { resolveKnownTagColors, resolveTagPalette } from "../utils/themeColors";

const emit = defineEmits<{ close: [] }>();
const settingsStore = useSettingsStore();
const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const settings = settingsStore.settings;
const isWindowMode = computed(() => settings.app_mode === "window");
const stats = computed(() => clipboardStore.stats);

const activeSection = ref("appearance");
const newIgnoredApp = ref("");
const exportStatus = ref("");
const exportStatusKind = ref<"success" | "error" | "">("");
const importStatus = ref("");
const importStatusKind = ref<"success" | "error" | "">("");
const isExporting = ref(false);
const isImporting = ref(false);
const isRecordingShortcut = ref(false);

const CONTENT_TYPE_OPTIONS = [
  { value: "text", label: "文本", icon: "type" as AppIconName, color: "var(--type-text)" },
  { value: "code", label: "代码", icon: "code" as AppIconName, color: "var(--type-code)" },
  { value: "link", label: "链接", icon: "link" as AppIconName, color: "var(--type-link)" },
  { value: "image", label: "图片", icon: "image" as AppIconName, color: "var(--type-image)" },
  { value: "file", label: "文件", icon: "file" as AppIconName, color: "var(--type-file)" },
] as const;

const SECTIONS: { key: string; icon: AppIconName; label: string }[] = [
  { key: "appearance", icon: "palette", label: "外观" },
  { key: "shortcuts", icon: "keyboard", label: "快捷键" },
  { key: "history", icon: "history", label: "历史" },
  { key: "tags", icon: "tag", label: "标签" },
  { key: "privacy", icon: "shield", label: "隐私" },
  { key: "system", icon: "settings", label: "系统" },
  { key: "data", icon: "package", label: "数据" },
  { key: "stats", icon: "stats", label: "统计" },
  { key: "help", icon: "help", label: "帮助" },
  { key: "about", icon: "info", label: "关于" },
];

const THEMES: { key: Settings["theme"]; icon: AppIconName; label: string }[] = [
  { key: "dark", icon: "moon", label: "暗色" },
  { key: "light", icon: "sun", label: "亮色" },
  { key: "oled", icon: "circle", label: "深黑" },
  { key: "system", icon: "monitor", label: "跟随系统" },
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
    label: "悬浮面板",
    desc: "无边框置顶，失焦后自动隐藏，适合快速粘贴。",
  },
  {
    key: "window",
    icon: "window" as AppIconName,
    label: "独立窗口应用",
    desc: "显示系统边框和任务栏，不会因失焦关闭，适合长期管理。",
  },
] as const;

const PASTE_MODES = [
  { key: "original", label: "原格式" },
  { key: "plain", label: "纯文本" },
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
  return resolveTagPalette()[0] ?? "#6366f1";
}

function addIgnoredApp() {
  const name = newIgnoredApp.value.trim();
  if (!name) {
    toast("请输入应用名称", "warning");
    return;
  }
  if (settings.ignored_apps.includes(name)) {
    toast("该应用已在忽略列表中", "warning");
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
  text: "文本",
  code: "代码",
  link: "链接",
  image: "图片",
  file: "文件",
  sensitive: "敏感",
};

const typeDistribution = computed(() => {
  const distribution = stats.value?.type_distribution ?? {};
  const total = Math.max(stats.value?.total_records ?? 0, 1);
  return Object.entries(distribution).map(([key, rawCount]) => {
    const count = Number(rawCount) || 0;
    return {
      key,
      count,
      label: TYPE_LABELS[key] ?? key,
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
    exportStatus.value = "导出完成，备份文件已保存。";
    exportStatusKind.value = "success";
  } catch (e) {
    console.error("Export failed:", e);
    exportStatus.value = `导出失败：${String(e)}`;
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
    importStatus.value = `导入完成：新增 ${imported} 条记录。`;
    importStatusKind.value = "success";
  } catch (e) {
    console.error("Import failed:", e);
    importStatus.value = `导入失败：${String(e)}`;
    importStatusKind.value = "error";
  } finally {
    isImporting.value = false;
  }
}

async function clearHistory() {
  const ok = await confirm({
    title: "清空历史",
    message: "确定要清空所有历史记录吗？此操作不可恢复。",
    confirmText: "清空",
    cancelText: "取消",
    danger: true,
  });
  if (!ok) return;
  try {
    await invoke("clear_history");
    await clipboardStore.loadRecords();
    toast("历史已清空", "success");
  } catch (e) {
    console.error("Clear history failed:", e);
    toast("清空失败", "error");
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
  border-radius: 6px;
  margin-bottom: 6px;
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
  gap: 6px;
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
  height: 32px;
  padding: 0 10px;
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
  border-radius: 999px;
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
  gap: 6px;
}

.auto-tag-type-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
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
  gap: 8px;
}

.ignore-input {
  flex: 1;
  height: 32px;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 0 10px;
  font-size: var(--text-md);
  color: var(--text-primary);
  transition: border-color var(--transition-fast), background var(--transition-smooth);
}

.ignore-input:focus {
  border-color: var(--border-focus);
}

.ignore-add-btn {
  height: 32px;
  padding: 0 14px;
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
  border-radius: 99px;
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
  border-radius: 12px;
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
