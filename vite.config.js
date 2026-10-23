import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

/**
 * Dev only. A store module (`src/lib/state/*.svelte.ts`) holds the session
 * state every component of a page shares. A hot update of one gave it a new
 * `?t=` URL, but components transformed earlier kept importing the old one:
 * two copies, and part of a page read an empty store (the Ships loadout bar
 * showed "Stock" beside cards showing the real loadout). A change to a store
 * now drops the cached transform of every module importing it, directly or
 * not, and reloads the page, so all of them import the same copy.
 *
 * @returns {import("vite").Plugin}
 */
function storesReloadWhole() {
  return {
    name: "starlume:stores-reload-whole",
    apply: "serve",
    handleHotUpdate({ file, server, modules }) {
      if (!/\/src\/lib\/state\/[^/]+\.svelte\.ts$/.test(file.replaceAll("\\", "/"))) return;
      /** @type {Set<import("vite").ModuleNode>} */
      const seen = new Set();
      /** @param {import("vite").ModuleNode} m */
      const drop = (m) => {
        if (seen.has(m)) return;
        seen.add(m);
        server.moduleGraph.invalidateModule(m);
        m.importers.forEach(drop);
      };
      modules.forEach(drop);
      server.ws.send({ type: "full-reload" });
      return [];
    },
  };
}

export default defineConfig(async () => ({
  plugins: [storesReloadWhole(), tailwindcss(), sveltekit()],

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
