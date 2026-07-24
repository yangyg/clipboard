# First-run Onboarding Implementation Plan

> **Status:** Implemented on `feat/first-run-onboarding` (commit `7cfa954`). Spec: `docs/superpowers/specs/2026-07-24-onboarding-design.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a one-page welcome dialog on first install that explains shortcut → paste → tray, then persist `onboarding_completed` so it never shows again (upgrades skip via serde default).

**Architecture:** Add `onboarding_completed` to Settings (Rust + TS). New `WelcomeDialog.vue` wraps `BaseDialog`. `App.vue` opens it after `loadSettings` when the flag is false; dismiss marks complete and saves.

**Tech Stack:** Vue 3, Pinia settings store, Tauri `get_settings`/`save_settings`, existing `BaseDialog`.

**Spec:** `docs/superpowers/specs/2026-07-24-onboarding-design.md`

---

## File map

| File | Responsibility |
|------|----------------|
| `src-tauri/src/lib.rs` | Settings field + `default_onboarding_completed` + `Default` |
| `src/types.ts` | Settings type field |
| `src/stores/settings.ts` | DEFAULT_SETTINGS |
| `src/components/WelcomeDialog.vue` | Welcome UI |
| `src/App.vue` | Show after load; wire complete |
| `src-tauri/src/lib.rs` (tests) or small module test | Serde missing-field → true; Default → false |

---

### Task 1: Settings field + serde defaults (Rust)

**Files:**
- Modify: `src-tauri/src/lib.rs` (Settings struct, Default, helper fn)
- Add tests in `src-tauri/src/lib.rs` under `#[cfg(test)] mod settings_onboarding_tests`

- [ ] **Step 1: Write failing tests**

Near other helpers in `lib.rs` (or at end of file before/after Settings):

```rust
fn default_onboarding_completed() -> bool {
    true
}

#[cfg(test)]
mod settings_onboarding_tests {
    use super::Settings;

    #[test]
    fn default_settings_needs_onboarding() {
        assert!(!Settings::default().onboarding_completed);
    }

    #[test]
    fn missing_json_field_skips_onboarding_for_upgrades() {
        let json = r#"{"global_shortcut":"Ctrl+Shift+V","max_records":1000,"retention_days":30,"theme":"dark","panel_opacity":94,"panel_radius":20,"enable_blur":false,"enable_animation":true,"font_size":16,"app_mode":"floating","default_paste_mode":"original","auto_close_on_paste":true,"enable_sensitive_detection":true,"sensitive_auto_expire_seconds":600,"data_path":"","auto_start":false,"minimize_to_tray":true,"ignored_apps":[]}"#;
        let s: Settings = serde_json::from_str(json).expect("parse");
        assert!(s.onboarding_completed);
    }
}
```

(Place `default_onboarding_completed` next to other `default_*` helpers; tests will fail until the field exists.)

- [ ] **Step 2: Run tests — expect FAIL**

```bash
cargo test --manifest-path src-tauri/Cargo.toml settings_onboarding -- --nocapture
```

Expected: compile error / missing field

- [ ] **Step 3: Add field to Settings**

Find `default_enable_auto_tag` / `default_auto_tag_rules` and add nearby:

```rust
fn default_onboarding_completed() -> bool {
    true
}
```

In `Settings` struct, after auto_tag fields:

```rust
    #[serde(
        default = "default_onboarding_completed",
        rename = "onboarding_completed"
    )]
    pub onboarding_completed: bool,
```

In `Default::default()`:

```rust
            onboarding_completed: false,
```

- [ ] **Step 4: Run tests — expect PASS**

```bash
cargo test --manifest-path src-tauri/Cargo.toml settings_onboarding -- --nocapture
```

Expected: 2 passed

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add onboarding_completed setting with upgrade-safe default"
```

---

### Task 2: Frontend Settings type + defaults

**Files:**
- Modify: `src/types.ts`
- Modify: `src/stores/settings.ts`
- Modify: `src/stores/settings.spec.ts` (assert default false)

- [ ] **Step 1: Extend failing / update smoke test**

In `settings.spec.ts` `initializes with sensible defaults`:

```ts
expect(store.settings.onboarding_completed).toBe(false);
```

- [ ] **Step 2: Run test — expect FAIL** (property missing / undefined)

```bash
npm test -- src/stores/settings.spec.ts
```

- [ ] **Step 3: Add to `types.ts` Settings**

```ts
  /** False until first-run welcome is dismissed. */
  onboarding_completed: boolean;
