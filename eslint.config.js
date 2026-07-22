import js from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";
import globals from "globals";

export default tseslint.config(
  {
    // Build artifacts, generated output, and non-source directories.
    ignores: [
      "dist/**",
      "node_modules/**",
      "src-tauri/**",
      "graphify-out/**",
      ".qoder/**",
      "prototype/**",
      "scripts/**",
      "public/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/essential"],
  {
    // Parse <script lang="ts"> blocks in Vue SFCs with the TS parser.
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },
  {
    files: ["**/*.{ts,vue}"],
    languageOptions: {
      globals: {
        ...globals.browser,
      },
    },
    rules: {
      // Single-word component names (App, SideBar, etc.) are intentional here.
      "vue/multi-word-component-names": "off",
      // Type-narrowing casts are used deliberately in a few store getters.
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },
  {
    files: ["**/*.spec.ts", "src/test/**/*.ts"],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
  },
  {
    // Ambient module shims (e.g. the *.vue declaration) rely on `{}` generics.
    files: ["**/*.d.ts"],
    rules: {
      "@typescript-eslint/no-empty-object-type": "off",
    },
  },
);
