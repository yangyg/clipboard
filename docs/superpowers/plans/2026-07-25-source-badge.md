# Source Badge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the tiny source color-dot with a shared 14×14 letter avatar + short name in the list and preview, without changing DB/IPC.

**Architecture:** Pure frontend. `sourceBadge.ts` owns short name / initial / stable color; `SourceBadge.vue` renders avatar + label (optional `labelHtml` for search highlight, optional `iconSrc` reserved for later). `RecordList` and `PreviewPane` consume the component and delete duplicated dot logic.

**Tech Stack:** Vue 3 + TypeScript, Vitest, existing `highlightSearchHtml` / `escapeHtml`.

**Spec:** `docs/superpowers/specs/2026-07-25-source-badge-design.md`

---

## File map

| File | Responsibility |
|------|----------------|
| `src/utils/sourceBadge.ts` | Short name, initial, color, `resolveSourceBadge` |
| `src/utils/sourceBadge.spec.ts` | Unit tests for util |
| `src/components/SourceBadge.vue` | Avatar + label UI |
| `src/components/RecordList.vue` | Use `SourceBadge`; remove `source-dot` / local palette |
| `src/components/PreviewPane.vue` | Use `SourceBadge` in meta line |

---

### Task 1: `sourceBadge` util (TDD)

**Files:**
- Create: `src/utils/sourceBadge.spec.ts`
- Create: `src/utils/sourceBadge.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/utils/sourceBadge.spec.ts`:

```ts
import { describe, it, expect } from "vitest";
import {
  SOURCE_AVATAR_PALETTE,
  resolveSourceBadge,
  sourceAvatarColor,
  sourceInitial,
  sourceShortName,
} from "./sourceBadge";

describe("sourceBadge", () => {
  it("maps empty source to 系统剪贴板 / 剪 / gray", () => {
    expect(sourceShortName("")).toBe("系统剪贴板");
    expect(sourceShortName("   ")).toBe("系统剪贴板");
    expect(sourceInitial("系统剪贴板", "")).toBe("剪");
    expect(sourceAvatarColor("")).toBe("var(--text-tertiary)");
    const badge = resolveSourceBadge("");
    expect(badge).toEqual({
      label: "系统剪贴板",
      initial: "剪",
      color: "var(--text-tertiary)",
    });
  });

  it("strips path and .exe for short name", () => {
    expect(sourceShortName("C:\\\\Program Files\\\\App\\\\msedge.exe")).toBe("msedge");
    expect(sourceShortName("/usr/bin/WorkBuddy")).toBe("WorkBuddy");
  });

  it("takes first latin/digit uppercase as initial", () => {
    expect(sourceInitial("msedge", "msedge")).toBe("M");
    expect(sourceInitial("WorkBuddy", "WorkBuddy")).toBe("W");
    expect(sourceInitial("应用App", "应用App")).toBe("A");
  });

  it("takes first character for non-latin short names", () => {
    expect(sourceInitial("微信", "微信")).toBe("微");
  });

  it("hashes the same source_app to the same palette color", () => {
    const a = sourceAvatarColor("msedge");
    const b = sourceAvatarColor("msedge");
    expect(a).toBe(b);
    expect(SOURCE_AVATAR_PALETTE).toContain(a);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/utils/sourceBadge.spec.ts`

Expected: FAIL (module not found / exports missing).

- [ ] **Step 3: Implement `src/utils/sourceBadge.ts`**

