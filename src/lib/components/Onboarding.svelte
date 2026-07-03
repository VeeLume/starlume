<script lang="ts">
  // The onboarding framework. Fixed frame around module-contributed steps:
  //
  //   Welcome → [core framework steps] → Modules (picker)
  //           → [steps of each SELECTED module] → Finish
  //
  // Core steps (online/privacy, Discord sign-in, SC account) are app-level —
  // every install goes through them. Modules never touch this file; they
  // contribute steps via `ModuleDescriptor.onboardingSteps`. The list is
  // derived from the current selection, so toggling a module adds/removes its
  // steps live.

  import type { Component } from "svelte";
  import { moduleRegistry } from "$lib/modules/registry";
  import type { OnboardingStepProps } from "$lib/modules/types";
  import { settingsStore, applySettings } from "$lib/state/settings.svelte";
  import { closeOnboarding } from "$lib/state/onboarding.svelte";
  import OnboardingOnline from "$lib/components/onboarding/OnboardingOnline.svelte";
  import OnboardingDiscord from "$lib/components/onboarding/OnboardingDiscord.svelte";
  import OnboardingRsi from "$lib/components/onboarding/OnboardingRsi.svelte";

  // App-level framework steps, in order. Each is optional (nothing gates
  // Next); they exist so the posture + identity are set deliberately.
  const CORE_STEPS: { id: string; title: string; component: Component<OnboardingStepProps> }[] = [
    { id: "online", title: "Online & privacy", component: OnboardingOnline },
    { id: "discord", title: "Sign in", component: OnboardingDiscord },
    { id: "rsi", title: "Your Star Citizen account", component: OnboardingRsi },
  ];

  type Step =
    | { kind: "welcome"; id: string; title: string }
    | {
        kind: "core-step";
        id: string;
        title: string;
        component: Component<OnboardingStepProps>;
      }
    | { kind: "modules"; id: string; title: string }
    | {
        kind: "module-step";
        id: string;
        title: string;
        moduleName: string;
        component: Component<OnboardingStepProps>;
      }
    | { kind: "finish"; id: string; title: string };

  let selected = $state<Set<string>>(
    new Set(settingsStore.current?.enabled_modules ?? []),
  );
  let index = $state(0);
  let canContinue = $state(true);
  let error = $state("");

  const steps: Step[] = $derived([
    { kind: "welcome", id: "welcome", title: "Welcome" },
    ...CORE_STEPS.map(
      (s): Step => ({ kind: "core-step", id: s.id, title: s.title, component: s.component }),
    ),
    { kind: "modules", id: "modules", title: "Choose your modules" },
    ...moduleRegistry
      .filter((d) => selected.has(d.id))
      .flatMap((d) =>
        (d.onboardingSteps ?? []).map(
          (s): Step => ({
            kind: "module-step",
            id: `${d.id}:${s.id}`,
            title: s.title,
            moduleName: d.name,
            component: s.component,
          }),
        ),
      ),
    { kind: "finish", id: "finish", title: "All set" },
  ]);
  const step = $derived(steps[Math.min(index, steps.length - 1)]);
  const atEnd = $derived(index >= steps.length - 1);

  function toggleModule(id: string) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

  function next() {
    if (!canContinue) return;
    canContinue = true; // each step starts permissive
    index = Math.min(index + 1, steps.length - 1);
  }

  function back() {
    canContinue = true;
    index = Math.max(0, index - 1);
  }

  async function finish() {
    error = "";
    const err = await applySettings({
      enabled_modules: [...selected],
      onboarding_completed: true,
    });
    if (err) {
      error = err.message;
      return;
    }
    closeOnboarding();
  }

  // Skip = complete without changing the module selection.
  async function skip() {
    error = "";
    const err = await applySettings({ onboarding_completed: true });
    if (err) {
      error = err.message;
      return;
    }
    closeOnboarding();
  }
</script>

<div class="overlay">
  <div class="card">
    <header>
      <h1>{step.title}</h1>
      {#if step.kind === "module-step"}
        <span class="module-tag">{step.moduleName}</span>
      {/if}
      <button class="skip" onclick={skip}>Skip setup</button>
    </header>

    <div class="body">
      {#if step.kind === "welcome"}
        <p>
          Starlume is a companion for Star Citizen — it keeps things like your
          localization patches and trackers current in the background while you
          play.
        </p>
        <p class="dim">
          Pick the features you want on the next page. Everything is optional
          and can be changed later in Settings.
        </p>
      {:else if step.kind === "modules"}
        {#if moduleRegistry.length === 0}
          <p class="dim">
            No feature modules are available in this build yet — the shell is
            all there is. Future updates add them here.
          </p>
        {:else}
          <div class="module-grid">
            {#each moduleRegistry as m (m.id)}
              <button
                class="module-card"
                class:selected={selected.has(m.id)}
                onclick={() => toggleModule(m.id)}
              >
                <span class="module-icon">{m.icon}</span>
                <span class="module-name">{m.name}</span>
                <span class="module-desc">{m.description}</span>
              </button>
            {/each}
          </div>
        {/if}
      {:else if step.kind === "core-step" || step.kind === "module-step"}
        <step.component setCanContinue={(ok: boolean) => (canContinue = ok)} />
      {:else}
        <p>
          {#if selected.size > 0}
            {selected.size} module{selected.size === 1 ? "" : "s"} enabled.
          {:else}
            No modules enabled — you can add some in Settings anytime.
          {/if}
        </p>
        <p class="dim">
          Tip: Starlume can start with Windows and live in the tray — see
          Settings → App.
        </p>
      {/if}
    </div>

    {#if error}
      <p class="error">{error}</p>
    {/if}

    <footer>
      <div class="dots">
        {#each steps as s, i (s.id)}
          <span class="dot" class:on={i === index}></span>
        {/each}
      </div>
      <div class="buttons">
        {#if index > 0}
          <button onclick={back}>Back</button>
        {/if}
        {#if atEnd}
          <button class="primary" onclick={finish}>Finish</button>
        {:else}
          <button class="primary" disabled={!canContinue} onclick={next}>
            Next
          </button>
        {/if}
      </div>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: grid;
    place-items: center;
    z-index: 100;
  }

  .card {
    width: min(560px, calc(100vw - 48px));
    min-height: 380px;
    display: flex;
    flex-direction: column;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 24px;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  h1 {
    font-size: 20px;
    margin: 0;
  }

  .module-tag {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }

  .skip {
    margin-left: auto;
    background: none;
    border: none;
    color: var(--text-dim);
    padding: 0;
  }
  .skip:hover {
    color: var(--text);
  }

  .body {
    flex: 1;
    padding: 16px 0;
  }

  .module-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .module-card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    text-align: left;
    padding: 12px;
    border-radius: 10px;
  }
  .module-card.selected {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-raised));
  }
  .module-icon {
    font-size: 20px;
  }
  .module-name {
    font-weight: 600;
  }
  .module-desc {
    font-size: 12px;
    color: var(--text-dim);
  }

  footer {
    display: flex;
    align-items: center;
  }

  .dots {
    display: flex;
    gap: 6px;
    flex: 1;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--border);
  }
  .dot.on {
    background: var(--accent);
  }

  .buttons {
    display: flex;
    gap: 8px;
  }

  .primary {
    border-color: var(--accent);
    color: var(--accent);
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .dim {
    color: var(--text-dim);
  }
  .error {
    color: #e06c6c;
    margin: 0 0 8px;
  }
</style>
