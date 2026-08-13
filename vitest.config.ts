import { mergeConfig } from "vite";
import { defineConfig } from "vitest/config";
import viteConfig from "./vite.config";

// Reuse the vite config (plugins + the `@` alias) so a change to either only
// needs to happen once — previously both files duplicated `vue()` and the
// alias, and forgetting one made tests diverge from the build.
export default defineConfig(
  mergeConfig(viteConfig, {
    test: {
      environment: "jsdom",
      // No globals: every spec imports { describe, it, ... } from "vitest" explicitly.
      setupFiles: ["./src/test/setup.ts"],
      include: ["src/**/*.{test,spec}.ts"],
    },
  }),
);
