<script lang="ts">
  // Persistent notification center — the "look at them after the fact"
  // surface (the Hearth pattern + a source tag per item). Lists every
  // notification this session with timestamp, severity, source and an
  // optional action. Opening it (from the sidebar bell) marks everything
  // read; the bell badge is the unread signal.
  import {
    notifications,
    dismiss,
    clearAll,
    type NotifLevel,
  } from "$lib/state/notifications.svelte";

  let { open = false, onClose }: { open?: boolean; onClose: () => void } = $props();

  const glyph: Record<NotifLevel, string> = {
    info: "•",
    success: "✓",
    warning: "!",
    error: "✕",
  };

  function relTime(ts: number): string {
    const s = Math.floor((Date.now() - ts) / 1000);
    if (s < 5) return "just now";
    if (s < 60) return `${s}s ago`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    return `${Math.floor(h / 24)}d ago`;
  }
</script>

{#if open}
  <button class="overlay" onclick={onClose} aria-label="Close notifications"></button>
  <div class="panel" role="dialog" aria-label="Notifications">
    <header>
      <span class="head-title">Notifications</span>
      {#if notifications.items.length > 0}
        <button class="link" onclick={clearAll}>Clear all</button>
      {/if}
    </header>

    {#if notifications.items.length === 0}
      <p class="empty">Nothing yet.</p>
    {:else}
      <ul>
        {#each notifications.items as n (n.id)}
          <li class:unread={!n.read}>
            <span class={`dot ${n.level}`}>{glyph[n.level]}</span>
            <div class="body">
              <span class="n-title">{n.title}</span>
              {#if n.body}<span class="n-body">{n.body}</span>{/if}
              <span class="n-meta">
                <span class="time">{relTime(n.ts)}</span>
                {#if n.source}<span class="source">{n.source}</span>{/if}
                {#if n.action}
                  <a class="action" href={n.action.href} onclick={onClose}>
                    {n.action.label} →
                  </a>
                {/if}
              </span>
            </div>
            <button class="x" onclick={() => dismiss(n.id)} aria-label="Dismiss">×</button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .panel {
    position: fixed;
    top: 0;
    left: 200px;
    z-index: 71;
    width: 340px;
    max-height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 0 0 12px 0;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.5);
    animation: panel-in 140ms ease-out;
  }
  @keyframes panel-in {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 0.85rem;
    border-bottom: 1px solid var(--border);
  }
  .head-title {
    font-size: 0.9rem;
    font-weight: 600;
  }
  .link {
    background: transparent;
    border: none;
    color: var(--text-dim);
    font-size: 0.72rem;
    cursor: pointer;
    padding: 0;
  }
  .link:hover {
    color: var(--text);
  }
  .empty {
    padding: 1.4rem 0.85rem;
    margin: 0;
    color: var(--text-dim);
    font-size: 0.82rem;
    text-align: center;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    padding: 0.65rem 0.85rem;
    border-bottom: 1px solid var(--border);
  }
  li.unread {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .dot {
    flex: 0 0 auto;
    width: 1.2rem;
    text-align: center;
    font-weight: 700;
    color: var(--text-dim);
  }
  .dot.success {
    color: var(--good);
  }
  .dot.warning {
    color: var(--warn);
  }
  .dot.error {
    color: var(--bad);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
    flex: 1;
  }
  .n-title {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text);
  }
  .n-body {
    font-size: 0.74rem;
    color: var(--text-dim);
    word-break: break-word;
  }
  .n-meta {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-top: 0.1rem;
  }
  .time {
    font-size: 0.68rem;
    color: var(--text-dim);
  }
  .source {
    font-size: 0.62rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
    border: 1px solid var(--border);
    padding: 0.02rem 0.3rem;
    border-radius: 4px;
  }
  .action {
    font-size: 0.7rem;
    color: var(--accent);
    text-decoration: none;
  }
  .action:hover {
    text-decoration: underline;
  }
  .x {
    flex: 0 0 auto;
    background: transparent;
    border: none;
    color: var(--text-dim);
    cursor: pointer;
    font-size: 1rem;
    line-height: 1;
    padding: 0 0.15rem;
  }
  .x:hover {
    color: var(--text);
  }
</style>
