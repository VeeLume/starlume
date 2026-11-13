<script lang="ts">
  // Settings → Game data: everything about where catalog data comes from —
  // the installs and their cached build, the startup warm, the build archive
  // (raw snapshots kept across patches for "what changed") and its
  // retention. The average user never needs this page: the startup warm and
  // the post-patch re-cook keep the catalogs current on their own.
  // Last, the running game's local telemetry stream: its opt-in, where the
  // connection stands, and the variable install that makes the game serve it.
  import { onMount } from "svelte";
  import { Button, ConfirmDialog, NumberInput, Settings, Switch } from "@veelume/ui";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import {
    dataStore,
    deleteBuild,
    loadBuilds,
    loadStatus,
    startLoad,
    wipe,
  } from "$lib/state/data.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import { SHOW_UNRELEASED } from "$lib/unreleased";
  import {
    installTelemetryEnv,
    telemetryStore,
    uninstallTelemetryEnv,
    watchTelemetry,
  } from "$lib/state/telemetry.svelte";

  const settings = $derived(settingsStore.current);
  let error = $state("");
  let confirmWipeAll = $state(false);
  /** The archived build awaiting delete confirmation. */
  let confirmDelete = $state<{ key: string; version: string } | null>(null);

  onMount(() => {
    void loadSettings();
    void loadStatus();
    void loadBuilds();
    return watchTelemetry();
  });

  // The connection in plain words. "no_stream" covers two cases the port
  // cannot tell apart: the game is still starting, or it runs without the
  // variable.
  const telemetryStatus = $derived(
    {
      off: "Off.",
      waiting_for_game: "Waiting for the game to start.",
      no_stream:
        "The game is running but not sending telemetry. It starts sending once it has loaded; if this stays, the game was started without the variable below.",
      connected: "Connected: the game is sending telemetry.",
    }[telemetryStore.status],
  );

  // What was last read, as proof the stream works (no consumer shows it yet).
  const telemetryRead = $derived.by(() => {
    const snap = telemetryStore.snapshot;
    if (!snap) return "";
    const parts: string[] = [];
    if (snap.balance != null) {
      parts.push(`${new Intl.NumberFormat().format(snap.balance)} aUEC`);
    }
    if (snap.vehicle) parts.push(snap.vehicle.class_name);
    return parts.join(" · ");
  });

  const telemetryEnvHint = $derived(
    {
      installed:
        "Installed. GRPC_CLIENT_SERVER_ENABLED=true is set for your Windows user, so the game sends telemetry however you start the launcher. Remove deletes it again.",
      set_elsewhere:
        "GRPC_CLIENT_SERVER_ENABLED=true is already set for your Windows user, not by Starlume. Nothing to install, and Starlume leaves it alone.",
      not_installed:
        "The game only sends telemetry when the launcher starts with GRPC_CLIENT_SERVER_ENABLED=true. Install sets that as an environment variable for your Windows user (not system-wide, no administrator rights). Without it, start the launcher from Starlume's Home page.",
    }[telemetryStore.env ?? "not_installed"],
  );

  let envBusy = $state(false);
  /** A launcher that already runs keeps its old environment. */
  let envChanged = $state(false);
  async function changeEnv(step: () => Promise<void>) {
    envBusy = true;
    await step();
    envBusy = false;
    envChanged = !telemetryStore.error;
  }

  async function apply(patch: Parameters<typeof applySettings>[0]) {
    error = "";
    const err = await applySettings(patch);
    if (err) error = err.message;
  }

  async function load(channel: string) {
    await startLoad(channel);
    await loadBuilds();
  }

  const tierNote = (tier: string) =>
    tier === "processed"
      ? "Cached — loads in about a second"
      : tier === "extract"
        ? "Snapshot only — about 20 s to rebuild"
        : "Not cached — the first load parses the game files";

  // Whole releases, at least one; an empty or invalid entry keeps the
  // current value (the input re-renders it on blur).
  function setReleases(next: number | null) {
    if (next == null || !Number.isFinite(next)) return;
    const releases = Math.max(1, Math.round(next));
    if (releases !== settings?.archive_releases) void apply({ archive_releases: releases });
  }

  const megabytes = (bytes: number) =>
    `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(bytes / 1048576)} MB`;

  const totalBytes = $derived(dataStore.builds.reduce((sum, b) => sum + b.bytes, 0));
</script>

