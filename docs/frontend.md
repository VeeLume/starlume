# Frontend rules — data lifecycle & styling

The SvelteKit SPA re-mounts a page component on every navigation. Without
discipline that means every tab switch refetches and re-spins — the opposite
of the resident-companion feel. These rules keep the app snappy; treat them
like the module rules in the README: invariants, not suggestions.

**Since 2026-08 the component system is [@veelume/ui](https://github.com/VeeLume/veelume-ui)**
(pinned by tag in `package.json`). The kit's own rulebook
(`packages/ui/CLAUDE.md` in that repo) is binding for everything it covers —
layers, the coupling contract, the surface pipeline, `derive` before
`filter`, URL-backed browse state. This document covers what stays
Starlume's: the tokens, the app-side conventions, and how the kit is wired
here.

## Data lifecycle

1. **Stores own data; pages render it.** Any dataset that outlives one visit
   (installs, statuses, catalog datasets, friends, identity) lives in a
   module store under `src/lib/state/*.svelte.ts`. Pages hold only ephemeral
   UI state (expansion, input focus, lazily fetched detail). A page that
   owns a long-lived dataset in local `$state` is a bug.

2. **Fetch-once, then stale-while-revalidate.** Store loaders return the
   cache when they have it and refresh in the background. A page renders
   cached data *synchronously* on mount — no spinner for data the app has
   already seen. Spinners (`Loading`, `Surface.List`'s own states) are for
   the genuine first load.

3. **Startup hydration.** The root layout kicks off every cheap load
   fire-and-forget at mount: settings → auth, SC scan, data statuses,
   catalog prefetch for the default channel. Heavy work (the DCB cook) runs
   Rust-side at startup (`spawn_startup_warm`) and is **visible**: the
   `data:progress` stages render as a banner in `Shell.Content` — a
   multi-minute parse must never read as a hang.

4. **Onboarding reads stores.** Wizard steps never gate rendering on a fetch
   the shell already did. Render from the store, refresh silently.

5. **Backend pushes invalidation.** Rust emits `data:changed` after any
   load / wipe / startup-warm; the layout refreshes statuses and invalidates
   catalog caches. Stores also refresh on window focus — a suspended webview
   runs no JS, so focus is the catch-up point (same pattern as
   notifications).

6. **Webview state is disposable** (docs/memory.md). Stores must be fully
   rebuildable from Tauri commands at any time — nothing user-durable may
   live only in a store.

7. **Browse state is URL-backed** (`createBrowseState` — the kit rule).
   Search, facets, and sort live in the URL: shareable, and the back button
   walks them. Navigating away and back via the nav resets to defaults —
   that is accepted; the URL is the canon, not a hidden store. **Expansion
   is page-local** (`createExpansion`), never the URL.

Per-store shape: module-level `$state` + a read-only exported view object +
exported loader/mutator functions (`sc.svelte.ts` is the reference example).

# Design language

**The split: @veelume/ui owns the component system; Starlume owns the
tokens.** `src/app.css` is the palette's source of truth; `src/theme.css`
bridges it onto the kit's shadcn-convention token names (`--background`,
`--primary`, …) — the kit renders in Starlume's own colors, faces, and
radii. When a written rule and a token disagree, the token wins — fix the
doc.

**Starlume in one line:** one dark theme; a warm-charcoal ground (no blue); a
single orange-amber accent used as a *quiet low-opacity tint*, not a solid fill;
compact and information-dense; border-first surfaces; a three-role type system;
fast, flat animation.

## Foundations (tokens in `src/app.css`)

Every value below is a CSS custom property on `:root`. **No hex in components —
tokens only.** Tints come from the `--accent-fill*` tokens or
`color-mix(in srgb, var(--accent) N%, transparent)`, never a new literal.

- **Palette**: ground `--bg` / `--bg-raised`, hairline `--border`, text `--text`
  / `--text-dim`; accent trio `--accent` / `--accent-dim` (identity gradient) /
  `--on-accent` (glyphs on a solid amber fill); status `--good` / `--warn` /
  `--bad`; `--discord` and `--em-link` are the only two non-palette colors
  (Discord blurple, and the in-text link blue for mission `<EM4>` runs).
  Semantic aliases name the recurring roles/tints: `--text-strong` /
  `--text-faint`, `--surface-*`, `--accent-fill` / `-weak` / `-faint` /
  `--accent-line`, `--border-soft` / `--accent-border` / `--danger-border`, and
  `--app-backdrop`.
- **The kit bridge** (`src/theme.css`): shadcn-`--primary` is the solid amber,
  shadcn-`--accent` is the *tinted* quiet-highlight (`--accent-fill-weak`) —
  so kit hover surfaces carry the signature tint, and app.css's own
  `--accent` keeps its meaning. Radii and typefaces map verbatim; density is
  the compact (36px-target) tier. The bridge uses `@theme inline` so no
  generated variable can collide with app.css's names.
- **Accent — the signature is a *quiet* tint, not a solid fill.** Active nav,
  the primary button, selected pills/rows, unread markers are amber over the
  charcoal at ~8–15% + an amber border/text. A *solid* amber fill appears only
  on the avatar mark, badge bubbles, and a Switch's on-track. Accent marks
  *interactive or active* things — **never body text.** `--bad`/red is only
  for destructive or failing things.
- **Radii**: `--radius` (7px) controls, `--radius-lg` (9px) cards/panels,
  `--radius-pill` (999px), `--radius-chip` (4px) badges. The kit sees these as
  `rounded-md` / `rounded-lg` via the bridge.
- **Type — three roles, self-hosted** (`static/fonts/`, `@font-face` in
  `app.css`; **no webfont CDN** — the online-policy invariant forbids runtime
  font fetches). DISPLAY `--font-display` = **Lekton** (wordmark, headings, nav,
  tabs, buttons; ships 400/700 only, so emphasis jumps straight to bold). BODY
  `--font-sans` / `--font-body` = **IBM Plex Sans** (body, tables, inputs,
  detail values). MONO `--font-mono` = **JetBrains Mono** (GUIDs, record paths,
  codes, uppercase-tracked micro-labels). 14px base; scale in `--text-*`.
  **Kit components inherit the body face** — a kit surface that should speak
  the display face gets the `font-display` utility passed through its `class`
  prop (the rail does this; tabs/dialog titles get the same treatment as they
  arrive). Assign the face deliberately per surface; don't assume it.
- **Spacing**: real `--space-0…8` tokens (2/4/6/8/10/12/14/16/24) plus layout
  constants `--content-pad` etc.
- **Surfaces are border-first.** At rest a card/panel is a `1px solid --border`
  outline with **no fill and no shadow**; `--bg-raised` on hover/raised;
  shadows only on floating layers. Panels are opaque — **no `backdrop-blur`.**
  (Known debt: the `--app-backdrop` top glow is currently hidden behind
  `Shell.Root`'s flat background — fix is a kit-side `class` pass-through.)
- **Motion & elevation**: `--transition-fast` (90ms), `--transition-panel`
  (140ms ease-out). Fast and flat — no bounce, no spring.

## Cascade mechanics (how app.css and the kit coexist)

Tailwind (v4, **preflight included**) is imported in layers; everything else
in `app.css` is **unlayered and therefore wins** wherever both could apply.
Two consequences, both deliberate:

- **Element-level control styling is opt-in.** `button`/`input`/`select`
  element rules would out-cascade every utility on kit-rendered elements
  (this bug shipped twice in one evening). App controls carry `.btn`
  (+ `.primary` / `.subtle`), `.input`, `.select`; kit components and fully
  self-styled one-offs stay bare. Never add an element-level rule for
  anything the kit renders.
- **Content defaults are owned, not inherited.** Preflight zeroes `p`/`ul`
  margins and bullets; a small `main`-scoped block in app.css restores them
  for the hand-rolled pages. It shrinks away as pages migrate.

**The shared-pattern rule, kit era:** a pattern used on 2+ pages goes to the
**kit** when there is one right answer (fix-in-kit-then-bump — never copy a
kit part into the app), to `app.css` when it is Starlume idiom (badges,
pills, `.metric`), and scoped `<style>` only for genuinely page-specific
layout.

## Layout & shell

The shell is the kit's: `Shell.Root` (nav as `NavGroup[]` data — adding a
destination is one entry in the root layout, never a layout edit),
`Shell.Rail` (collapses to icons below ~840px; speaks `font-display`; brand
row + `Notify.Bell`/`Center` in its header snippet), `Shell.AccountFooter`
(RSI-primary identity states + settings cog; wrapped back to `font-sans`),
`Shell.Content` (the game-data progress banner rides its `banner` snippet).
Rail-only — no bottom bar. Unmigrated pages get their 24px padding from a
temporary `main` rule.

## Primitives

**Kit parts are the default** — `Button`, `Switch` (stateless: `checked` +
`onchange(next)`), `Segmented`, `StatusBadge` (+ per-domain `StatusMap`s),
`Dialog`/`ConfirmDialog`, `Popup`, `Wizard`, `Loading`/`Progress`/
`Placeholder`, the notify funnel (`notify()` / keyed `ingest()`; the Tauri
transport adapter lives in `src/lib/state/notifications.svelte.ts` and
re-exports the store, so consumers keep one import path).

**Surviving app classes** (`app.css`): `.btn` / `.input` / `.select` (see
Cascade mechanics), `.check` (inline filter checkbox — `Switch` is for
preferences), `table.data`, `.badge` (+ `.accent` / `.danger`), `.pills` +
`.pill`, `.metric` (quiet mono right-slot readout), text utilities `.dim` /
`.error` / `.mono` / `.label`.

**Surviving app components**: `Avatar.svelte` (identity mark),
`CatalogDescriptions.svelte` + `RichText.svelte` (SC text rendering — see
below), the onboarding step components.

## Catalogs — the kit surface recipe

Every catalog (Items, Resources, Missions, Manufacturers) is the same
composition — `Surface.Root` (descriptor + URL browse) → `Surface.List`
(search + filter panel + sort come from the descriptor; the list is
viewport-windowed) → an `Expand.Row` per row:

- **Descriptor**: `sources` → `derive` (records → rows with stable `key` +
  `title`) → `searchIn` → `facets` (option lists may derive from the data)
  → `sorts`. Facet ids match browse-state keys.
- **Expansion is the design-doc accordion** — no side panels.
  `createExpansion("one")` for deep reads (Missions), `"many"` for shallow
  peeks (Resources, Items). Expensive detail is fetched lazily on first
  expand and kept for the session (Items is the reference).
- **Fill order inside an expansion:** `Expand.Facts` (label/value pairs) →
  description (via `CatalogDescriptions rich` for SC text) → custom detail,
  with `Expand.Cols` splitting description | detail on wide screens.
- **Paging: the windowed list, never a pager.** `Surface.List` windows the
  full in-memory set (the kit's 10k envelope); the old sentinel/pager
  patterns are gone. A genuinely server-paged list is the only place a pager
  may return — no current page qualifies (Items ships whole via
  `data_items_all`).

## Text & content

- **`RichText.svelte`** renders raw Star Citizen strings: literal `\n` → real
  line breaks, and DCB `<EMn>…</EMn>` emphasis tags → styled runs — **EM0–EM2
  plain, EM3 underline, EM4 `--em-link` blue.** Pass `rich` to
  `CatalogDescriptions` to route a block through it. Never `{@html}` game data.
- **Copy is terse and self-explanatory.** Label the control, don't caption it;
  explanations only for a real risk / non-obvious consequence / an empty
  state that needs a next step — one short line in `--text-dim`.
- **Sentence case everywhere.** Uppercase is reserved for mono micro-labels,
  never running copy. **No em-dash "label — aside" mannerism** in UI copy.
- **Voice:** second person, plain, no marketing tone, no exclamation points.
  First person only for the user's own things ("My friend code," "Me").
- **Buttons:** imperative verb, one or two words. **Empty states:** one line
  stating the next action.
- **Numbers & units:** thousands separators (`24,500 aUEC`), `~` for
  estimates, monospace for GUIDs / build IDs / paths. **No emoji in UI copy.**
- **Domain vocabulary is SC/RSI-native** and used exactly: RSI, UEE Citizen,
  org / SID, launcher, channel (LIVE / PTU), aUEC, blueprint, gRPC, PU build.

## Iconography

- **lucide-svelte is the icon set** (adopted 2026-08-29 with the kit shell —
  its nav contract takes component icons; Lucide was the pre-flagged
  choice). Nav, settings categories, and chrome icons come from it; pick
  outline-simple glyphs and pass them as components.
- **Geometric Unicode glyphs survive for inline data marks** (render at text
  size/color, not emoji): `✓` verified, `✕` close/error, `⚠` illegal, `⇄`
  shareable, `1×` once-only, `◆` Discord-linked, `👑` group owner (the lone
  permitted emoji). Module descriptor `icon` strings (onboarding picker) are
  glyphs; module nav icons are Lucide components.

## Brand

No logo or mark exists — the brand is the wordmark **"Starlume"** in Lekton,
`--accent`, weight 700. Don't invent a mark. `src-tauri/icons/` are Hearth
placeholders (see CLAUDE.md gotchas) — replace before any public release.