```ts
/** Stable colors for source letter avatars (same palette as the old list source-dot). */
export const SOURCE_AVATAR_PALETTE = [
  "#3b82f6",
  "#34d399",
  "#fbbf24",
  "#f87171",
  "#38bdf8",
  "#a78bfa",
  "#fb923c",
  "#94a3b8",
] as const;

const EMPTY_COLOR = "var(--text-tertiary)";

export function sourceShortName(sourceApp: string): string {
  const raw = (sourceApp || "").trim();
  if (!raw) return "系统剪贴板";
  const base = raw.replace(/^.*[/\\]/, "").replace(/\.exe$/i, "");
  return base || raw;
}

export function sourceInitial(shortName: string, sourceApp: string): string {
  if (!(sourceApp || "").trim()) return "剪";
  const latin = shortName.match(/[A-Za-z0-9]/);
  if (latin) return latin[0].toUpperCase();
  const first = [...shortName][0];
  return first || "剪";
}

export function sourceAvatarColor(sourceApp: string): string {
  const s = (sourceApp || "").trim();
  if (!s) return EMPTY_COLOR;
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
  return SOURCE_AVATAR_PALETTE[h % SOURCE_AVATAR_PALETTE.length];
}

export function resolveSourceBadge(sourceApp: string): {
  label: string;
  initial: string;
  color: string;
} {
  const label = sourceShortName(sourceApp);
  return {
    label,
    initial: sourceInitial(label, sourceApp),
    color: sourceAvatarColor(sourceApp),
  };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/utils/sourceBadge.spec.ts`

Expected: PASS (all 5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/utils/sourceBadge.ts src/utils/sourceBadge.spec.ts
git commit -m "feat: 添加来源短名与首字母色块工具函数"
```

---

### Task 2: `SourceBadge.vue`

**Files:**
- Create: `src/components/SourceBadge.vue`

- [ ] **Step 1: Create the component**

```vue
<template>
  <span class="source-badge" :title="resolvedTitle">
    <img
      v-if="iconSrc"
      class="source-avatar source-avatar--img"
      :src="iconSrc"
      alt=""
      aria-hidden="true"
    />
    <span
      v-else
      class="source-avatar"
      :style="{ background: badge.color }"
      aria-hidden="true"
    >{{ badge.initial }}</span>
    <span
      v-if="labelHtml != null && labelHtml !== ''"
      class="source-label"
      v-html="labelHtml"
    />
    <span v-else class="source-label">{{ badge.label }}</span>
  </span>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { resolveSourceBadge } from "../utils/sourceBadge";

const props = defineProps<{
  sourceApp: string;
  /** Full tooltip; defaults to `来源：{raw or 系统剪贴板}`. */
  title?: string;
  /** Pre-highlighted / escaped HTML for the label (search). */
  labelHtml?: string;
  /** Reserved for future real app icons. */
  iconSrc?: string;
}>();

const badge = computed(() => resolveSourceBadge(props.sourceApp ?? ""));

const resolvedTitle = computed(() => {
  if (props.title != null && props.title !== "") return props.title;
  const raw = (props.sourceApp || "").trim();
  return `来源：${raw || "系统剪贴板"}`;
});
</script>

<style scoped>
.source-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  max-width: 100%;
  vertical-align: middle;
}

.source-avatar {
  box-sizing: border-box;
  flex-shrink: 0;
  width: 14px;
  height: 14px;
  border-radius: 4px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 9px;
  font-weight: 600;
  line-height: 1;
  color: #fff;
  user-select: none;
}

.source-avatar--img {
  object-fit: cover;
  padding: 0;
  background: transparent;
}

