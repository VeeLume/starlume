<script lang="ts">
  // Settings → About: the version and the update check, every release's
  // What's-new guide, and running the setup again.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Button, Settings } from "@veelume/ui";
  import { RELEASE_GUIDES } from "$lib/guides";
  import { compareVersions } from "$lib/home/version";
  import { guide, loadVersion } from "$lib/state/guide.svelte";
  import { loadSettings } from "$lib/state/settings.svelte";
  import { openOnboarding } from "$lib/state/onboarding.svelte";
  import { checkForUpdates } from "$lib/updater";

  onMount(() => {
    void loadSettings();
    void loadVersion();
  });

  const releases = $derived(
    guide.version ? RELEASE_GUIDES.filter((g) => compareVersions(g.version, guide.version!) <= 0) : [],
  );
</script>

<Settings.Page title="About">
  <Settings.Section>
    <Settings.Row label={`Starlume ${guide.version ?? ""}`} hint="Update checks run at startup.">
      <Button variant="outline" onclick={() => checkForUpdates(true)}>Check for updates</Button>
    </Settings.Row>
  </Settings.Section>

  <Settings.Section title="What's new">
    {#if guide.pending}
      <Settings.Row label="Not seen yet" hint={guide.newModules.map((m) => m.name).join(", ") || "Changes in this release"}>
        <Button variant="primary" onclick={() => goto("/whats-new")}>Open</Button>
      </Settings.Row>
    {/if}
    {#each releases as r (r.version)}
      <Settings.Row label={`What's new in ${r.version}`} hint={r.changes.map((c) => c.title).join(", ")}>
        <Button variant="outline" onclick={() => goto(`/whats-new?v=${r.version}`)}>Open</Button>
      </Settings.Row>
    {/each}
  </Settings.Section>

  <Settings.Section>
    <Settings.Row label="Setup" hint="Modules, privacy and the tray, from the start.">
      <Button variant="outline" onclick={openOnboarding}>Run again</Button>
    </Settings.Row>
  </Settings.Section>
</Settings.Page>
