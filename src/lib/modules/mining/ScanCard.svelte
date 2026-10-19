<script lang="ts">
  // The rock on the calculator, as the scanner shows it: the name large with
  // Starlume's difficulty beside it, the figures as tiles, the composition
  // with bars and quality. "Laser on rock" shows what the scanner reads with
  // the beam on (the loadout's modifiers applied), "Bare rock" the rock on
  // its own. "What if it weighs" tries the rock at another mass (the solve
  // follows) over the loadout's classes along the mass axis. One way in:
  // Read scan grabs the game window and reads its SCAN RESULTS panel; the
  // menu beside it reads a screenshot file, types a scan in, or rolls a
  // rock. A reading fills the card only when every value checked out (a
  // wrong value is worse than none); one that didn't shows the panel as
  // read and fills nothing, with the read values one click from the
  // editor. The draft is the store's (frontend rule 1).
  import { open } from "@tauri-apps/plugin-dialog";
  import { Segmented } from "@veelume/ui";
  import { X } from "lucide-svelte";
  import Chevron from "$lib/components/Chevron.svelte";
  import type { MiningCatalogView, MiningReportView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { dataStore } from "$lib/state/data.svelte";
  import {
    captureScan,
    dismissCapture,
    miningStore,
    parseNumber,
    readScreenshot,
    readingAccepted,
    setMassOverride,
    useCapture,
  } from "$lib/state/mining.svelte";
  import { TONE_HUE, difficulty, int, namesOf, oreName } from "./format";
  import MassBand from "./MassBand.svelte";
  import RockArt from "./RockArt.svelte";

  let {
    catalog,
    report,
    onedit,
    onroll,
  }: {
    catalog: MiningCatalogView;
    report: MiningReportView | null;
    onedit: () => void;
    onroll: () => void;
  } = $props();

  let beam = $state<"on" | "off">("on");

  const names = $derived(namesOf(catalog));
  const rock = miningStore.rock;
  const rows = $derived(
    rock.rows.flatMap((r) => {
      const percent = parseNumber(r.percent);
      if (!r.element || percent === null || percent <= 0) return [];
      const q = parseNumber(r.quality);
      return [{ name: names.get(r.element) ?? "Unknown", percent, quality: q === null ? null : Math.round(q) }];
    }),
  );
  const inert = $derived(Math.max(0, 100 - rows.reduce((s, r) => s + r.percent, 0)));
  const scanned = $derived(parseNumber(rock.mass, true));
  const mass = $derived(miningStore.massOverride ?? scanned);
  const source = $derived(miningStore.source);
  const band = $derived(miningStore.band);
  const title = $derived(
    (source ? oreName(source.rockType) : null) ??
      (rows.length ? oreName([...rows].sort((a, b) => b.percent - a.percent)[0].name) : "No rock yet"),
  );

  const yours = $derived(report?.yours ?? null);
  const word = $derived(yours ? difficulty(yours.eval, yours.guide) : null);
  const on = $derived(beam === "on" && yours !== null);
  const resistance = $derived(
    report ? (on && yours ? `${yours.scannerResistancePct.toFixed(0)}%` : `${(report.rock.gadgetResistance * 100).toFixed(0)}%`) : "—",
  );
  const instability = $derived(
    report ? (on && yours ? yours.scannerInstability.toFixed(1) : report.rock.gadgetInstability.toFixed(1)) : "—",
  );

  const capture = $derived(miningStore.capture?.kind === "rock" ? miningStore.capture : null);
  const busy = $derived(miningStore.capturing !== null);
  const reading = $derived(capture?.rock ?? null);
  const accepted = $derived(readingAccepted(capture));
  const fromGame = $derived(source?.pick === "Read from the game");
  let menu = $state(false);
  let asRead = $state(false);

  function readNow() {
    const c = dataStore.channel;
    asRead = false;
    if (c) void captureScan(c, "rock");
  }

  async function fromFile() {
    menu = false;
    const c = dataStore.channel;
    const path = await open({ multiple: false, filters: [{ name: "Screenshots", extensions: ["png", "jpg", "jpeg"] }] });
    if (c && typeof path === "string") void readScreenshot(c, "rock", path);
  }

  /** A reading that didn't check out, into the editor to correct by hand. */
  function typeInRead() {
    if (capture) useCapture(capture);
    dismissCapture();
    onedit();
  }

  function closeMenu(e: MouseEvent) {
    if (menu && !(e.target as HTMLElement).closest(".splitbtn")) menu = false;
  }
</script>

<svelte:window onclick={closeMenu} />

<NowCard hue="var(--hue-sand)" chip="◆ SCAN" version={title} size="wide">
  {#snippet art()}<RockArt />{/snippet}
  {#snippet top()}
    <Segmented
      class="beam"
      options={[
        { value: "on", label: "Laser on rock" },
        { value: "off", label: "Bare rock" },
      ]}
      value={beam}
      onchange={(v) => (beam = v as "on" | "off")}
    />
  {/snippet}
  {#if word}
    <div><span class="hue-chip" style:--hue={TONE_HUE[word.tone]}>{word.word}</span></div>
  {/if}
  {#if rows.length || mass !== null}
    <div class="stat-tiles">
      <div class="stat-tile"><b>{mass !== null ? `${int(mass)} t` : "—"}</b><span>mass</span></div>
      <div class="stat-tile"><b>{report ? report.rock.scu.toFixed(1) : "—"}</b><span>SCU</span></div>
      <div class="stat-tile"><b class:raised={on}>{resistance}</b><span>resistance</span></div>
      <div class="stat-tile"><b class:raised={on}>{instability}</b><span>instability</span></div>
    </div>
    {#if band && scanned !== null}
      <div class="weighs">
        <div class="h">
          <span class="label">What if it weighs</span>
          {#if miningStore.massOverride !== null}
            <button class="link" onclick={() => setMassOverride(null)}>Back to the scan</button>
          {/if}
        </div>
        <MassBand {band} stale={miningStore.bandStale} value={mass ?? scanned} scan={scanned} onchange={(m) => setMassOverride(Math.round(m))} />
      </div>
    {/if}
    <div class="comp">
      {#each rows as r, i (i)}
        <span class="p">{r.percent.toFixed(2)}</span>
        <span class="nm">{r.name}</span>
        <span class="track"><i style:width="{Math.min(100, r.percent)}%"></i></span>
        <span class="q">{r.quality === null ? "" : `Q ${r.quality}`}</span>
      {/each}
      {#if inert > 0.005}
        <span class="p">{inert.toFixed(2)}</span>
        <span class="nm faint">Inert</span>
        <span class="track"><i style:width="{inert}%"></i></span>
        <span class="q"></span>
      {/if}
    </div>
  {/if}
  {#if miningStore.captureError}
    <div class="read bad">
      <p>{miningStore.captureError}</p>
      <button type="button" class="x" onclick={dismissCapture} aria-label="Dismiss"><X size={14} /></button>
    </div>
  {:else if capture && reading && !reading.found}
    <div class="read bad">
      <p>No SCAN RESULTS panel in that shot. In mining mode, with a rock scanned?</p>
      <button type="button" class="x" onclick={dismissCapture} aria-label="Dismiss"><X size={14} /></button>
    </div>
  {:else if capture && reading && !accepted}
    <div class="read bad">
      {#if capture.preview}<img src={capture.preview} alt="The scan panel as read" />{/if}
      <div class="said">
        <b>Not filled in</b>
        <span
          >{reading.unsure.length ? `${reading.unsure.join(", ")} didn't check out` : "Not every value could be read"}; a wrong value
          is worse than none, so the card keeps the rock from before.</span
        >
        {#each reading.notes as n (n)}<span class="faint">{n}</span>{/each}
        <div class="acts">
          <button type="button" class="btn primary" disabled={busy} onclick={readNow}>Read again</button>
          <button type="button" class="btn" onclick={typeInRead}>Type it in, with what was read</button>
        </div>
      </div>
      <button type="button" class="x" onclick={dismissCapture} aria-label="Dismiss"><X size={14} /></button>
    </div>
  {:else if asRead && accepted && capture?.preview}
    <div class="read"><img src={capture.preview} alt="The scan panel as read" /></div>
  {/if}
  {#snippet action()}
    <span class="from">
      {#if fromGame && source}Read from the game, {source.place}
        {#if accepted}<span class="ok">✓ checked</span>{/if}
        {#if accepted && capture?.preview}<button type="button" class="link" onclick={() => (asRead = !asRead)}
            >{asRead ? "hide" : "as read"}</button
          >{/if}
      {:else if source}{source.pick} at {source.place}{:else if rows.length}typed in{/if}
    </span>
    <button class="btn" onclick={onedit}>{rows.length ? "Edit" : "Type it in"}</button>
    <span class="splitbtn">
      <button
        type="button"
        class="btn primary"
        disabled={busy}
        onclick={readNow}
        title="Capture the game window and read its SCAN RESULTS panel">{busy ? "Reading…" : "Read scan"}</button
      ><button type="button" class="btn primary more" onclick={() => (menu = !menu)} aria-label="More ways in" aria-expanded={menu}
        ><Chevron open={menu} /></button
      >
      {#if menu}
        <div class="menu" role="menu">
          <button type="button" role="menuitem" onclick={() => void fromFile()}
            >From a screenshot…<small>a PNG or JPG of the scan panel</small></button
          >
          <button type="button" role="menuitem" onclick={() => ((menu = false), onedit())}>Type it in</button>
          <button type="button" role="menuitem" onclick={() => ((menu = false), onroll())}
            >Roll a rock<small>from the Rock finder</small></button
          >
        </div>
      {/if}
    </span>
  {/snippet}
</NowCard>

<style>
  :global(.beam) {
    font-size: var(--text-small);
  }
  .raised {
    color: var(--warn);
  }
  .weighs {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .weighs .h {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .weighs .label {
    flex: 1;
    margin: 0;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: var(--text-small);
    color: var(--accent);
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .comp {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr) minmax(60px, 140px) 52px;
    gap: 6px 12px;
    align-items: center;
    font-size: 0.86rem;
  }
  .p {
    font-family: var(--font-mono);
    font-size: 0.82rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .nm {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .faint {
    color: var(--text-faint);
  }
  .track {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--hue-sand) 65%, transparent);
  }
  .q {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: color-mix(in srgb, var(--hue-quality) 80%, var(--text));
    text-align: right;
  }
  /* A reading: the panel as read, and what became of it. */
  .read {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    padding: 10px;
    border-radius: var(--radius);
    background: var(--bg-sunk);
    border: 1px solid var(--border);
    font-size: var(--text-small);
  }
  .read.bad {
    background: color-mix(in srgb, var(--warn) 6%, var(--bg-sunk));
    border-color: color-mix(in srgb, var(--warn) 30%, transparent);
  }
  .read img {
    max-height: 220px;
    max-width: 45%;
    border-radius: 4px;
  }
  .read p {
    margin: 0;
    flex: 1;
    color: var(--text-dim);
  }
  .said {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    color: var(--text-dim);
  }
  .said b {
    font-size: 0.95rem;
    color: var(--text);
  }
  .acts {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: 2px;
  }
  .acts .btn {
    padding: 4px 10px;
  }
  .x {
    display: inline-flex;
    padding: 2px;
    border: 0;
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }
  .ok {
    color: var(--good);
  }
  .from {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-right: auto;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  /* Read scan and its menu as one button: no gap, the seam a hairline. */
  .splitbtn {
    position: relative;
    display: inline-flex;
  }
  .splitbtn .btn:first-child {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .splitbtn .more {
    display: inline-flex;
    align-items: center;
    margin-left: -1px;
    padding-inline: 8px;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    border-left: 1px solid color-mix(in srgb, var(--text-on-accent) 30%, transparent);
  }
  .menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 6px);
    z-index: 5;
    min-width: 210px;
    display: flex;
    flex-direction: column;
    padding: 5px;
    border-radius: 9px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    box-shadow: 0 18px 40px color-mix(in srgb, var(--bg-sunk) 80%, transparent);
  }
  .menu button {
    display: flex;
    flex-direction: column;
    padding: 6px 10px;
    border: 0;
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: 0.85rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .menu small {
    font-size: 0.72rem;
    color: var(--text-faint);
  }
</style>
