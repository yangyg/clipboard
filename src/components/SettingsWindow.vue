<template>
  <div class="settings-overlay" tabindex="-1" @keydown.esc="emit('close')">
    <div class="settings-window">
      <!-- Header -->
      <div class="settings-header">
        <span class="settings-title">⚙ 设置</span>
        <button class="icon-btn" title="返回" @click="emit('close')">✕</button>
      </div>

      <!-- Nav -->
      <nav class="settings-nav">
        <div class="settings-nav-title">⚙ ClipVault</div>
        <div
          v-for="section in SECTIONS"
          :key="section.key"
          class="nav-item"
          :class="{ active: activeSection === section.key }"
          @click="activeSection = section.key"
        >
          <span class="nav-icon">{{ section.icon }}</span>
          {{ section.label }}
        </div>
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
              <input class="shortcut-input" :value="settings.global_shortcut" @change="(e) => update('global_shortcut', (e.target as HTMLInputElement).value)" />
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
        </template>

        <!-- Appearance -->
        <template v-else-if="activeSection === 'appearance'">
          <div class="settings-section">
            <div class="settings-section-title">主题</div>
            <div class="theme-cards">
              <div
                v-for="t in THEMES"
                :key="t.key"
                class="theme-card"
                :class="{ selected: settings.theme === t.key }"
                @click="update('theme', t.key)"
              >
                <div class="theme-preview" :class="`theme-${t.key}`"></div>
                <div class="theme-name">{{ t.icon }} {{ t.label }}</div>
              </div>
            </div>
          </div>
          <div class="settings-section">
            <div class="settings-section-title">应用模式</div>
            <div class="mode-grid">
              <button
                v-for="mode in APP_MODES"
                :key="mode.key"
                class="mode-card"
                :class="{ selected: settings.app_mode === mode.key }"
                @click="update('app_mode', mode.key)"
              >
                <span class="mode-icon">{{ mode.icon }}</span>
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
                <input type="range" min="0" max="40" :value="settings.panel_radius" @input="(e) => update('panel_radius', Number((e.target as HTMLInputElement).value))" />
                <span class="slider-value">{{ settings.panel_radius }}px</span>
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-label">透明度</div>
              <div class="slider-row">
                <input type="range" min="60" max="100" :value="settings.panel_opacity" @input="(e) => update('panel_opacity', Number((e.target as HTMLInputElement).value))" />
                <span class="slider-value">{{ settings.panel_opacity }}%</span>
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-label">毛玻璃效果</div>
              <div class="toggle" :class="{ on: settings.enable_blur }" @click="update('enable_blur', !settings.enable_blur)"></div>
            </div>
            <div class="setting-row">
              <div class="setting-label">动画效果</div>
              <div class="toggle" :class="{ on: settings.enable_animation }" @click="update('enable_animation', !settings.enable_animation)"></div>
            </div>
            <div class="setting-row">
              <div class="setting-label">字体大小</div>
              <div class="slider-row">
                <input type="range" min="11" max="18" :value="settings.font_size" @input="(e) => update('font_size', Number((e.target as HTMLInputElement).value))" />
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
                <input type="range" min="100" max="10000" step="100" :value="settings.max_records" @input="(e) => update('max_records', Number((e.target as HTMLInputElement).value))" />
                <span class="slider-value">{{ settings.max_records }}</span>
              </div>
            </div>
            <div class="setting-row">
              <div>
                <div class="setting-label">保留天数</div>
                <div class="setting-desc">超出天数自动删除（收藏除外）</div>
              </div>
              <div class="slider-row">
                <input type="range" min="7" max="365" step="1" :value="settings.retention_days" @input="(e) => update('retention_days', Number((e.target as HTMLInputElement).value))" />
                <span class="slider-value">{{ settings.retention_days }} 天</span>
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
              <div class="toggle" :class="{ on: settings.enable_sensitive_detection }" @click="update('enable_sensitive_detection', !settings.enable_sensitive_detection)"></div>
            </div>
            <div class="setting-row">
              <div>
                <div class="setting-label">自动过期时间</div>
                <div class="setting-desc">敏感内容自动删除</div>
              </div>
              <div class="slider-row">
                <input type="range" min="10" max="3600" step="10" :value="settings.sensitive_auto_expire_seconds" @input="(e) => update('sensitive_auto_expire_seconds', Number((e.target as HTMLInputElement).value))" />
                <span class="slider-value">{{ Math.floor(settings.sensitive_auto_expire_seconds / 60) }} 分钟</span>
              </div>
            </div>
          </div>
          <div class="settings-section">
            <div class="settings-section-title">忽略应用</div>
            <div class="ignore-list">
              <div v-for="app in settings.ignored_apps" :key="app" class="ignore-item">
                <span class="ignore-icon">🖥️</span>
                <span class="ignore-name">{{ app }}</span>
                <button class="ignore-remove" @click="removeIgnoredApp(app)">✕</button>
              </div>
            </div>
            <div class="ignore-add-row">
              <input class="ignore-input" placeholder="输入应用进程名…" v-model="newIgnoredApp" @keydown.enter="addIgnoredApp" />
              <button class="ignore-add-btn" @click="addIgnoredApp">＋ 添加</button>
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
            <div class="setting-row">
              <div>
                <div class="setting-label">本地数据库体积</div>
                <div class="setting-desc">按剪贴板文本内容估算，不包含 SQLite 索引开销</div>
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
                {{ isExporting ? '导出中…' : '📤 选择保存位置' }}
              </button>
            </div>
            <div v-if="exportStatus" class="status-line success">{{ exportStatus }}</div>

            <div class="data-card">
              <div>
                <div class="setting-label">导入记录</div>
                <div class="setting-desc">读取 JSON 备份，按内容 hash 自动跳过重复记录</div>
              </div>
              <button class="btn btn-secondary" :disabled="isImporting" @click="importData">
                {{ isImporting ? '导入中…' : '📥 选择备份文件' }}
              </button>
            </div>
            <div v-if="importStatus" class="status-line">{{ importStatus }}</div>

            <div class="setting-row">
              <div>
                <div class="setting-label">清理历史</div>
                <div class="setting-desc">手动清理所有记录</div>
              </div>
              <button class="btn btn-danger" @click="clearHistory">🗑 清空历史</button>
            </div>
          </div>
          <div class="settings-section">
            <div class="settings-section-title">系统</div>
            <div class="setting-row">
              <div>
                <div class="setting-label">开机自启动</div>
                <div class="setting-desc">Windows 启动时自动运行</div>
              </div>
              <div class="toggle" :class="{ on: settings.auto_start }" @click="update('auto_start', !settings.auto_start)"></div>
            </div>
            <div class="setting-row">
              <div>
                <div class="setting-label">最小化到托盘</div>
                <div class="setting-desc">关闭按钮最小化而非退出</div>
              </div>
              <div class="toggle" :class="{ on: settings.minimize_to_tray }" @click="update('minimize_to_tray', !settings.minimize_to_tray)"></div>
            </div>
          </div>
        </template>

        <!-- About -->
        <template v-else-if="activeSection === 'about'">
          <div class="settings-section">
            <div class="about-content">
              <div class="about-logo">📋</div>
              <div class="about-name">ClipVault</div>
              <div class="about-version">版本 0.1.0</div>
              <div class="about-desc">Windows 剪贴板管理工具 · Tauri + Vue 3 + Rust</div>
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useSettingsStore } from "../stores/settings";
import { useClipboardStore } from "../stores/clipboard";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import type { ClipboardRecord } from "../types";

