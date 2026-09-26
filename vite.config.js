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
  // Excluding a package also un-bundles its dependencies, so the kit's own
  // copies are listed explicitly (the kit's consumer rule, 0.1.1) — else
  // bits-ui's styled components miss their virtual CSS and dev dies.
  optimizeDeps: {
    exclude: ["@veelume/ui"],
    include: [
      "@veelume/ui > bits-ui",
      "@veelume/ui > @floating-ui/dom",
      "@veelume/ui > @internationalized/date",
      "@veelume/ui > clsx",
      "@veelume/ui > tailwind-merge",
    ],
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
