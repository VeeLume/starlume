import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [sveltekit()],

  // Vite options for Tauri development
  clearScreen: false,
  server: {
    port: 1445,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1446,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
}));
