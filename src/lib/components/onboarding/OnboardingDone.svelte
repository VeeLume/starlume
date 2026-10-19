<script lang="ts">
  // Onboarding: where things are. The sidebar the chosen modules give, what
  // the user allowed, how Starlume runs, and that later changes come as a
  // short guide (the What's-new page, Settings → About).
  import { onMount } from "svelte";
  import { House, Newspaper, Sparkles } from "lucide-svelte";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import type { ModuleDescriptor } from "$lib/modules/types";
  import { settingsStore } from "$lib/state/settings.svelte";

  let { modules }: { modules: ModuleDescriptor[] } = $props();

  const settings = $derived(settingsStore.current);
  let features = $state<GrpcFeatureInfo[]>([]);
  onMount(async () => {
    features = await commands.listGrpcFeatures();
  });

  const nav = $derived([
    { label: "Home", icon: House },
    { label: "News", icon: Newspaper },
    ...modules.flatMap((m) => m.nav ?? []),
  ]);
  const allowed = $derived(
    settings?.online_enabled && settings.grpc_enabled
      ? features.filter((f) => settings.grpc_features.includes(f.id)).map((f) => f.name)
      : [],
  );
</script>

{#if settings}
  <div class="done">
    <div class="sum">
      <div>
        <p class="label">In the sidebar</p>
        {#each nav as n (n.label)}
          {@const Icon = n.icon}
          <span class="nav"><Icon size={16} />{n.label}</span>
        {/each}
      </div>
      <div>
        <p class="label">Online</p>
        <ul>
          <li>{settings.online_enabled ? "Online features on" : "Offline"}</li>
          {#each allowed as a (a)}<li>{a}</li>{:else}<li>No unofficial APIs</li>{/each}
        </ul>
      </div>
      <div>
        <p class="label">Background</p>
        <ul>
          <li>{settings.autostart ? "Starts with Windows" : "Starts when you open it"}</li>
          <li>{settings.close_to_tray ? "Closes to the tray" : "Closing quits"}</li>
          <li>{settings.native_notifications ? "Notifications on" : "Notifications off"}</li>
        </ul>
      </div>
    </div>
    <p class="next">
      <Sparkles size={18} />
      <span>
        When an update brings a new module or changes something you set, Starlume shows a short
        guide like this one. Settings → About lists them.
      </span>
    </p>
  </div>
{/if}

<style>
  .done {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .sum {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
  }
  .sum > div {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
  }
  .sum .label {
    margin: 0;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-dim);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--text-dim);
  }
  .next {
    margin: 0;
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr);
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--accent) 7%, var(--bg-raised));
    border: 1px solid color-mix(in srgb, var(--accent) 25%, var(--border));
    color: var(--text-dim);
  }
  .next :global(svg) {
    color: var(--accent);
  }
</style>