```

In `settings.ts` `DEFAULT_SETTINGS`:

```ts
  onboarding_completed: false,
```

- [ ] **Step 4: Run test — expect PASS**

```bash
npm test -- src/stores/settings.spec.ts
```

- [ ] **Step 5: Commit**

```bash
git add src/types.ts src/stores/settings.ts src/stores/settings.spec.ts
git commit -m "feat: wire onboarding_completed in frontend settings"
```

---

### Task 3: WelcomeDialog component

**Files:**
- Create: `src/components/WelcomeDialog.vue`

- [ ] **Step 1: Create component**

```vue
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

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  padding: 12px 16px 16px;
}

.btn-confirm {
  /* match ConfirmDialog primary if global .btn-confirm exists; else: */
  padding: 8px 16px;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--accent, #6366f1);
  color: #fff;
  font-size: var(--text-md);
  cursor: pointer;
}
</style>
```

Check `ConfirmDialog` / global styles for `.dialog-footer` / `.btn-confirm` — prefer reusing global classes from `BaseDialog` consumers (copy footer markup from ConfirmDialog; ConfirmDialog relies on unscoped styles in BaseDialog.css or parent). Read `BaseDialog.vue` style section and Match ConfirmDialog: only add scoped steps styles; use same footer class names as ConfirmDialog without redefining if already global.

If `.btn-confirm` is only in ConfirmDialog scoped, duplicate the minimal button styles as above or lift is YAGNI — duplicate is OK for this task.

- [ ] **Step 2: Type-check**

```bash
npx vue-tsc --noEmit
```

Expected: pass (component unused yet is fine)

- [ ] **Step 3: Commit**

```bash
git add src/components/WelcomeDialog.vue
git commit -m "feat: add WelcomeDialog for first-run onboarding"
```

---

### Task 4: Wire App.vue

**Files:**
- Modify: `src/App.vue`

- [ ] **Step 1: Import and state**

```vue
    <ConfirmDialog />
    <WelcomeDialog
      :open="welcomeOpen"
      :shortcut="settings.global_shortcut"
      @complete="completeOnboarding"
    />
```

```ts
import WelcomeDialog from "./components/WelcomeDialog.vue";

const welcomeOpen = ref(false);

async function completeOnboarding() {
  if (!welcomeOpen.value) return;
  welcomeOpen.value = false;
  settingsStore.updateSetting("onboarding_completed", true);
}
```

Confirm `updateSetting` persists via existing debounced save (it does). If save is debounced only, call whatever flushes — read `settings.ts`: `updateSetting` triggers debounced save — OK for dismiss.

- [ ] **Step 2: After loadSettings in onMounted**

After `await settingsStore.loadSettings();`:

```ts
  if (!settings.value.onboarding_completed) {
    welcomeOpen.value = true;
  }
```

Place after loadSettings, before or after showPanel — both OK; prefer after `await showPanel()` so panel is visible under the dialog.

- [ ] **Step 3: Build**

```bash
npm run build
```

Expected: success

- [ ] **Step 4: Commit**

```bash
git add src/App.vue
git commit -m "feat: show welcome dialog on first launch"
```

---

### Task 5: Manual verification

- [ ] **Step 1:** Backup or rename `%LOCALAPPDATA%/ClipVault/clipvault.db` (or only clear `app_settings` row) for a clean first-run, or temporarily force `onboarding_completed: false` in DB.

- [ ] **Step 2:** `npm run tauri dev` — checklist from spec:
  - [ ] First launch shows dialog
  - [ ] Shortcut text matches settings
  - [ ] Overlay click does not close
  - [ ] 「开始使用」 closes; restart does not show
  - [ ] Esc completes and closes
  - [ ] Existing settings JSON without field does not show (upgrade path)

- [ ] **Step 3:** Fix issues if any; commit follow-ups

---

## Spec coverage

| Spec item | Task |
|-----------|------|
| `onboarding_completed` + upgrade-safe default | 1 |
| Frontend type/default | 2 |
| Welcome UI copy + BaseDialog | 3 |
| Trigger after loadSettings | 4 |
| Persist on complete / Esc | 3–4 (`@close` → complete) |
| Manual QA | 5 |

## Self-review

- No TBD placeholders  
- Esc handled via BaseDialog `@close` → `complete`  
- Overlay blocked via `close-on-overlay=false`  
- Upgrade users: Rust serde default `true`
