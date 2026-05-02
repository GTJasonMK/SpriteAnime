import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;
const port = parseInt(process.env.VITE_PORT || "1420", 10);
const hmrPort = port + 1;

export default defineConfig({
  clearScreen: false,
  server: {
    port,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: hmrPort,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
