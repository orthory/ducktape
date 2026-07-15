// defineConfig comes from vitest/config (not vite) so the `test` block below
// is typed — vitest 4 dropped the `/// <reference types="vitest" />` module
// augmentation. at runtime it is vite's own defineConfig, re-exported.
import { defineConfig } from "vitest/config";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [tailwindcss(), react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  test: {
    environment: "jsdom",
    setupFiles: "./src/test-setup.ts",
    // Node ≥22 ships its own global localStorage/sessionStorage; vitest's jsdom
    // env skips globals that already exist, so Node's non-functional stub (no
    // --localstorage-file) shadows jsdom's real Storage and every access throws.
    // Drop Node's webstorage in the workers so jsdom's Storage wins.
    execArgv: ["--no-experimental-webstorage"],
  },
});
