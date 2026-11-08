<script lang="ts">
  // The ship picker, after erkul's: the makers in one column, the hovered
  // maker's hulls beside it, so the whole list is two short lists instead
  // of one long one. Typing searches every hull by name and role. Editions
  // (Mk II, Executive, Wikelo specials) are the page's second picker; here
  // only base hulls. Filtering and sorting the whole list is the start
  // page's job; this stays a quick pick. Esc or a click outside closes.
  import { ChevronDown, ChevronRight, Factory, Search } from "lucide-svelte";
  import type { ShipRowView } from "$lib/bindings";
  import { shipsImages } from "$lib/state/ships.svelte";
  import { careerLook } from "./careers";
  import { crewTitle } from "./seats";

  let {
    ships,
    value,
    onpick,
  }: {
    /** Base hulls (no editions). */
    ships: ShipRowView[];
    /** The open ship's base hull GUID. */
    value: string;
    onpick: (guid: string) => void;
  } = $props();

  let open = $state(false);
  let q = $state("");
  let maker = $state("");
  let el = $state<HTMLElement>();

  const current = $derived(ships.find((s) => s.guid === value) ?? null);
  const makers = $derived([...new Set(ships.map((s) => s.manufacturer ?? "Other"))].sort((a, b) => a.localeCompare(b)));
  const shown = $derived(maker || current?.manufacturer || makers[0] || "");
  const hulls = $derived(ships.filter((s) => (s.manufacturer ?? "Other") === shown).sort((a, b) => a.name.localeCompare(b.name)));
  const query = $derived(q.trim().toLowerCase());
  const flat = $derived(!!query);
  const found = $derived(
    ships
      .filter((s) => `${s.name} ${s.role ?? ""} ${s.career ?? ""}`.toLowerCase().includes(query))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

  /** A hull's name without its maker's (the column already says it). */
  const short = (s: ShipRowView) => {
    const m = s.manufacturer?.split(" ")[0];
    return m && s.name.startsWith(`${m} `) ? s.name.slice(m.length + 1) : s.name;
  };

  function pick(guid: string) {
    onpick(guid);
    open = false;
    q = "";
  }

  /** Esc closes the menu and goes no further (the ship sheet under it stays). */
  function key(e: KeyboardEvent) {
    if (e.key !== "Escape" || !open) return;
    open = false;
    e.preventDefault();
  }

  function outside(e: MouseEvent) {
    const t = e.target as HTMLElement;
    if (open && el && t.isConnected && !el.contains(t)) open = false;
  }
</script>

<svelte:window onkeydown={key} onclick={outside} />

<div class="menu" bind:this={el}>
  <button type="button" class="trigger" class:on={open} aria-haspopup="true" aria-expanded={open} onclick={() => (open = !open)}>
    <Factory size={15} />
    <span class="cur">{current ? current.name : "Pick a ship"}</span>
    <ChevronDown size={15} />
  </button>
  {#if open}
    <div class="pop" role="dialog" aria-label="Pick a ship">
      <label class="find">
        <Search size={14} />
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={q} placeholder="Find a ship or a role" aria-label="Find a ship" autofocus />
      </label>
      {#if flat}
        <div class="flat">
          <span class="fh"><span>Ship</span><span>Role</span><span>Crew</span><span>Cargo</span></span>
          {#each found as s (s.guid)}
            <button type="button" class="hull row" class:sel={s.guid === value} onclick={() => pick(s.guid)}>
              <span>{s.name}</span><small>{s.role ?? ""}</small><b title={crewTitle(s.seats)}>{s.seats.crew}</b><b>{s.cargo ? `${s.cargo.toLocaleString("en-US")} SCU` : "—"}</b>
            </button>
          {:else}
            <p class="none">No ship matches.</p>
          {/each}
        </div>
      {:else}
        <div class="cols">
          <div class="makers">
            {#each makers as m (m)}
              <button type="button" class="maker" class:sel={m === shown} onmouseenter={() => (maker = m)} onclick={() => (maker = m)}>
                <span>{m}</span><ChevronRight size={14} />
              </button>
            {/each}
          </div>
          <div class="hulls">
            {#each hulls as s (s.guid)}
              {@const img = shipsImages.byGuid.get(s.guid)}
              <button type="button" class="hull pic" class:sel={s.guid === value} onclick={() => pick(s.guid)}>
                {#if img}<img src={img} alt="" loading="lazy" />{:else}<span class="ph" style:--hue={careerLook(s.career).hue}></span>{/if}
                <span>{short(s)}</span><small>{s.role ?? ""}</small>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
  }
  .trigger {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 280px;
    padding: 7px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text-dim);
    cursor: pointer;
  }
  .trigger:hover,
  .trigger.on {
    border-color: var(--accent-border);
  }
  .cur {
    flex: 1;
    color: var(--text-strong);
    font-weight: var(--weight-medium);
  }
  /* The picker sits at the tab row's right end: the panel opens leftwards
   * from it, so it stays in the window. */
  .pop {
    position: absolute;
    z-index: 20;
    top: calc(100% + 6px);
    right: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    box-shadow: var(--shadow-panel);
    animation: rise 120ms var(--easing);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .find {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--text-faint);
  }
  .find input {
    all: unset;
    flex: 1;
    color: var(--text);
    font-size: var(--text-control);
  }
  .cols {
    display: grid;
    grid-template-columns: 250px 470px;
    gap: 6px;
    height: min(820px, calc(100vh - 220px));
  }
  .makers,
  .hulls,
  .flat {
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow-y: auto;
    min-height: 0;
  }
  .hulls {
    padding-left: 6px;
    border-left: 1px solid var(--border);
  }
  .flat {
    width: 726px;
    height: min(820px, calc(100vh - 220px));
  }
  .fh,
  .hull.row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 190px 50px 90px;
    gap: 10px;
  }
  .fh {
    padding: 2px 10px 4px;
    font-size: 0.68rem;
    font-weight: 600;
    color: var(--text-faint);
  }
  .fh span:nth-child(n + 3),
  .hull.row b {
    text-align: right;
  }
  .hull.row small {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hull.row b {
    font: 600 0.78rem var(--font-mono);
    color: var(--text-strong);
  }
  .maker,
  .hull {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 7px 10px;
    border-radius: var(--radius);
    color: var(--text);
    cursor: pointer;
  }
  .maker {
    color: var(--text-dim);
  }
  .maker:hover,
  .hull:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .maker.sel {
    background: var(--accent-fill-weak);
    color: var(--text-strong);
  }
  .hull.sel {
    background: var(--accent-fill-weak);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .hull.pic {
    justify-content: flex-start;
    padding: 4px 10px 4px 4px;
  }
  .hull.pic span {
    flex: 1;
  }
  .hull img,
  .ph {
    flex: none;
    width: 80px;
    height: 48px;
    border-radius: 5px;
    object-fit: cover;
  }
  .ph {
    background: linear-gradient(160deg, color-mix(in srgb, var(--hue) 22%, var(--bg-sunk)), var(--bg-sunk) 80%);
  }
  .hull span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hull small {
    flex: none;
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
  }
  .none {
    margin: 8px 10px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
