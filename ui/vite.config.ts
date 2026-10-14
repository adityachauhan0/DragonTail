import { defineConfig } from "vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const uiRoot = path.dirname(fileURLToPath(import.meta.url));
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  root: uiRoot,
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
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: path.resolve(uiRoot, "index.html"),
        desktop: path.resolve(uiRoot, "desktop.html"),
      },
    },
  },
}));
