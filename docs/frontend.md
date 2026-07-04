# Frontend rules — data lifecycle & styling

The SvelteKit SPA re-mounts a page component on every navigation. Without
discipline that means every tab switch refetches and re-spins — the opposite
of the resident-companion feel. These rules keep the app snappy; treat them
like the module rules in the README: invariants, not suggestions.

## Data lifecycle

1. **Stores own data; pages render it.** Any dataset that outlives one visit
   (installs, statuses, catalogs, friends, identity) lives in a module store
   under `src/lib/state/*.svelte.ts`. Pages hold only ephemeral UI state
   (open panels, input focus). A page that owns fetched data in local
   `$state` is a bug — navigating away throws the data away.

2. **Fetch-once, then stale-while-revalidate.** Store loaders return the
   cache when they have it and refresh in the background. A page renders
   cached data *synchronously* on mount — no spinner for data the app has
   already seen. Spinners are reserved for the genuine first load.

3. **Startup hydration.** The root layout kicks off every cheap load
   fire-and-forget at mount: settings → auth, SC scan, data statuses,
   catalog prefetch for the default channel. Heavy work (the DCB cook) runs
   Rust-side at startup (`spawn_startup_warm`). By the time the user reaches
   a page or wizard step, its data is normally already in a store.

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

Per-store shape: module-level `$state` + a read-only exported view object +
exported loader/mutator functions (`sc.svelte.ts` is the reference example).

# Design language

