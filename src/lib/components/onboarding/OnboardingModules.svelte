<script lang="ts">
  // Onboarding: the module picker as a list and a sheet (style guide,
  // "Surfaces and layout"). Each row has its switch and what the module
  // needs; the sheet beside it says what you get, what it needs and where
  // it shows up, for the row in focus. All from the registry: a new module
  // appears here with no edit to this file.
  import { onMount } from "svelte";
  import { Switch } from "@veelume/ui";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import { moduleRegistry } from "$lib/modules/registry";
  import type { ModuleDescriptor } from "$lib/modules/types";
  import { scStore } from "$lib/state/sc.svelte";

  let { selected, ontoggle }: { selected: Set<string>; ontoggle: (id: string) => void } = $props();

  let focus = $state(moduleRegistry[0]?.id ?? "");
  let features = $state<GrpcFeatureInfo[]>([]);
  onMount(async () => {
    features = await commands.listGrpcFeatures();
  });

  const current = $derived(moduleRegistry.find((m) => m.id === focus) ?? moduleRegistry[0]);
  const hasInstall = $derived((scStore.status?.installs.length ?? 0) > 0);
  const featureName = (id: string) => features.find((f) => f.id === id)?.name ?? id;

  function showsUp(m: ModuleDescriptor): string {
    const parts = [
      ...(m.nav ?? []).map((n) => `${n.label} in the sidebar`),
      ...(m.homeCards ?? []).map((c) => `a ${c.title} card on Home`),
      ...(m.installStatus ? ["a check on Home's game status"] : []),
    ];
    return parts.length ? parts.join(", ") : "Its page";
  }
</script>

{#if moduleRegistry.length === 0}
  <p class="dim">No feature modules in this build yet.</p>
{:else}
  <div class="mods">
    <div class="list">
      {#each moduleRegistry as m (m.id)}
        {@const Icon = m.nav?.[0]?.icon}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="row" class:focus={m.id === current?.id} onclick={() => (focus = m.id)}>
          <span class="icon">{#if Icon}<Icon size={17} />{/if}</span>
          <span class="nm">
            <b>{m.name}</b>
            <span class="line">{m.description}</span>
            <span class="needs">
              {#if m.needsGame}<span class="need">game install</span>{/if}
              {#if m.unofficial?.some((u) => u.required)}
                <span class="need u">unofficial API</span>
              {:else if m.unofficial?.length}
                <span class="need">unofficial API, optional</span>
              {/if}
            </span>
          </span>
          <span onclick={(e) => e.stopPropagation()}>
            <Switch label={m.name} checked={selected.has(m.id)} onchange={() => ontoggle(m.id)} />
          </span>
        </div>
      {/each}
      <p class="count">{selected.size} of {moduleRegistry.length} on. Settings → Modules changes this later.</p>
    </div>

    {#if current}
      {@const Icon = current.nav?.[0]?.icon}
      <aside class="sheet">
        <div class="sheet-h">
          <span class="icon big">{#if Icon}<Icon size={20} />{/if}</span>
          <div>
            <h3>{current.name}</h3>
            <p>{current.description}</p>
          </div>
        </div>
        <div class="sec">
          <p class="label">What you get</p>
          <ul class="pitch">
            {#each current.pitch as p (p)}<li>{p}</li>{/each}
          </ul>
        </div>
        <div class="sec">
          <p class="label">Needs</p>
          <div class="req">
            {#if current.needsGame}
              {#if hasInstall}
                <span class="ok">✓</span><span>Your Star Citizen install</span>
              {:else}
                <span class="faint">–</span><span>A Star Citizen install<small>Not found. The module waits for one.</small></span>
              {/if}
            {/if}
            {#each current.unofficial ?? [] as u (u.feature)}
              <span class="u">!</span>
              <span>{featureName(u.feature)}{u.required ? "" : " (optional)"}<small>An unofficial API, asked on Online & privacy.</small></span>
            {/each}
            {#if !current.needsGame && !current.unofficial?.length}
              <span class="ok">✓</span><span>Nothing</span>
            {/if}
          </div>
        </div>
        <div class="sec">
          <p class="label">Shows up as</p>
          <span class="dim">{showsUp(current)}</span>
        </div>
      </aside>
    {/if}
  </div>
{/if}

<style>
  .mods {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(300px, 380px);
    gap: 16px;
    align-items: start;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    padding: 10px 12px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background-color var(--transition-fast) var(--easing);
  }
  .row:hover {
    background: var(--bg-raised);
  }
  .row.focus {
    border-color: var(--accent-border);
    background: color-mix(in srgb, var(--accent) 7%, var(--bg-raised));
  }
  .icon {
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    display: grid;
    place-items: center;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text-dim);
  }
  .icon.big {
    width: 40px;
    height: 40px;
  }
  .nm {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .nm b {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .line {
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .needs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .need {
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    padding: 1px 6px;
    border-radius: var(--radius-chip);
    border: 1px solid var(--border);
    color: var(--text-faint);
  }
  .need.u {
    color: color-mix(in srgb, var(--warn) 80%, var(--text));
    border-color: color-mix(in srgb, var(--warn) 30%, transparent);
  }
  .count {
    margin: 0;
    padding: 4px 2px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }

  .sheet {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px 18px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
  }
  .sheet-h {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr);
    gap: 12px;
    align-items: start;
  }
  .sheet-h h3 {
    margin: 0;
    font-size: 1.1rem;
    color: var(--text-strong);
  }
  .sheet-h p {
    margin: 2px 0 0;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .sec {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .sec > .label {
    margin: 0;
  }
  .req {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    gap: 6px 8px;
  }
  .req small {
    display: block;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .ok {
    color: var(--good);
  }
  .u {
    color: var(--warn);
  }
  .faint {
    color: var(--text-faint);
  }
  @media (max-width: 900px) {
    .mods {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
