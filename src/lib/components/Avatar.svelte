<script lang="ts">
  // Round accent-gradient badge (the Hearth pattern) — or the user's Discord
  // avatar when an image URL is available.
  let {
    text,
    src = null,
    size = "1.9rem",
    muted = false,
    title,
  }: {
    /** Fallback glyph when there's no image (or it fails to load). */
    text: string;
    /** Avatar image URL (Discord CDN). */
    src?: string | null;
    /** Diameter, any CSS length. The glyph is sized at ~45% of it. */
    size?: string;
    /** Grey variant (signed-out / placeholder states). */
    muted?: boolean;
    title?: string;
  } = $props();

  let broken = $state(false);
  $effect(() => {
    src; // reset the error state when the URL changes
    broken = false;
  });
</script>

<span class="avatar" class:muted style="--avatar-size: {size}" {title}>
  {#if src && !broken}
    <img {src} alt="" onerror={() => (broken = true)} />
  {:else}
    {text}
  {/if}
</span>

<style>
  .avatar {
    width: var(--avatar-size);
    height: var(--avatar-size);
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    border-radius: 50%;
    overflow: hidden;
    background: linear-gradient(135deg, var(--accent), var(--accent-dim));
    color: var(--on-accent);
    font-weight: 700;
    font-size: calc(var(--avatar-size) * 0.45);
    user-select: none;
  }
  .avatar.muted {
    background: var(--bg-raised);
    color: var(--text-dim);
    border: 1px solid var(--border);
  }
  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
