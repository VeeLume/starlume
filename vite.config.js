import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig, loadEnv } from "vite";

const host = process.env.TAURI_DEV_HOST;

/**
 * The dev server's port, 1445 unless `STARLUME_DEV_PORT` says otherwise (the
 * environment, or an untracked `.env.local`). A second checkout, such as the
 * App's worktree, takes its own so both run side by side. Tauri's `devUrl`
 * cannot read the environment: `tools/dev.mjs` (`just dev-here`) sets it.
 *
 * @param {string} mode
 */
function devPort(mode) {
  const fromFile = loadEnv(mode, process.cwd(), "STARLUME_").STARLUME_DEV_PORT;
  return Number(process.env.STARLUME_DEV_PORT || fromFile) || 1445;
}

/**
 * Dev only. A store module (`src/lib/state/*.svelte.ts`) holds the session
 * state every component of a page shares. A hot update of one gave it a new
 * `?t=` URL, but components transformed earlier kept importing the old one:
 * two copies, and part of a page read an empty store (the Ships loadout bar
 * showed "Stock" beside cards showing the real loadout). A change to a store
 * now drops the cached transform of every module importing it, directly or
 * not, and reloads the page, so all of them import the same copy. So does
 * a change to anything a store imports (a regenerated `bindings.ts` hot-
 * updates every store above it, the same split: the Ships cards lost their
 * pictures and prices).
 *
 * @returns {import("vite").Plugin}
 */
function storesReloadWhole() {
  /** @param {import("vite").ModuleNode} m */
  const isStore = (m) => /\/src\/lib\/state\/[^/]+\.svelte\.ts$/.test((m.file ?? "").replaceAll("\\", "/"));
  return {
    name: "starlume:stores-reload-whole",
    apply: "serve",
    handleHotUpdate({ server, modules }) {
      /** @type {Set<import("vite").ModuleNode>} */
      const above = new Set();
      /** @param {import("vite").ModuleNode} m */
      const walk = (m) => {
        if (above.has(m)) return;
        above.add(m);
        m.importers.forEach(walk);
      };
      modules.forEach(walk);
      if (![...above].some(isStore)) return;
      above.forEach((m) => server.moduleGraph.invalidateModule(m));
      server.ws.send({ type: "full-reload" });
      return [];
    },
  };
}

export default defineConfig(async ({ mode }) => ({
  plugins: [storesReloadWhole(), tailwindcss(), sveltekit()],

  // @veelume/ui ships SOURCE (.svelte / .svelte.ts rune modules). The dev
  // dependency pre-bundler (esbuild) cannot parse those — it mangles the
  // module text and fails with js_parse_error — so the kit must reach the
  // Svelte plugin uncompiled. Prod builds were never affected.
  // Excluding a package also un-bundles its dependencies, so the kit's own
  // copies are listed explicitly (the kit's consumer rule, 0.1.1) — else
  // bits-ui's styled components miss their virtual CSS and dev dies.
  optimizeDeps: {
    exclude: ["@veelume/ui", "@veelume/sc-render"],
    include: [
      "@veelume/sc-render > three",
      "@veelume/sc-render > three/addons/controls/OrbitControls.js",
      "@veelume/sc-render > three/addons/environments/RoomEnvironment.js",
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
    port: devPort(mode),
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: devPort(mode) + 1,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
}));
