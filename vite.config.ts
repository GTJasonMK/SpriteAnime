import { defineConfig } from "vite";

const tauriHost = process.env.TAURI_DEV_HOST;
const host = tauriHost || "127.0.0.1";
const port = parseInt(process.env.VITE_PORT || "1420", 10);
const hmrPort = port + 1;

export default defineConfig({
  clearScreen: false,
  optimizeDeps: {
    include: ["@tauri-apps/api/core"],
  },
  server: {
    port,
    strictPort: true,
    host,
    hmr: tauriHost
      ? {
          protocol: "ws",
          host: tauriHost,
          port: hmrPort,
        }
      : undefined,
    warmup: {
      clientFiles: ["./src/main.ts"],
    },
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
