/// <reference types="vitest" />
import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";

// Tauri dev injects a pre-resolved free port and pins it strictly so its devUrl
// matches; bare `vite` auto-finds a port instead of dying when 1420 is taken.
// @ts-expect-error process is a nodejs global
const explicitDevPort = process.env.DUCKTAPE_TAURI_DEV_PORT;
const devPort = Number(explicitDevPort || 1420);
const strictPort = Boolean(explicitDevPort);

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
  },
});
