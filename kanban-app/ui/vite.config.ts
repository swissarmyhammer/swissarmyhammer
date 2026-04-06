/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { playwright } from "@vitest/browser-playwright";
import path from "path";
import * as integrationCommands from "./src/test/integration-commands.ts";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2021",
    minify: !process.env.TAURI_DEBUG ? "oxc" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
  test: {
    globals: true,
    projects: [
      // Node-side tests — for tests that need filesystem access (fs, path,
      // __dirname). Uses happy-dom instead of jsdom to avoid the
      // ERR_REQUIRE_ASYNC_MODULE error from cssstyle's ESM dependency.
      {
        plugins: [react()],
        resolve: {
          alias: { "@": path.resolve(__dirname, "./src") },
        },
        test: {
          name: "unit",
          include: ["src/**/*.node.test.{ts,tsx}"],
          environment: "happy-dom",
          globals: true,
          setupFiles: ["src/test/setup.ts"],
        },
      },
      // Browser-side tests — the primary test project. Runs in real Chromium
      // via Playwright, eliminating jsdom entirely.
      {
        plugins: [react()],
        resolve: {
          alias: {
            "@": path.resolve(__dirname, "./src"),
            "@tauri-apps/plugin-dialog": path.resolve(
              __dirname,
              "./src/test/stubs/tauri-plugin-dialog.ts",
            ),
          },
        },
        optimizeDeps: {
          entries: ["src/**/*.test.{ts,tsx}"],
          exclude: [
            "@tauri-apps/api",
            "@tauri-apps/plugin-dialog",
            "@tauri-apps/plugin-log",
          ],
        },
        test: {
          name: "browser",
          include: ["src/**/*.test.{ts,tsx}"],
          exclude: ["src/**/*.node.test.{ts,tsx}"],
          globals: true,
          setupFiles: ["src/test/setup.ts"],
          browser: {
            enabled: true,
            provider: playwright(),
            headless: true,
            instances: [{ browser: "chromium" }],
            commands: integrationCommands,
          },
        },
      },
    ],
  },
});
