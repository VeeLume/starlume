<script lang="ts">
  // A solve under way where its chart will be: a band sweeping across, and
  // what it waits for — measuring now, or queued behind others.
  let { height = 24, label = "" }: { height?: number; label?: string } = $props();
</script>

<span class="pend" style:height="{height}px" role="status"><i></i>{#if label}<small>{label}</small>{/if}</span>

<style>
  .pend {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
    border-radius: 4px;
    overflow: hidden;
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  i {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent) 16%, transparent), transparent);
    transform: translateX(-100%);
    animation: sweep 1.4s ease-in-out infinite;
  }
  small {
    position: relative;
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  @keyframes sweep {
    to {
      transform: translateX(100%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    i {
      animation: none;
      transform: none;
      opacity: 0.4;
    }
  }
</style>
