<script lang="ts">
  // What a catalog page shows while it has no data to browse. While the
  // data is being prepared (startup warm, post-patch cook, a cook revision
  // bump) it is a card in the channel's hue: the build, why, and the load's
  // stages, the running one with the time so far — a multi-minute parse
  // must never read as a hang. The page fills itself when the cook lands
  // (`data:changed`). With nothing running it points at Settings → Game
  // data, where loading is managed; `waiting` (the page has a channel, its
  // data is on the way) shows only a faint outline instead. Nothing shows
  // for the first 400 ms, so a cached load never flashes it.
  import { onMount } from "svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import { activityFor, activityLabel, activityStore } from "$lib/state/activities.svelte";
  import { dataStore } from "$lib/state/data.svelte";

  let { waiting = false }: { waiting?: boolean } = $props();

  /** The load's stages in order, by their labels in data.rs `stage_label`. */
  const STAGES: [string, string][] = [
    ["Checking snapshots…", "Saved data"],
    ["Opening Data.p4k…", "Game files"],
    ["Extracting assets…", "Assets"],
    ["Parsing DataCore… (the long part)", "DataCore, the long part"],
    ["Building indices…", "Catalogs, rock tables, places"],
    ["Saving snapshots…", "Saving"],
  ];
  const DELAY_MS = 400;

  let shown = $state(false);
  let now = $state(Date.now());
  onMount(() => {
    const t = setTimeout(() => (shown = true), DELAY_MS);
    const tick = setInterval(() => (now = Date.now()), 1000);
    return () => {
      clearTimeout(t);
      clearInterval(tick);
    };
  });

  const preparing = $derived(
    dataStore.channel
      ? activityFor("data-load", dataStore.channel)
      : activityStore.items.find((a) => a.kind === "data-load"),
  );
  const status = $derived(
    preparing?.channel ? dataStore.statuses.find((s) => s.channel.toLowerCase() === preparing.channel) : undefined,
  );
  const stage = $derived.by(() => {
    const phase = preparing?.phase;
    return phase?.type === "running" ? STAGES.findIndex(([label]) => label === phase.stage) : -1;
  });
  const reason = $derived(
    status?.predicted_tier === "processed"
      ? "Loading the saved data."
      : status?.predicted_tier === "extract"
        ? "Rebuilding from the saved extract."
        : "Reading the game files once for this build.",
  );
  const elapsed = $derived.by(() => {
    if (!preparing) return "";
    const s = Math.max(0, Math.floor((now - preparing.started) / 1000));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  });
</script>

{#if shown}
  {#if preparing}
    <div class="wait">
      <div class="card-row">
        <NowCard
          channel={status?.channel ?? preparing.channel ?? undefined}
          chip={status ? `● ${status.channel.toUpperCase()}` : undefined}
          version="Preparing game data"
          date={status?.version ?? ""}
        >
          <p class="dim reason">{preparing.phase.type === "waiting" ? preparing.phase.reason : reason}</p>
          <div class="indet" aria-hidden="true"></div>
          {#if stage >= 0}
            <ul class="stages">
              {#each STAGES as [, label], i (label)}
                <li class:done={i < stage} class:todo={i > stage}>
                  <span class="dot" class:run={i === stage}></span>
                  <span>{label}</span>
                  <span class="n">{i === stage ? elapsed : ""}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="dim now">{activityLabel(preparing)} <span class="n">{elapsed}</span></p>
          {/if}
        </NowCard>
      </div>
      <div class="skel" aria-hidden="true"><i></i><i></i><i></i><i></i></div>
    </div>
  {:else if waiting}
    <div class="skel" aria-hidden="true"><i></i><i></i><i></i><i></i></div>
  {:else}
    <p class="dim">
      No game data loaded yet — load it in <a href="/settings/game-data">Settings → Game data</a>.
    </p>
  {/if}
{/if}

<style>
  .wait {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .card-row {
    display: flex;
    max-width: 640px;
  }
  .reason,
  .now {
    margin: 0;
  }
  .indet {
    position: relative;
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }
  .indet::after {
    content: "";
    position: absolute;
    top: 0;
    height: 100%;
    width: 30%;
    border-radius: 2px;
    background: var(--hue);
    animation: slide 1.4s ease-in-out infinite;
  }
  .stages {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  li {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 8px 0;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    font-size: 0.9rem;
  }
  li.done {
    color: var(--text-dim);
  }
  li.todo {
    color: var(--text-faint);
  }
  .dot {
    justify-self: center;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid var(--border);
  }
  li.done .dot {
    border: 0;
    background: var(--good);
  }
  .dot.run {
    width: 12px;
    height: 12px;
    border-color: var(--hue);
    border-right-color: transparent;
    animation: spin 0.9s linear infinite;
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--text-faint);
  }
  .skel {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    gap: 16px;
    opacity: 0.3;
    pointer-events: none;
  }
  .skel i {
    display: block;
    height: 200px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes slide {
    from {
      left: -30%;
    }
    to {
      left: 100%;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dot.run,
    .indet::after {
      animation: none;
    }
  }
  @media (max-width: 760px) {
    .skel {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
