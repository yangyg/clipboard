import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL(".", import.meta.url));

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  resolve: {
    alias: {
      "@": resolve(projectRoot, "src"),
    },
  },
  build: {
    target: "chrome120",
    rollupOptions: {
      input: {
        main: resolve(projectRoot, "index.html"),
        trayMenu: resolve(projectRoot, "tray-menu.html"),
      },
      output: {
        manualChunks(id) {
          if (!id.includes("node_modules")) return undefined;
          if (/[\\/]node_modules\/(vue|@vue|vue-i18n|pinia)[\\/]/.test(id)) {
            return "vendor-vue";
          }
          if (id.includes("lucide-vue-next") || id.includes("@sketchyicons")) {
            return "vendor-icons";
          }
          if (id.includes("dompurify")) return "vendor-sanitize";
          return "vendor";
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
