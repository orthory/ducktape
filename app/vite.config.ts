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
    // The jsdom UI suite runs many files across forks; on a loaded/oversubscribed
    // runner a `waitFor` can jitter well past the 1s default and flake. Give it
    // real headroom (asyncUtilTimeout is raised to 10s in test-setup.ts) while
    // keeping testTimeout strictly above it, so a genuine hang still fails as a
    // clean waitFor error rather than a bare test-timeout.
    testTimeout: 15_000,
    // Node ≥22 ships its own global localStorage/sessionStorage; vitest's jsdom
    // env skips globals that already exist, so Node's non-functional stub (no
    // --localstorage-file) shadows jsdom's real Storage and every access throws.
    // Drop Node's webstorage in the workers so jsdom's Storage wins.
    execArgv: ["--no-experimental-webstorage"],
  },
});
