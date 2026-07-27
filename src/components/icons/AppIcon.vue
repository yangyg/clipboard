<script setup lang="ts">
import { computed } from "vue";
import type { Component } from "vue";
import {
  ClipboardList,
  Search,
  Settings,
  Star,
  Pin,
  Trash2,
  SquareCheck,
  X,
  CornerDownLeft,
  Type,
  Link2,
  Image,
  FileText,
  Code2,
  Tag,
  Plus,
  RotateCcw,
  AlertTriangle,
  Keyboard,
  History,
  Palette,
  Shield,
  BarChart3,
  Package,
  Info,
  Moon,
  Sun,
  Monitor,
  Copy,
  Check,
  Pause,
  Play,
  PanelTop,
  AppWindow,
  Circle,
  Pencil,
  ArrowLeft,
  HelpCircle,
  List,
  LayoutGrid,
  Sparkles,
  Cloud,
  CloudUpload,
  CloudDownload,
  RefreshCw,
  Zap,
  ArrowUp,
} from "lucide-vue-next";

const ICONS = {
  clipboard: ClipboardList,
  search: Search,
  settings: Settings,
  star: Star,
  pin: Pin,
  trash: Trash2,
  batch: SquareCheck,
  close: X,
  back: ArrowLeft,
  paste: CornerDownLeft,
  type: Type,
  link: Link2,
  image: Image,
  file: FileText,
  code: Code2,
  tag: Tag,
  plus: Plus,
  restore: RotateCcw,
  warning: AlertTriangle,
  keyboard: Keyboard,
  history: History,
  palette: Palette,
  shield: Shield,
  stats: BarChart3,
  package: Package,
  info: Info,
  moon: Moon,
  sun: Sun,
  monitor: Monitor,
  copy: Copy,
  check: Check,
  pause: Pause,
  play: Play,
  panel: PanelTop,
  window: AppWindow,
  circle: Circle,
  edit: Pencil,
  help: HelpCircle,
  list: List,
  grid: LayoutGrid,
  sparkles: Sparkles,
  cloud: Cloud,
  cloudUpload: CloudUpload,
  cloudDownload: CloudDownload,
  refresh: RefreshCw,
  zap: Zap,
  arrowUp: ArrowUp,
} as const;

export type AppIconName = keyof typeof ICONS;

const props = withDefaults(
  defineProps<{
    name: AppIconName;
    size?: number | string;
    strokeWidth?: number | string;
    /** Use "currentColor" for solid filled icons (e.g. active star). */
    fill?: string;
  }>(),
  {
    size: 16,
    strokeWidth: 2,
    fill: "none",
  }
);

const icon = computed<Component>(() => ICONS[props.name]);
</script>

<template>
  <component
    :is="icon"
    class="app-icon"
    :class="{ filled: fill !== 'none' }"
    :size="size"
    :stroke-width="strokeWidth"
    :fill="fill"
    aria-hidden="true"
  />
</template>

<style scoped>
.app-icon {
  display: block;
  flex-shrink: 0;
  line-height: 0;
}
</style>