const emit = defineEmits<{ close: [] }>();
const settingsStore = useSettingsStore();
const clipboardStore = useClipboardStore();
const settings = settingsStore.settings;
const stats = computed(() => clipboardStore.stats);

const activeSection = ref("appearance");
const newIgnoredApp = ref("");
const exportStatus = ref("");
const importStatus = ref("");
const isExporting = ref(false);
const isImporting = ref(false);

const SECTIONS = [
  { key: "shortcuts", icon: "⌨", label: "快捷键" },
  { key: "history", icon: "📋", label: "历史" },
  { key: "appearance", icon: "🎨", label: "外观" },
  { key: "privacy", icon: "🛡", label: "隐私" },
  { key: "stats", icon: "▦", label: "统计" },
  { key: "data", icon: "📦", label: "数据" },
  { key: "about", icon: "ℹ", label: "关于" },
];

const THEMES = [
  { key: "dark", icon: "🌙", label: "暗色" },
  { key: "light", icon: "☀️", label: "亮色" },
  { key: "oled", icon: "✦", label: "深黑" },
  { key: "system", icon: "💻", label: "跟随系统" },
];

const APP_MODES = [
  {
    key: "floating",
    icon: "▣",
    label: "悬浮面板",
    desc: "无边框置顶，失焦后自动隐藏，适合快速粘贴。",
  },
  {
    key: "window",
    icon: "□",
    label: "独立窗口应用",
    desc: "显示系统边框和任务栏，不会因失焦关闭，适合长期管理。",
  },
] as const;

function update(key: string, value: any) {
  settingsStore.updateSetting(key as any, value);
}

function addIgnoredApp() {
  const name = newIgnoredApp.value.trim();
  if (name && !settings.ignored_apps.includes(name)) {
    const updated = [...settings.ignored_apps, name];
    settingsStore.updateSetting("ignored_apps", updated);
    newIgnoredApp.value = "";
  }
}

