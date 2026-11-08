<script lang="ts">
  // Settings → About: the version and the update check, every release's
  // What's-new guide, running the setup again, and where the data comes from.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Button, Settings } from "@veelume/ui";
  import { commands } from "$lib/bindings";
  import { RELEASE_GUIDES } from "$lib/guides";
  import { compareVersions } from "$lib/home/version";
  import { guide, loadVersion } from "$lib/state/guide.svelte";
  import { loadSettings } from "$lib/state/settings.svelte";
  import { openOnboarding } from "$lib/state/onboarding.svelte";
  import { checkForUpdates } from "$lib/updater";

  /** Each URL is on the allow-list in `window.rs`. */
  const CREDITS = [
    {
      label: "UEX",
      hint: "In-game shop, rental and player-market prices, reported by the players on UEX.",
      action: "uexcorp.space",
      url: "https://uexcorp.space",
    },
    {
      label: "Roberts Space Industries",
      hint: "Ship pictures and pledge prices, from the public ship store. Everything else is read from your own game install.",
      action: "robertsspaceindustries.com",
      url: "https://robertsspaceindustries.com",
    },
    {
      label: "Starlume",
      hint: "The source code and the releases.",
      action: "GitHub",
      url: "https://github.com/VeeLume/starlume",
    },
  ];

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

  <Settings.Section
    title="Credits"
    description="Starlume is a fan-made tool. It is not affiliated with or endorsed by Cloud Imperium Games. Star Citizen® and Squadron 42® are trademarks of Cloud Imperium Rights LLC."
  >
    {#each CREDITS as c (c.url)}
      <Settings.Row label={c.label} hint={c.hint}>
        <Button variant="outline" onclick={() => void commands.openCreditLink(c.url)}>{c.action}</Button>
      </Settings.Row>
    {/each}
  </Settings.Section>
</Settings.Page>
