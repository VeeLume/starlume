import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [tailwindcss(), sveltekit()],

  // @veelume/ui ships SOURCE (.svelte / .svelte.ts rune modules). The dev
  // dependency pre-bundler (esbuild) cannot parse those — it mangles the
  // module text and fails with js_parse_error — so the kit must reach the
  // Svelte plugin uncompiled. Prod builds were never affected.
  optimizeDeps: {
    exclude: ["@veelume/ui"],
  },

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
