import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    strictPort: true,
    port: 1420,
    watch: {
      // Keep the dev-server watcher off the multi-gigabyte third-party trees
      // (`vendor/whisper.cpp`, `vendor/python-cmake`). Without this, chokidar
      // recursively scans them on startup and pegs the CPU, leaving the
      // webview stuck on a white page because `.tsx` requests never get
      // served.
      ignored: [
        "**/vendor/**",
        "**/src-tauri/target/**",
        "**/dist/**",
        "**/node_modules/**"
      ]
    },
    fs: {
      // Refuse to serve files from the vendor tree even if a stray import
      // ever asks for one.
      deny: ["vendor"]
    }
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: Boolean(process.env.TAURI_DEBUG)
  }
});