The complete Starlume design system. This document plus `src/app.css` are the
**source of truth** — the design is self-contained in the repo (it originated as
a reverse-engineering of this app's own CSS, evolved in a warm "Hearth
direction"; there is no external design tool it depends on). When a written rule
and a token disagree, the token in `src/app.css` wins — fix the doc.

**Starlume in one line:** one dark theme; a warm-charcoal ground (no blue); a
single orange-amber accent used as a *quiet low-opacity tint*, not a solid fill;
compact and information-dense; border-first surfaces; a three-role type system;
minimal line/Unicode iconography; fast, flat animation.

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
- **Accent — the signature is a *quiet* tint, not a solid fill.** Active nav,
  the primary button, selected pills/rows, unread markers are amber over the
  charcoal at ~8–15% via the `--accent-fill*` tokens + an amber border/text. A
  *solid* amber fill appears only on the avatar gradient, the bell badge, and a
  Switch's on-track. Accent marks *interactive or active* things — links,
  primary action, active nav/tabs, selected rows, live values — **never body
  text.** `--bad`/red is only for destructive or failing things.
- **Radii**: `--radius` (7px) controls, `--radius-lg` (9px) cards/panels,
  `--radius-pill` (999px), `--radius-chip` (4px) badges. Instrument-tight —
  don't snap to a 4/8px grid beyond these.
- **Type — three roles, self-hosted** (`static/fonts/`, `@font-face` in
  `app.css`; **no webfont CDN** — the online-policy invariant forbids runtime
  font fetches). DISPLAY `--font-display` = **Lekton** (wordmark, headings, nav,
  tabs, buttons; ships 400/700 only, so emphasis jumps straight to bold — there
  is no medium). BODY `--font-sans` / `--font-body` = **IBM Plex Sans** (body,
  tables, inputs, detail values). MONO `--font-mono` = **JetBrains Mono** (GUIDs,
  record paths, codes, and the uppercase-tracked micro-labels — table headers,
  kickers, source tags). 14px base; type scale in `--text-*`, weights in
  `--weight-*`, and `--label-transform` / `--label-tracking` for micro-labels.
- **Spacing**: real `--space-0…8` tokens (2/4/6/8/10/12/14/16/24) plus layout
  constants `--sidebar-width` / `--detail-panel-width` / `--content-pad`.
- **Surfaces are border-first.** At rest a card/panel is a `1px solid --border`
  outline with **no fill and no shadow**. A `--bg-raised` fill appears only on
  hover (clickable cards/rows) or on genuinely raised elements. The reserve:
  *outline = at rest, fill = raised/hover, shadow = floating.* The app ground is
  `--app-backdrop` (flat warm charcoal + a whisper of warm glow from the top).
  Panels are opaque — **no `backdrop-blur`.**
- **Motion & elevation**: `--transition-fast` (90ms) on color/background/border;
  `--transition-panel` (140ms ease-out) for panel slide+fade. Fast and flat — no
  bounce, no spring. The one transform is the bell nudging to `scale(1.12)` on
  hover. Shadows only on floating layers: `--shadow-panel` (notification
  center), `--shadow-toast` (toasts) — nothing inline is shadowed. Glow is a
  whisper (a faint amber drop-shadow on the active nav icon), never on body text
  or data.

**Interaction states.** Hover: muted text brightens to `--text`;
default/primary buttons brighten their border to `--accent`; clickable
cards/rows gain the `--bg-raised` fill. Focus: inputs brighten their border to
`--accent` (no separate outline ring). Press: nothing beyond hover.

## Layout

- Fixed **`--sidebar-width` (200px)** sidebar: brand + notification bell (top),
  nav (middle), account block + settings cog (bottom); a `1px` right border.
- Scrollable `main` with **24px** padding over the backdrop.
- **List + detail**: a flexible `.list` beside a **sticky** `.detail-panel`
  (`.split`). List-heavy catalogs instead expand **inline** (see Catalogs).
- **Toolbars** are a wrap-flex row of search + selects + toggles (`.toolbar`).

## Shared primitives

**The rule: a pattern used on 2+ pages moves into `app.css` (or a shared
component under `src/lib/components/`) — scoped `<style>` is for genuinely
page-specific layout only.** Existing pages migrate opportunistically when
touched. Everything references tokens, so a palette change cascades for free.

Global classes (`app.css`): `button` (+ `.primary` quiet-amber CTA, `.subtle`
borderless), `input[type="text|url|search"]`, `select`, `.check` (inline
checkbox row), `table.data` (+ `.clickable`, `tr.selected`), `.toolbar`,
`.split` + `.list` + `.detail-panel` (+ `.wide`), `.pager`, `.card`, `.badge`
(+ `.accent` / `.danger`), `.count-badge` (+ `.owned`) / `.count-box`, `.pills`
+ `.pill` (+ `.active`, trailing `.count`), `.tabs` + `.tab` (+ `.active`), and
text utilities `.dim` / `.error` / `.mono` / `.label`.

Stateful/structured primitives are components under `src/lib/components/`:
`Switch.svelte` (Hearth toggle slider — `checked` + `onchange(next)`, optional
label snippet), `Avatar.svelte` (amber-gradient identity mark), `Toasts.svelte`,
`NotificationCenter.svelte`, and the catalog set below.

**Which control:** `Switch` for on/off **preferences** (Settings); keep the
`.check` checkbox for inline **filter** toggles in toolbars. `.pill` is the
segmented filter (All / Owned / …); `.badge` is a status chip; `.count-badge` is
the catalog "4/8" owned-progress chip (`.count-box` is its empty single-item
square).

## Catalogs — the CatalogRow system

Every catalog (Items, Resources, Missions, Manufacturers) renders through one
unified row, `src/lib/components/catalog/CatalogRow.svelte`. One anatomy:

```
[ gutter ] [ caret ] [ title (+ meta line) ] ……… [ actions ] [ right ]
```

- **Complexity scales by which slots you fill**, not by bespoke markup. Tier 1 =
  gutter + title + right (a line). Tier 2 adds the caret and expands. Tier 3 adds
  a meta line of chips.
- The `gutter` is a fixed **40px** slot so ownership badges (`CountBadge`) align
  across every catalog; omit it on undecorated rows.
- **Expansion is always an inline accordion** — no side panels. Open state is
  *controlled* by the page (`open` + `ontoggle`) so it can persist in the browse
  store. Only one row open at a time is the norm.
- **Fill order inside an expansion:** `CatalogFacts` (glued label/value pairs) →
  `CatalogDescriptions` (labeled prose) → custom detail (reward grids,
  locations, nested rows).
- **Wide screens:** wrap the description + the rest in `.catalog-cols` — a grid
  that puts the description left and everything after it right, collapsing to one
  column under 1100px. Facts span full width on top; the GUID line sits full
  width at the bottom. `CatalogDescriptions flush` drops its gutter so it can
  live inside a column.

Supporting classes: `.catalog` (the list), `.metric` (a quiet mono right-slot
readout), `.catalog-detail` (indented custom-detail block aligned to the 52px
gutter), `.catalog-cols` (the two-column expansion body).

**Paging: infinite scroll, never a pager.** A prev/next pager *plus* a scroll
container is banned — it's the worst of both. An in-memory catalog reveals rows
in batches as a bottom sentinel nears the viewport (`IntersectionObserver`,
Missions is the reference), resetting the window when the search/filter/sort
changes. (The `.pager` class remains only for a genuinely server-paged list that
fetches one page at a time.)

## Text & content

- **`RichText.svelte`** renders raw Star Citizen strings: literal `\n` → real
  line breaks, and DCB `<EMn>…</EMn>` emphasis tags → styled runs — **EM0–EM2
  plain, EM3 underline, EM4 `--em-link` blue.** Pass `rich` to
  `CatalogDescriptions` to route a block through it. Never `{@html}` game data.
- **Copy is terse and self-explanatory.** Label the control, don't caption it;
  if a control needs a sentence to explain it, fix the control or its placement
  first. Explanations only for a real risk / non-obvious consequence / an empty
  state that needs a next step — one short line in `--text-dim`.
- **Sentence case everywhere.** Uppercase is reserved for mono micro-labels
  (`--label-transform` — table headers, section sub-headers, source tags), never
  running copy. **No em-dash "label — aside" mannerism** — write a plain
  sentence or drop the aside.
- **Voice:** second person, plain, no marketing tone, no exclamation points.
  First person only for the user's own things ("My friend code," "Me").
- **Buttons:** imperative verb, one or two words (Load, Reload, Sign out, Add
  friend). **Empty states:** one line stating the next action.
- **Numbers & units:** thousands separators (`24,500 aUEC`), `~` for estimates,
  monospace for GUIDs / build IDs / paths. **No emoji in UI copy.**
- **Domain vocabulary is SC/RSI-native** and used exactly: RSI, UEE Citizen,
  org / SID, launcher, channel (LIVE / PTU), aUEC, blueprint, gRPC, PU build.

## Iconography

There is **no icon library, icon font, or SVG sprite.** Two sources:

- **Geometric Unicode glyphs** (render at text size/color), not emoji: `✦` Home,
  `◈` Friends, `▤` Catalogs, `◆` Discord-linked (tinted `--discord`), `✓`
  verified/success, `✕` close/error, `•` info, `!` warning, `⚠` illegal flag,
  `⇄` shareable, `1×` once-only, `▸`/`▾` accordion caret, `👑` group owner (the
  lone permitted emoji — introduce no more).
- **Hand-authored inline SVGs** — the notification bell and settings cog live in
  `src/routes/+layout.svelte`. House style: `stroke-width: 1.8`, round caps/
  joins, `fill: none`, `currentColor`, ~1.05rem. Match it if you must add one;
  **Lucide** is the closest aesthetic if a real icon set is ever needed (flag the
  substitution).

## Brand

No logo or mark exists — the brand is the wordmark **"Starlume"** in Lekton,
`--accent`, weight 700. Don't invent a mark. `src-tauri/icons/` are Hearth
placeholders (see CLAUDE.md gotchas) — replace before any public release.
