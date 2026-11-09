<script lang="ts">
  // Settings → Performance: what Starlume has claimed from the system right
  // now. Page-local on purpose: a sample means nothing seconds later, so
  // there is nothing for a store to keep (data-lifecycle rule 1).
  import { onMount } from "svelte";
  import { Settings } from "@veelume/ui";
  import { commands, type MemoryView } from "$lib/bindings";

  const POLL_MS = 2000;

  let memory = $state<MemoryView | null>(null);

  async function sample() {
    const r = await commands.performanceMemory();
    if (r.status === "ok") memory = r.data;
  }

  onMount(() => {
    void sample();
    const timer = setInterval(sample, POLL_MS);
    return () => clearInterval(timer);
  });

  const total = $derived(memory ? memory.appMb + memory.webviewMb : 0);
  const heap = $derived(memory ? memory.heapUsedMb + memory.heapHeldMb : 0);

  const mb = (n: number | undefined) => (n === undefined ? "–" : `${n.toLocaleString("en-US")} MB`);
  const share = (part: number, whole: number) => (whole ? `${(part / whole) * 100}%` : "0%");
</script>

<Settings.Page title="Performance">
  <Settings.Section title="Memory" description="What Starlume has claimed right now. This updates while the page is open.">
    <div class="bar" role="presentation">
      <span class="main" style:width={share(memory?.appMb ?? 0, total)}></span>
      <span class="rest"></span>
    </div>
    <Settings.Row label="Total"><span class="num strong">{mb(memory ? total : undefined)}</span></Settings.Row>
    <Settings.Row label="App" hint="Game data, ship holos and background work.">
      <span class="num">{mb(memory?.appMb)}</span>
    </Settings.Row>
    <Settings.Row
      label="Window"
      hint={memory ? `The web view that draws Starlume, ${memory.webviewProcesses} processes.` : "The web view that draws Starlume."}
    >
      <span class="num">{mb(memory?.webviewMb)}</span>
    </Settings.Row>
  </Settings.Section>

  <Settings.Section
    title="Inside the app"
    description="Memory the app has finished with stays reserved for a while, and its next job reuses it."
  >
    <div class="bar" role="presentation">
      <span class="main" style:width={share(memory?.heapUsedMb ?? 0, heap)}></span>
      <span class="rest"></span>
    </div>
    <Settings.Row label="In use"><span class="num">{mb(memory?.heapUsedMb)}</span></Settings.Row>
    <Settings.Row label="Finished with, still reserved"><span class="num">{mb(memory?.heapHeldMb)}</span></Settings.Row>
  </Settings.Section>
</Settings.Page>

<style>
  .bar {
    display: flex;
    width: 100%;
    height: 8px;
    overflow: hidden;
    border-radius: var(--radius-chip);
  }
  .main {
    background: var(--accent);
    transition: width 240ms var(--easing);
  }
  .rest {
    flex: 1;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .num {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
  }
  .strong {
    color: var(--text-strong);
  }
</style>