function removeIgnoredApp(app: string) {
  const updated = settings.ignored_apps.filter((a) => a !== app);
  settingsStore.updateSetting("ignored_apps", updated);
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
  isExporting.value = true;
  try {
    const path = await save({
      defaultPath: `clipvault-export-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "ClipVault JSON", extensions: ["json"] }],
    });
    if (!path) return;
    const json = await invoke<string>("export_data");
    await writeTextFile(path, json);
    exportStatus.value = "导出完成，备份文件已保存。";
  } catch (e) {
    console.error("Export failed:", e);
    exportStatus.value = `导出失败：${String(e)}`;
  } finally {
    isExporting.value = false;
  }
}

async function importData() {
  importStatus.value = "";
  isImporting.value = true;
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: "ClipVault JSON", extensions: ["json"] }],
    });
    if (!path || Array.isArray(path)) return;
    const text = await readTextFile(path);
    const records = JSON.parse(text) as ClipboardRecord[];
    if (!Array.isArray(records)) {
      throw new Error("备份文件格式不正确");
    }
    const imported = await clipboardStore.importRecords(records);
    importStatus.value = `导入完成：新增 ${imported} 条记录。`;
  } catch (e) {
    console.error("Import failed:", e);
    importStatus.value = `导入失败：${String(e)}`;
  } finally {
    isImporting.value = false;
  }
}

async function clearHistory() {
  if (confirm("确定要清空所有历史记录吗？此操作不可恢复。")) {
    try {
      await invoke("clear_history");
      await clipboardStore.loadRecords();
    } catch (e) {
      console.error("Clear history failed:", e);
    }
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

onMounted(() => {
  clipboardStore.loadStats();
});
</script>

<style scoped>
.settings-overlay {
  position: fixed;
  inset: 0;
  background: var(--bg-surface);
  display: flex;
  z-index: 200;
}

.settings-window {
  width: 100%;
  height: 100%;
  background: var(--bg-surface);
  display: flex;
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

.settings-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.settings-header .icon-btn {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  cursor: pointer;
  font-size: 14px;
  transition: all var(--transition-fast);
}

.settings-header .icon-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.settings-nav {
  width: 180px;
  background: var(--bg-elevated);
  border-right: 1px solid var(--border-subtle);
  padding: 16px 0;
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  transition: background var(--transition-smooth), border-color var(--transition-smooth);
}

.settings-nav-title {
  font-size: 13px;
  font-weight: 600;
  padding: 0 16px 14px;
  color: var(--text-primary);
  display: flex;
  align-items: center;
  gap: 8px;
  border-bottom: 1px solid var(--border-subtle);
  margin-bottom: 8px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
  font-size: 12.5px;
  color: var(--text-secondary);
  cursor: pointer;
  border-left: 2px solid transparent;
  transition: all var(--transition-fast);
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
  font-size: 14px;
  width: 18px;
  text-align: center;
}

.settings-body {
  flex: 1;
  padding: 20px 24px;
  overflow-y: auto;
}

.settings-section {
  margin-bottom: 24px;
}

.settings-section-title {
  font-size: 14px;
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
  font-size: 12.5px;
  color: var(--text-primary);
  flex: 1;
}

.setting-desc {
  font-size: 11px;
  color: var(--text-tertiary);
  margin-top: 2px;
}

.toggle {
  width: 36px;
  height: 20px;
  background: var(--bg-active);
  border-radius: 10px;
  position: relative;
  cursor: pointer;
  transition: background var(--transition-fast);
  flex-shrink: 0;
}

.toggle.on {
  background: var(--accent);
}

.toggle::after {
  content: "";
  position: absolute;
  width: 16px;
  height: 16px;
  background: white;
  border-radius: 50%;
  top: 2px;
  left: 2px;
  transition: transform var(--transition-fast);
}

.toggle.on::after {
  transform: translateX(16px);
}

.shortcut-input {
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 5px 10px;
  font-size: 11.5px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  min-width: 120px;
  text-align: center;
  transition: background var(--transition-smooth), border-color var(--transition-smooth);
}

.kbd-display {
  background: var(--bg-active);
  padding: 3px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  white-space: nowrap;
}

.slider-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.slider-value {
  font-size: 12px;
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
  font-size: 11px;
  font-weight: 500;
  color: var(--text-secondary);
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
  font-size: 28px;
  line-height: 1;
}

.mode-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.mode-desc {
  font-size: 11px;
  color: var(--text-tertiary);
  line-height: 1.4;
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
  font-size: 14px;
}

.ignore-name {
  flex: 1;
  font-size: 12px;
  color: var(--text-secondary);
}

.ignore-remove {
  font-size: 12px;
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
  font-size: 12px;
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
  font-size: 11.5px;
  font-weight: 500;
  cursor: pointer;
  transition: background var(--transition-fast);
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
  font-size: 24px;
  font-weight: 700;
  line-height: 1;
}

.stats-value.accent { color: var(--accent); }
.stats-value.success { color: var(--success); }
.stats-value.warning { color: var(--warning); }
.stats-value.sensitive { color: var(--sensitive); }

.stats-label {
  margin-top: 6px;
  font-size: 11px;
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
  font-size: 11.5px;
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

.status-line {
  margin: 0 0 12px;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 11.5px;
}

.status-line.success {
  background: var(--success-soft);
  color: var(--success);
}

/* About */
.about-content {
  text-align: center;
  padding: 20px;
}

.about-logo {
  font-size: 48px;
  margin-bottom: 12px;
}

.about-name {
  font-size: 22px;
  font-weight: 700;
  margin-bottom: 4px;
}

.about-version {
  font-size: 12px;
  color: var(--accent);
  font-family: var(--font-mono);
  margin-bottom: 8px;
}

.about-desc {
  font-size: 12px;
  color: var(--text-tertiary);
}
</style>
