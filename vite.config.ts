import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [svelte()],

  // Vite options tailored for Tauri development and only applied in
  // `tauri dev` or `tauri build`.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  // Two Vite entry points — see implementation plan §6. The dashboard
  // is the main Svelte app; `overlay.html` ships a tiny bundle that
  // boots instantly for break overlays.
  build: {
    rollupOptions: {
      input: {
        main: "index.html",
        overlay: "overlay.html",
      },
    },
    target:
      process.env.TAURI_ENV_PLATFORM == "windows" ? "chrome105" : "safari13",
    // Don't minify for debug builds
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    // Produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
}));
