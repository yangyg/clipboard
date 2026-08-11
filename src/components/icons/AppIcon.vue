<script setup lang="ts">
import { computed } from "vue";
import type { Component as VueComponent } from "vue";
import { useHanddrawnTheme } from "../../composables/useHanddrawnTheme";
import { usePixelTheme } from "../../composables/usePixelTheme";
import { PIXEL_ICONS } from "./pixelIcons";
// Clean line icons — every theme except hand-drawn / pixel.
import {
  ClipboardList,
  Search,
  Settings,
  Settings2,
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
  PencilOff,
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
  Component,
  Eye,
  EyeOff,
} from "lucide-vue-next";
// Hand-drawn icons (derived from Lucide, same names) — hand-drawn theme only.
import {
  ClipboardList as SketchyClipboardList,
  Search as SketchySearch,
  Settings as SketchySettings,
  Settings2 as SketchySettings2,
  Star as SketchyStar,
  Pin as SketchyPin,
  Trash2 as SketchyTrash2,
  SquareCheck as SketchySquareCheck,
  X as SketchyX,
  CornerDownLeft as SketchyCornerDownLeft,
  Type as SketchyType,
  Link2 as SketchyLink2,
  Image as SketchyImage,
  FileText as SketchyFileText,
  Code2 as SketchyCode2,
  Tag as SketchyTag,
  Plus as SketchyPlus,
  RotateCcw as SketchyRotateCcw,
  AlertTriangle as SketchyAlertTriangle,
  Keyboard as SketchyKeyboard,
  History as SketchyHistory,
  Palette as SketchyPalette,
  Shield as SketchyShield,
  BarChart3 as SketchyBarChart3,
  Package as SketchyPackage,
  Info as SketchyInfo,
  Moon as SketchyMoon,
  Sun as SketchySun,
  Monitor as SketchyMonitor,
  Copy as SketchyCopy,
  Check as SketchyCheck,
  Pause as SketchyPause,
  Play as SketchyPlay,
  PanelTop as SketchyPanelTop,
  AppWindow as SketchyAppWindow,
  Circle as SketchyCircle,
  Pencil as SketchyPencil,
  PencilOff as SketchyPencilOff,
  ArrowLeft as SketchyArrowLeft,
  HelpCircle as SketchyHelpCircle,
  List as SketchyList,
  LayoutGrid as SketchyLayoutGrid,
  Sparkles as SketchySparkles,
  Cloud as SketchyCloud,
  CloudUpload as SketchyCloudUpload,
  CloudDownload as SketchyCloudDownload,
  RefreshCw as SketchyRefreshCw,
  Zap as SketchyZap,
  ArrowUp as SketchyArrowUp,
  Component as SketchyComponent,
  Eye as SketchyEye,
  EyeOff as SketchyEyeOff,
} from "@sketchyicons/vue";

const LUCIDE_ICONS = {
  clipboard: ClipboardList,
  search: Search,
  settings: Settings,
  settings2: Settings2,
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
  pencilOff: PencilOff,
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
  component: Component,
  eye: Eye,
  eyeOff: EyeOff,
} as const;

const SKETCHY_ICONS = {
  clipboard: SketchyClipboardList,
  search: SketchySearch,
  settings: SketchySettings,
  settings2: SketchySettings2,
  star: SketchyStar,
  pin: SketchyPin,
  trash: SketchyTrash2,
  batch: SketchySquareCheck,
  close: SketchyX,
  back: SketchyArrowLeft,
  paste: SketchyCornerDownLeft,
  type: SketchyType,
  link: SketchyLink2,
  image: SketchyImage,
  file: SketchyFileText,
  code: SketchyCode2,
  tag: SketchyTag,
  plus: SketchyPlus,
  restore: SketchyRotateCcw,
  warning: SketchyAlertTriangle,
  keyboard: SketchyKeyboard,
  history: SketchyHistory,
  palette: SketchyPalette,
  shield: SketchyShield,
  stats: SketchyBarChart3,
  package: SketchyPackage,
  info: SketchyInfo,
  moon: SketchyMoon,
  sun: SketchySun,
  monitor: SketchyMonitor,
  copy: SketchyCopy,
  check: SketchyCheck,
  pause: SketchyPause,
  play: SketchyPlay,
  panel: SketchyPanelTop,
  window: SketchyAppWindow,
  circle: SketchyCircle,
  edit: SketchyPencil,
  pencilOff: SketchyPencilOff,
  help: SketchyHelpCircle,
  list: SketchyList,
  grid: SketchyLayoutGrid,
  sparkles: SketchySparkles,
  cloud: SketchyCloud,
  cloudUpload: SketchyCloudUpload,
  cloudDownload: SketchyCloudDownload,
  refresh: SketchyRefreshCw,
  zap: SketchyZap,
  arrowUp: SketchyArrowUp,
  component: SketchyComponent,
  eye: SketchyEye,
  eyeOff: SketchyEyeOff,
} as const;

export type AppIconName = keyof typeof LUCIDE_ICONS;

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

const isHanddrawn = useHanddrawnTheme();
const isPixel = usePixelTheme();

const icon = computed<VueComponent>(() => {
  // Hand-drawn wins outright — theme classes are mutually exclusive, so the
  // pixel branch only ever applies outside hand-drawn themes. The pixel set is
  // partial; any unmapped name falls back to the clean Lucide icon.
  if (isHanddrawn.value) {
    return SKETCHY_ICONS[props.name];
  }
  if (isPixel.value) {
    return PIXEL_ICONS[props.name] ?? LUCIDE_ICONS[props.name];
  }
  return LUCIDE_ICONS[props.name];
});
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
