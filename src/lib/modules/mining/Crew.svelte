<script lang="ts">
  // Crew: your group's loadouts, each measured as what it adds beside your
  // ship on the calculator's rock (the figure Bring along shows). Selecting
  // a row opens a sheet beside it that edits with the ship card's parts: the
  // same heads, the same picker (laid over the list), the same try-out
  // against the saved loadout. Bring along puts it on the calculator.
  //
  // For now a crew loadout is a static copy you keep yourself. With a
  // backend it becomes a friend's shared loadout, kept in sync and read-only
  // here.
  import type { EvalView, LaserInputView, LoadoutView, MiningCatalogView, SavedLoadoutView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import { nameHue } from "$lib/monogram";
  import {
    addSupport,
    changesFrom,
    compareOnRock,
    deleteLoadout,
    miningStore,
    removeSupport,
    saveLoadout,
    savedForm,
  } from "$lib/state/mining.svelte";
  import CrackCell from "./CrackCell.svelte";
  import HeadsEditor from "./HeadsEditor.svelte";
  import { int, lasersLine, namesOf, oreName, ownLasers } from "./format";
  import PartPicker from "./PartPicker.svelte";
  import TryOut from "./TryOut.svelte";

  let { catalog, channel, loadout }: { catalog: MiningCatalogView; channel: string; loadout: LoadoutView } = $props();

  const NEW = "new";

  const names = $derived(namesOf(catalog));
  const mstate = $derived(miningStore.state);
  const saved = $derived(mstate?.saved ?? []);
  const crew = $derived(saved.filter((s) => s.owner !== null).sort((a, b) => (a.owner ?? "").localeCompare(b.owner ?? "")));
  const aboard = $derived(new Set(loadout.lasers.flatMap((l) => (l.crew ? [l.crew] : []))));
  const own = $derived(ownLasers(loadout));
  const distance = $derived(own.find((l) => l.distanceM !== null)?.distanceM ?? null);
  const shipName = $derived(saved.find((s) => s.id === mstate?.basedOn)?.name ?? "ship");

  // ── Selection and the sheet's draft (page-local) ──
  let selected = $state("");
  let draft = $state<LaserInputView[]>([]);
  let draftName = $state("");
  let draftOwner = $state("");
  let open = $state("");
  let error = $state("");

  const sel = $derived(crew.find((s) => s.id === selected) ?? null);
  const isNew = $derived(selected === NEW);

  function select(id: string) {
    selected = id;
    open = "";
    error = "";
    const s = crew.find((x) => x.id === id);
    draft = s ? structuredClone($state.snapshot(s.loadout.lasers)) : [];
    draftName = s?.name ?? "";
    draftOwner = s?.owner ?? "";
  }
  // Open on the first one aboard, else the first one.
  $effect(() => {
    if (selected && (isNew || sel)) return;
    const first = crew.find((s) => aboard.has(s.id))?.id ?? crew[0]?.id ?? "";
    if (first) select(first);
  });

  // ── Measured on the rock, beside your ship ──
  const alone = (lasers: LaserInputView[]): LoadoutView => ({
    lasers: lasers.map((l) => ({ ...l, crew: null, distanceM: distance })),
    gadget: loadout.gadget,
    lead: null,
  });
  /** A crew mate's ship beside your ship as it is on the calculator. */
  const beside = (lasers: LaserInputView[]): LoadoutView => {
    const after = Math.max(-1, ...own.map((l) => l.owner)) + 1;
    const base = Math.min(...lasers.map((l) => l.owner), Infinity);
    return {
      lasers: [...own.map((l) => ({ ...l, crew: null })), ...lasers.map((l) => ({ ...l, owner: after + (l.owner - base), crew: "compare", distanceM: null }))],
      gadget: loadout.gadget,
      lead: null,
    };
  };
  const pct = (e: EvalView | null | undefined) => (e === undefined ? undefined : e?.difficulty ? Math.round(e.difficulty.controlled * 100) : null);

  // undefined: being worked out; null: no rock to compare on.
  let rowEvals = $state<Map<string, EvalView | null> | null | undefined>(undefined);
  let yoursAlone = $state<EvalView | null | undefined>(undefined);
  $effect(() => {
    const list = [...crew.map((s) => beside(s.loadout.lasers)), alone(own)];
    const ids = crew.map((s) => s.id);
    let live = true;
    void compareOnRock(channel, list).then((evals) => {
      if (!live) return;
      rowEvals = evals ? new Map(ids.map((id, k) => [id, evals[k]])) : null;
      yoursAlone = evals ? evals[evals.length - 1] : null;
    });
    return () => (live = false);
  });
  const minus = $derived(pct(yoursAlone) ?? 0);
  const rowCrack = (s: SavedLoadoutView) => {
    if (!rowEvals) return rowEvals;
    const v = pct(rowEvals.get(s.id));
    return v === undefined || v === null ? v : v - minus;
  };

  // The sheet's try-out: the draft against the saved loadout.
  const changes = $derived(
    sel ? changesFrom(sel.loadout.lasers, draft) + (draftName.trim() !== sel.name ? 1 : 0) + ((draftOwner.trim() || null) !== sel.owner ? 1 : 0) : 0,
  );
  let wasEval = $state<EvalView | null | undefined>(undefined);
  let nowEval = $state<EvalView | null | undefined>(undefined);
  $effect(() => {
    if (!sel || !changes) return;
    const pair = [beside(sel.loadout.lasers), beside(draft)];
    wasEval = nowEval = undefined;
    let live = true;
    void compareOnRock(channel, pair).then((e) => {
      if (!live) return;
      wasEval = e?.[0] ?? null;
      nowEval = e?.[1] ?? null;
    });
    return () => (live = false);
  });

  const rock = $derived.by(() => {
    const r = miningStore.report?.rock;
    if (!r) return "";
    const src = miningStore.source;
    const name = src ? oreName(src.rockType) : "the rock";
    const gadget = loadout.gadget ? catalog.gadgets.find((g) => g.id === loadout.gadget)?.name : null;
    return `${name}, ${int(r.mass)} t${gadget ? `, ${gadget}` : ""}`;
  });

  // ── Actions ──
  async function save(id: string | null, name: string) {
    const owner = draftOwner.trim();
    if (!owner) {
      error = "Whose loadout is it?";
      return error;
    }
    error = (await saveLoadout(id, name, owner, savedForm(draft))) ?? "";
    if (error) return error;
    const made = miningStore.state?.saved.find((s) => s.owner === owner && s.name.toLowerCase() === name.trim().toLowerCase());
    if (made) select(made.id);
    return null;
  }
  function startNew() {
    selected = NEW;
    draft = [];
    draftName = "Prospector";
    draftOwner = "";
    open = "";
    error = "";
  }
  async function duplicate() {
    if (!sel) return;
    const name = `${sel.name} copy`;
    error = (await saveLoadout(null, name, sel.owner, sel.loadout)) ?? "";
    const made = miningStore.state?.saved.find((s) => s.owner === sel.owner && s.name === name);
    if (made) select(made.id);
  }
  async function remove() {
    if (!sel) return;
    if (aboard.has(sel.id) && sel.owner) await removeSupport(sel.id, sel.owner);
    await deleteLoadout(sel.id);
    selected = "";
  }
</script>

<div class="lo">
  <div class="list">
    {#if open}
      <div class="over">
        <PartPicker
          {catalog}
          {channel}
          target={open}
          lasers={draft}
          build={beside}
          {minus}
          onpick={(next) => {
            open = "";
            draft = next;
          }}
          onclose={() => (open = "")}
        />
      </div>
    {/if}
    <section>
      <div class="gh">
        <p class="label">Crew</p>
        <small class="faint">what each adds beside your {shipName}</small>
        <span class="grow"></span>
        <button type="button" class="btn" onclick={startNew}>+ Crew mate</button>
      </div>
      <small class="faint sub">{rock ? `on the calculator's rock: ${rock}` : "scan or roll a rock to measure them"}</small>
      {#each crew as s (s.id)}
        <button type="button" class="row" class:sel={s.id === selected} onclick={() => select(s.id)}>
          <Monogram name={s.owner ?? "?"} size={30} />
          <span class="nm">
            <b>{s.owner}<span class="faint"> · {s.name}</span>{#if aboard.has(s.id)}<span class="on">aboard</span>{/if}</b>
            <small>{lasersLine(s.loadout, names)} · {s.loadout.lasers.flatMap((l) => l.modules).map((m) => (names.get(m) ?? "?").replace(/\s+Module$/i, "")).join(", ") || "no modules"}</small>
          </span>
          <CrackCell value={rowCrack(s)} adds />
        </button>
      {:else}
        <p class="faint none">Your group's loadouts: add one, then bring it along on the calculator.</p>
      {/each}
    </section>
    <p class="later">
      Crew loadouts are copies you keep yourself for now. Later, with a Starlume server: friends share their loadouts and
      they stay in sync here.
    </p>
  </div>

  {#if sel || isNew}
    <div class="sheet">
      <NowCard hue={nameHue(draftOwner || "?")} chip="● CREW" version="">
        {#snippet top()}{#if sel && aboard.has(sel.id)}<span class="on">aboard</span>{/if}{/snippet}
        <div class="nf">
          <label>
            <Monogram name={draftOwner || "?"} size={26} />
            <input class="pl" bind:value={draftOwner} placeholder="Player" aria-label="Player" />
          </label>
          <input class="nmi" bind:value={draftName} placeholder="Prospector" aria-label="Loadout name" />
          {#if changes && !isNew}<span class="mod">modified</span>{/if}
        </div>
        {#if sel && changes}
          <TryOut
            was={wasEval}
            now={nowEval}
            {changes}
            {minus}
            name={`${draftName} 2`}
            onrevert={() => select(sel.id)}
            onsave={() => void save(sel.id, draftName)}
            onsaveas={(n) => save(null, n)}
          />
        {/if}
        <HeadsEditor
          {catalog}
          lasers={draft}
          base={sel?.loadout.lasers ?? null}
          {open}
          onopen={(k) => (open = open === k ? "" : k)}
          onchange={(next) => (draft = next)}
          structure
        />
        {#if error}<p class="error">{error}</p>{/if}
        {#snippet action()}
          {#if isNew}
            <button type="button" class="btn subtle" onclick={() => (selected = "")}>Cancel</button>
            <button type="button" class="btn primary" disabled={!draftName.trim() || !draft.length} onclick={() => void save(null, draftName)}
              >Save</button
            >
          {:else if sel}
            <button type="button" class="btn subtle danger" onclick={() => void remove()}>Delete</button>
            <button type="button" class="btn" onclick={() => void duplicate()}>Duplicate</button>
            <span class="grow"></span>
            {#if aboard.has(sel.id)}
              <button type="button" class="btn" onclick={() => sel.owner && void removeSupport(sel.id, sel.owner)}>Take off the rock</button>
            {:else}
              <button type="button" class="btn primary" onclick={() => void addSupport(sel.id)}>Bring along</button>
            {/if}
          {/if}
        {/snippet}
      </NowCard>
    </div>
  {/if}
</div>

<style>
  /* The rows reach 10px past their column (their content lines up with the
     headers); the inset keeps that off the page's scrollbar. */
  .lo {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 430px;
    gap: 18px;
    align-items: start;
    padding-inline: 10px;
  }
  .list {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .over {
    position: absolute;
    inset: 0 0 auto 0;
    z-index: 4;
  }
  section {
    display: flex;
    flex-direction: column;
  }
  .gh {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 6px;
  }
  .gh .label {
    margin: 0;
  }
  .gh small {
    font-size: 0.78rem;
  }
  .gh .btn {
    padding: 3px 10px;
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
  }
  .none {
    margin: 4px 0;
    font-size: var(--text-small);
  }
  .sub {
    margin: -2px 0 4px;
    font-size: 0.78rem;
  }
  .row {
    display: grid;
    grid-template-columns: 30px minmax(0, 1fr) 220px;
    gap: 0 12px;
    align-items: center;
    padding: 8px 10px;
    margin-inline: -10px;
    border: 0;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    border-radius: 8px;
    background: none;
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .row.sel {
    border-top-color: transparent;
    background: var(--accent-fill-weak);
    box-shadow: inset 0 0 0 1px var(--accent-border);
  }
  .nm {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .nm b {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: var(--weight-semibold);
  }
  .nm b .faint {
    font-weight: var(--weight-medium);
  }
  .nm small {
    font-size: 0.76rem;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .on {
    margin-left: 4px;
    padding: 1px 7px;
    border-radius: var(--radius-pill);
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: var(--weight-medium);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--accent) 85%, var(--text));
    border: 1px solid var(--accent-border);
  }
  .later {
    margin: 0;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px dashed var(--border);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .sheet {
    position: sticky;
    top: 12px;
    display: flex;
  }
  .nf {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
    margin-top: -4px;
  }
  .nf label {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .nf input {
    font: inherit;
    color: var(--text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 2px 6px;
    margin-left: -6px;
  }
  .nf input:hover,
  .nf input:focus {
    border-color: var(--border);
    background: var(--bg);
    outline: none;
  }
  .nf .nmi {
    width: 100%;
    max-width: 300px;
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 2rem;
    line-height: 1.1;
    color: color-mix(in srgb, var(--hue) 55%, var(--text-strong));
  }
  .nf .pl {
    width: 9rem;
    font-weight: var(--weight-semibold);
  }
  .mod {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--warn);
  }
  .danger {
    color: var(--bad);
  }
  .error {
    margin: 0;
  }
  @media (max-width: 1100px) {
    .lo {
      grid-template-columns: minmax(0, 1fr);
    }
    .sheet {
      position: static;
    }
  }
</style>