.source-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
```

- [ ] **Step 2: Smoke-check types**

Run: `npx vue-tsc --noEmit`

Expected: no new errors from `SourceBadge.vue`.

- [ ] **Step 3: Commit**

```bash
git add src/components/SourceBadge.vue
git commit -m "feat: 添加 SourceBadge 来源色块组件"
```

---

### Task 3: Wire `RecordList.vue`

**Files:**
- Modify: `src/components/RecordList.vue`

- [ ] **Step 1: Import component + util; remove local source helpers**

In `<script setup>`:

1. Add: `import SourceBadge from "./SourceBadge.vue";`
2. Add: `import { sourceShortName } from "../utils/sourceBadge";`
3. Delete: `sourceLabel`, `SOURCE_DOT_PALETTE`, `sourceDotColor`, `sourceHtml`.
4. Keep search highlight via:

```ts
function sourceLabelHtml(record: ClipboardRecord): string | undefined {
  const q = clipboardStore.searchQuery.trim();
  if (!q) return undefined;
  return highlightSearchHtml(sourceShortName(record.source_app), q);
}
```

(Keep existing `highlightSearchHtml` import; drop `escapeHtml` from source path if unused elsewhere in the file — only remove if no other call sites.)

- [ ] **Step 2: Replace meta source markup**

Replace:

```vue
<span class="record-source" v-html="sourceHtml(item.record!)"></span>
```

with:

```vue
<span class="record-source">
  <SourceBadge
    :source-app="item.record!.source_app"
    :label-html="sourceLabelHtml(item.record!)"
  />
</span>
```

- [ ] **Step 3: Replace CSS**

Delete `.source-dot { ... }`.

Keep `.record-source` as a layout wrapper (inline-flex / max-width / ellipsis). Example:

```css
.record-source {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  max-width: 160px;
}
```

(Grid overrides for `.view-grid .record-source` can stay; they only need width/ellipsis constraints.)

- [ ] **Step 4: Verify**

Run:

```bash
npx vitest run src/utils/sourceBadge.spec.ts
npx vue-tsc --noEmit
```

Expected: tests PASS; tsc clean for touched files.

Manual (dev): list shows 14px letter tiles; search still highlights source short name.

- [ ] **Step 5: Commit**

```bash
git add src/components/RecordList.vue
git commit -m "feat: 列表来源改用 SourceBadge 首字母色块"
```

---

### Task 4: Wire `PreviewPane.vue`

**Files:**
- Modify: `src/components/PreviewPane.vue`

- [ ] **Step 1: Import and replace source span**

Add: `import SourceBadge from "./SourceBadge.vue";`

Replace the source meta span:

```vue
<span :title="`来源：${record.source_app || '系统剪贴板'}`">{{ record.source_app || '系统剪贴板' }}</span>
```

with:

```vue
<SourceBadge :source-app="record.source_app" />
```

(`SourceBadge` default `title` already uses `来源：…`. Preview previously showed raw `source_app` path; short name matches list and the spec.)

- [ ] **Step 2: Verify**

Run: `npx vue-tsc --noEmit`

Manual: open a record — meta line shows letter avatar + short name next to time.

- [ ] **Step 3: Commit**

```bash
git add src/components/PreviewPane.vue
git commit -m "feat: 详情来源改用 SourceBadge 首字母色块"
```

---

### Task 5: Doc touch-up (optional but preferred)

**Files:**
- Modify: `CLAUDE.md` (one bullet under Theming or List UI)

- [ ] **Step 1: Add a short design note**

Under Key Design Decisions (near list / theming), add:

```markdown
- **Source badge:** List + preview show a 14px letter avatar + short name via `SourceBadge` / `sourceBadge.ts`. Empty source →「系统剪贴板」/「剪」/ gray. Real exe icons later via optional `iconSrc`.
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: 记录来源 SourceBadge 约定"
```

---

## Spec coverage check

| Spec requirement | Task |
|------------------|------|
| List meta badge | Task 3 |
| Preview meta badge | Task 4 |
| Util + component | Tasks 1–2 |
| Util unit tests | Task 1 |
| 14×14 letter, short name, empty→剪/灰 | Tasks 1–2 |
| Search highlight via `labelHtml` | Tasks 2–3 |
| `iconSrc` reserved | Task 2 |
| No DB/IPC change | (none) |
| No row-height change | Task 2 (14px inside meta) |

## Placeholder / consistency check

- Function names match spec: `sourceShortName`, `sourceInitial`, `sourceAvatarColor`, `resolveSourceBadge`.
- Palette exported as `SOURCE_AVATAR_PALETTE` for tests.
- Preview switches from raw path text to short name (intentional, per spec).
