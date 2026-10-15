<script lang="ts">
  // The dev tracker before the opt-in (decision 2026-10-14): Spectrum is an
  // unofficial API, so the tracker stays off until the user allows it — and
  // says so instead of showing an empty feed.
  let { reason, compact = false }: { reason: string; compact?: boolean } = $props();
</script>

<div class="notice" class:compact>
  <p>
    The dev tracker reads CIG staff posts from Spectrum with your RSI launcher session. Spectrum
    is not an official API, so it stays off until you allow it: turn on
    <strong>Unofficial APIs</strong> and <strong>Dev tracker</strong> in
    <a href="/settings/online">Settings → Online</a>.
  </p>
  {#if !compact}<p class="dim">{reason}</p>{/if}
</div>

<style>
  .notice p {
    margin: 0 0 6px;
  }
  .notice:not(.compact) {
    max-width: 60ch;
  }
</style>