<Settings.Page title="Game data">
  <Settings.Section title="Installs">
    {#each dataStore.statuses as status (status.channel)}
      {@const work = activityFor("data-load", status.channel)}
      <Settings.Row
        label={status.is_default ? `${status.channel} (default)` : status.channel}
        hint={status.loaded
          ? `${status.version} · ${status.item_count} items, ${status.mission_count} missions`
          : status.version}
      >
        <span class="flex flex-wrap items-center justify-end gap-2">
          {#if work}
            <span class="activity-note">⟳ {activityLabel(work)}</span>
          {:else if dataStore.starting[status.channel]}
            <span class="activity-note">Starting…</span>
          {:else}
            <span class="tier-note">{tierNote(status.predicted_tier)}</span>
            <Button variant="outline" onclick={() => void load(status.channel)}>
              {status.loaded ? "Reload" : "Load"}
            </Button>
            {#if status.predicted_tier !== "live"}
              <Button variant="ghost" onclick={() => void wipe(status.channel)}>Wipe</Button>
            {/if}
          {/if}
        </span>
      </Settings.Row>
    {:else}
      <p class="dim empty">
        {dataStore.statusLoaded
          ? "No Star Citizen installation found — installs are found through the RSI Launcher."
          : "Scanning installs…"}
      </p>
    {/each}
    {#if settings}
      <Settings.Row
        label="Load game data at startup"
        hint="Prepares the newest Live build in the background. Local only; the slow parse runs once per game patch."
      >
        <Switch
          label="Load game data at startup"
          checked={settings.auto_load_game_data}
          onchange={(v) => apply({ auto_load_game_data: v })}
        />
      </Settings.Row>
    {/if}
  </Settings.Section>

  {#if settings}
    <Settings.Section title="Build archive">
      <Settings.Row
        label="Releases to keep"
        hint="When a patch replaces a build, its raw snapshot (about 35 MB) is kept so changes between patches can be compared."
      >
        <NumberInput
          value={settings.archive_releases}
          onchange={setReleases}
          format={{ maximumFractionDigits: 0 }}
          class="w-20"
        />
      </Settings.Row>
      <Settings.Row
        label="Every build of a release"
        hint="Also keep earlier builds of the same release — PTU waves and hotfixes — not only the newest."
      >
        <Switch
          label="Every build of a release"
          checked={settings.archive_all_builds}
          onchange={(v) => apply({ archive_all_builds: v })}
        />
      </Settings.Row>
      <Settings.Row
        label="PTU and Tech Preview builds"
        hint="Archive test-channel builds too. Off: Live and Hotfix builds only."
      >
        <Switch
          label="PTU and Tech Preview builds"
          checked={settings.archive_test_channels}
          onchange={(v) => apply({ archive_test_channels: v })}
        />
      </Settings.Row>
    </Settings.Section>
  {/if}

  <Settings.Section title="Cached builds">
    {#each dataStore.builds as build (build.key)}
      <Settings.Row
        label={build.version}
        hint={`${build.channels.join(", ")} · ${build.current ? "installed" : "archived"}`}
      >
        <span class="flex items-center justify-end gap-2">
          <span class="tier-note">{megabytes(build.bytes)}</span>
          <!-- Installed builds are wiped from their install row above. -->
          {#if !build.current}
            <Button
              variant="ghost"
              onclick={() => (confirmDelete = { key: build.key, version: build.version })}
            >
              Delete
            </Button>
          {/if}
        </span>
      </Settings.Row>
    {:else}
      <p class="dim empty">
        {dataStore.buildsLoaded ? "Nothing cached yet." : "Reading the cache…"}
      </p>
    {/each}
    <Settings.Row
      label="Clear the cache"
      hint={`${megabytes(totalBytes)} in total. Deletes every cached build, the archive included; the next load parses the game files again.`}
    >
      <Button variant="outline" onclick={() => (confirmWipeAll = true)}>Clear</Button>
    </Settings.Row>
  </Settings.Section>

  <!-- Unreleased: nothing shows what is read yet (wallet, session log). -->
  {#if settings && SHOW_UNRELEASED}
    <Settings.Section title="Live telemetry">
      <Settings.Row
        label="Read the game's telemetry"
        hint="The running game can report your ship, your vitals and your aUEC balance to programs on this PC. Starlume reads it from this PC only (127.0.0.1); nothing is sent anywhere, so it works with online features off."
      >
        <Switch
          label="Read the game's telemetry"
          checked={settings.telemetry_enabled}
          onchange={(v) => apply({ telemetry_enabled: v })}
        />
      </Settings.Row>
      {#if settings.telemetry_enabled}
        <Settings.Row label="Status" hint={telemetryStatus}>
          <span class="tier-note">{telemetryRead}</span>
        </Settings.Row>
      {/if}
      <!-- Shown with the opt-in off too once installed: it must stay removable. -->
      {#if telemetryStore.env && (settings.telemetry_enabled || telemetryStore.env === "installed")}
        <Settings.Row label="Make the game send it" hint={telemetryEnvHint}>
          {#if telemetryStore.env === "installed"}
            <Button
              variant="outline"
              disabled={envBusy}
              onclick={() => void changeEnv(uninstallTelemetryEnv)}
            >
              Remove
            </Button>
          {:else if telemetryStore.env === "not_installed"}
            <Button
              variant="outline"
              disabled={envBusy}
              onclick={() => void changeEnv(installTelemetryEnv)}
            >
              Install
            </Button>
          {:else}
            <span class="tier-note">Already set</span>
          {/if}
        </Settings.Row>
        {#if envChanged}
          <p class="dim empty">
            A launcher that is already running keeps its old settings: quit it from its tray
            icon and start it again for this to take effect.
          </p>
        {/if}
      {/if}
    </Settings.Section>
  {/if}

  {#if error || dataStore.error || telemetryStore.error}
    <p class="text-sm text-destructive">{error || dataStore.error || telemetryStore.error}</p>
  {/if}
</Settings.Page>

<ConfirmDialog
  open={confirmWipeAll}
  title="Clear the game-data cache?"
  description="Every cached build and the whole archive are deleted. Archived builds cannot be recovered — the game no longer has them."
  confirmLabel="Clear cache"
  destructive
  onclose={() => (confirmWipeAll = false)}
  onconfirm={async () => {
    await wipe(null);
    confirmWipeAll = false;
  }}
/>

<ConfirmDialog
  open={confirmDelete !== null}
  title={`Delete ${confirmDelete?.version ?? "this build"}?`}
  description="The archived snapshot is deleted. It cannot be recovered — the game no longer has this build — so changes against it can no longer be compared."
  confirmLabel="Delete build"
  destructive
  onclose={() => (confirmDelete = null)}
  onconfirm={async () => {
    if (confirmDelete) await deleteBuild(confirmDelete.key);
    confirmDelete = null;
  }}
/>

<style>
  .empty {
    font-size: 0.85rem;
    padding: 8px 0;
  }
  .tier-note {
    font-size: 0.78rem;
    color: var(--text-dim);
  }
</style>
