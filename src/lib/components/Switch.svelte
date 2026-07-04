<script lang="ts">
  import type { Snippet } from "svelte";

  // Toggle slider — the Hearth pattern. Amber-filled pill track with a sliding
  // knob; use for on/off preferences (Settings). For inline toolbar filter
  // toggles keep the .check checkbox instead (docs/frontend.md).
  interface Props {
    checked?: boolean;
    disabled?: boolean;
    id?: string;
    /** Receives the NEXT boolean, not the event. */
    onchange?: (next: boolean) => void;
    /** Optional label rendered to the right of the track. */
    children?: Snippet;
  }

  let { checked = false, disabled = false, id, onchange, children }: Props = $props();

  function toggle() {
    if (disabled) return;
    onchange?.(!checked);
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  {id}
  {disabled}
  class="switch"
  class:on={checked}
  onclick={toggle}
>
  <span class="track"><span class="knob"></span></span>
  {#if children}<span class="switch-label">{@render children()}</span>{/if}
</button>

<style>
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    background: transparent;
    border: none;
    border-radius: 0;
    padding: 0;
    margin: 0;
    font-family: var(--font-sans);
    font-size: var(--text-control);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .switch:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .switch:focus-visible {
    outline: none;
  }
  .switch:focus-visible .track {
    box-shadow: 0 0 0 2px var(--accent-border);
  }

  .track {
    position: relative;
    width: 38px;
    height: 21px;
    flex: 0 0 auto;
    border-radius: var(--radius-pill);
    background: var(--bg-raised);
    border: 1px solid var(--border);
    transition:
      background var(--transition-fast),
      border-color var(--transition-fast),
      box-shadow var(--transition-fast);
  }
  .switch.on .track {
    background: var(--accent);
    border-color: var(--accent);
  }

  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: var(--text-dim);
    transition:
      left var(--transition-fast),
      background var(--transition-fast);
  }
  .switch.on .knob {
    left: 19px;
    background: var(--on-accent);
  }

  .switch-label {
    min-width: 0;
  }
</style>
