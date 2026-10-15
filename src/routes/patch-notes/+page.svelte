<script lang="ts">
  // Patch Notes — an app page, not a module's (decision 2026-10-15): LIVE
  // release notes are reference data from RSI's public patch-note articles,
  // gated by the online switch only, and Home reads them too. Modules add
  // sections while enabled (`ModuleDescriptor.patchNotes`), e.g. the dev
  // tracker's test-channel notes from Spectrum.
  //
  // The LIVE source (a patch-notes service over sc-dossier's parser) is not
  // wired yet; until then the page shows the module sections only.
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore } from "$lib/state/settings.svelte";

  const sections = $derived(
    moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) => (d.patchNotes ?? []).map((s) => ({ ...s, key: `${d.id}:${s.id}` }))),
  );
</script>

<div class="page">
  <section>
    <h2>LIVE releases</h2>
    <p class="dim">
      The LIVE release notes, with a short headline per release, come from RSI's patch-note
      articles. Not connected yet.
    </p>
  </section>

  {#each sections as s (s.key)}
    <section>
      <h2>{s.title}</h2>
      <s.component />
    </section>
  {/each}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 16px;
  }
</style>
