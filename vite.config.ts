import { fileURLToPath, URL } from "node:url";

import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2022", "chrome105", "safari13"],
    sourcemap: true,
  },
  test: {
    environment: "happy-dom",
    setupFiles: ["./tests/frontend/setup.ts"],
    include: ["tests/frontend/**/*.spec.ts"],
    coverage: {
      reporter: ["text", "html"],
    },
  },
});
