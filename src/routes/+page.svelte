<script lang="ts">
  // Home — a grid of cards answering "can I play?" and "what is going on?".
  // The shell and the enabled modules contribute cards ($lib/home/cards);
  // the user orders and hides them in customize mode, saved Rust-side as
  // `AppSettings.home_cards` (frontend rule 6). Cards read stores only:
  // Home fetches nothing and starts no polling.
  import { Button } from "@veelume/ui";
  import { ArrowRight, ChevronDown, ChevronUp, Eye, EyeOff, SlidersHorizontal } from "lucide-svelte";
  import { settingsStore, applySettings } from "$lib/state/settings.svelte";
  import { availableCards, type AvailableCard } from "$lib/home/cards";
  import { moveCard, resolveLayout, toPrefs, type Placed } from "$lib/home/layout";

  const prefs = $derived(settingsStore.current?.home_cards ?? []);
  const cards = $derived(
    settingsStore.current ? resolveLayout(availableCards(settingsStore.current.enabled_modules), prefs) : [],
  );
  const shown = $derived(cards.filter((c) => c.visible));

  let customizing = $state(false);
  let error = $state("");

  async function save(next: Placed<AvailableCard>[]) {
    const err = await applySettings({ home_cards: toPrefs(next, prefs) });
    error = err ? err.message : "";
  }

  const move = (i: number, delta: -1 | 1) => void save(moveCard(cards, i, delta));
  const toggle = (i: number) => void save(cards.map((c, j) => (j === i ? { ...c, visible: !c.visible } : c)));
  // Forget the arrangement, including cards of disabled modules.
  const reset = () => void applySettings({ home_cards: [] });
</script>

<div class="home">
  <header class="bar">
    {#if customizing}
      <span class="dim">Arrange and hide cards. Changes save as you go.</span>
      <span class="grow"></span>
      <Button variant="ghost" onclick={reset}>Reset</Button>
      <Button onclick={() => (customizing = false)}>Done</Button>
    {:else}
      <span class="grow"></span>
      <Button variant="ghost" onclick={() => (customizing = true)}>
        <SlidersHorizontal size={15} /> Customize
      </Button>
    {/if}
  </header>

  {#if error}<p class="error">{error}</p>{/if}

  {#if settingsStore.current && !customizing && shown.length === 0}
    <p class="dim">Every card is hidden. Use Customize to bring some back.</p>
  {/if}

  {#snippet controls(card: Placed<AvailableCard>, i: number)}
    {#if card.ownerName !== card.title && !card.key.startsWith("shell:")}
      <span class="module-tag">{card.ownerName}</span>
    {/if}
    <Button variant="ghost" size="icon" title="Move up" disabled={i === 0} onclick={() => move(i, -1)}>
      <ChevronUp size={16} />
    </Button>
    <Button
      variant="ghost"
      size="icon"
      title="Move down"
      disabled={i === cards.length - 1}
      onclick={() => move(i, 1)}
    >
      <ChevronDown size={16} />
    </Button>
    <Button variant="ghost" size="icon" title={card.visible ? "Hide" : "Show"} onclick={() => toggle(i)}>
      {#if card.visible}<Eye size={16} />{:else}<EyeOff size={16} />{/if}
    </Button>
  {/snippet}

  <div class="grid" class:customizing>
    {#each customizing ? cards : shown as card, i (card.key)}
      {#if card.frameless && (card.visible || !customizing)}
        <!-- Draws its own frame and heading; Home adds only the controls. -->
        <div class="frameless" class:wide={card.wide}>
          {#if customizing}<div class="overlay">{@render controls(card, i)}</div>{/if}
          <card.component />
        </div>
      {:else}
      <section class="home-card" class:wide={card.wide} class:hidden-card={!card.visible}>
        <header>
          <h2>{card.title}</h2>
          {#if !customizing && card.badge?.()}<span class="hue-chip badge-count">{card.badge()}</span>{/if}
          {#if customizing}
            <span class="grow"></span>
            {@render controls(card, i)}
          {:else if card.href}
            <span class="grow"></span>
            <a class="open" href={card.href}>Open <ArrowRight size={13} /></a>
          {/if}
        </header>
        {#if !customizing || card.visible}
          <card.component />
        {/if}
      </section>
      {/if}
    {/each}
  </div>
</div>

<style>
  .home {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .bar,
  .home-card header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .bar {
    min-height: 36px;
  }
  .grow {
    flex: 1;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 12px;
    align-items: start;
  }
  .home-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .home-card,
  .frameless {
    position: relative;
  }
  .home-card {
    padding: 14px 16px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
  }
  .wide {
    grid-column: 1 / -1;
  }
  /* Customize: every card outlined, its controls in place. */
  .customizing .home-card,
  .customizing .frameless {
    outline: 1px dashed color-mix(in srgb, var(--accent) 45%, transparent);
    outline-offset: 3px;
  }
  .overlay {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--bg) 80%, transparent);
  }
  .badge-count {
    --hue: var(--accent);
  }
  .home-card.hidden-card {
    opacity: 0.55;
  }
  .home-card h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 0.75rem;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .module-tag {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .open {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--text-small);
    color: var(--accent);
    text-decoration: none;
  }
  .open:hover {
    text-decoration: underline;
  }
</style>
