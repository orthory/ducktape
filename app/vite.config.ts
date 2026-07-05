// defineConfig comes from vitest/config (not vite) so the `test` block below
// is typed — vitest 4 dropped the `/// <reference types="vitest" />` module
// augmentation. at runtime it is vite's own defineConfig, re-exported.
import { defineConfig } from "vitest/config";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";

// Tauri's webview loads a FIXED devUrl (tauri.conf.json), so vite must bind that
// exact port — a drifted port would leave the native window loading whatever else
// sits on the default (e.g. another Tauri app's dev server). Pin it strictly and
// fail loudly instead. 1430 is ducktape's own port; override with
// DUCKTAPE_TAURI_DEV_PORT (keep tauri.conf.json's devUrl in sync).
const explicitDevPort = process.env.DUCKTAPE_TAURI_DEV_PORT;
const devPort = Number(explicitDevPort || 1430);
const strictPort = true;

export default defineConfig({
  plugins: [tailwindcss(), react()],
  clearScreen: false,
  server: {
    port: devPort,
    strictPort,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
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
