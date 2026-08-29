<script lang="ts">
  // The onboarding framework on the kit `Wizard`. Fixed frame around
  // module-contributed steps:
  //
  //   Welcome → [core framework steps] → Modules (picker)
  //           → [steps of each SELECTED module] → Finish
  //
  // Core steps (online/privacy, Discord sign-in, SC account) are app-level —
  // every install goes through them. Modules never touch this file; they
  // contribute steps via `ModuleDescriptor.onboardingSteps`. The step list is
  // `$derived` from the current selection, so toggling a module adds/removes
  // its steps live — the wizard has no registry concept, the list just
  // changes under it (its index self-clamps).
  //
  // The overlay host is this file's (the kit rule: the wizard owns the step
  // frame, never where it lives).

  import type { Component } from "svelte";
  import { Wizard, type WizardStep } from "@veelume/ui";
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

  let selected = $state<Set<string>>(new Set(settingsStore.current?.enabled_modules ?? []));
  let index = $state(0);
  let busy = $state(false);
  let error = $state("");

  // Per-step forward gates, written by step components through the module
  // contract's imperative `setCanContinue` (from handlers/effects, never
  // during render) and read declaratively by the wizard. Absent = passable.
  let gates = $state<Record<string, boolean>>({});

  // Step descriptors — the component steps share one snippet (`compStep`)
  // that renders the ACTIVE descriptor's component; only the current step is
  // ever mounted, so the indirection is safe.
  type CompDesc = { id: string; component: Component<OnboardingStepProps> };
  const compDescs = $derived<Record<string, CompDesc>>(
    Object.fromEntries(
      [
        ...CORE_STEPS.map((s) => ({ id: s.id, component: s.component })),
        ...moduleRegistry
          .filter((d) => selected.has(d.id))
          .flatMap((d) =>
            (d.onboardingSteps ?? []).map((s) => ({
              id: `${d.id}:${s.id}`,
              component: s.component,
            })),
          ),
      ].map((d) => [d.id, d]),
    ),
  );

  const steps = $derived<WizardStep[]>([
    { id: "welcome", title: "Welcome", step: welcome },
    ...CORE_STEPS.map((s) => ({
      id: s.id,
      title: s.title,
      canContinue: () => gates[s.id] ?? true,
      step: compStep,
    })),
    { id: "modules", title: "Choose your modules", step: modulesStep },
    ...moduleRegistry
      .filter((d) => selected.has(d.id))
      .flatMap((d) =>
        (d.onboardingSteps ?? []).map((s) => {
          const id = `${d.id}:${s.id}`;
          return {
            id,
            title: s.title,
            tag: d.name,
            canContinue: () => gates[id] ?? true,
            step: compStep,
          };
        }),
      ),
    { id: "finish", title: "All set", step: done },
  ]);

  const active = $derived(steps[Math.min(index, Math.max(0, steps.length - 1))]);
  const activeComp = $derived(active ? compDescs[active.id] : undefined);

  function toggleModule(id: string) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

  async function finish() {
    error = "";
    busy = true;
    try {
      const err = await applySettings({
        enabled_modules: [...selected],
        onboarding_completed: true,
      });
      if (err) {
        error = err.message;
        return;
      }
      closeOnboarding();
    } finally {
      busy = false;
    }
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

{#snippet welcome()}
  <p>
    Starlume is a companion for Star Citizen — it keeps things like your localization
    patches and trackers current in the background while you play.
  </p>
  <p class="text-muted-foreground">
    Pick the features you want on the next page. Everything is optional and can be changed
    later in Settings.
  </p>
{/snippet}

{#snippet modulesStep()}
  {#if moduleRegistry.length === 0}
    <p class="text-muted-foreground">
      No feature modules are available in this build yet — the shell is all there is.
      Future updates add them here.
    </p>
  {:else}
    <div class="module-grid">
      {#each moduleRegistry as m (m.id)}
        <button
          class="btn module-card"
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
{/snippet}

{#snippet compStep()}
  {#if activeComp}
    {@const Step = activeComp.component}
    <Step setCanContinue={(ok: boolean) => (gates[activeComp.id] = ok)} />
  {/if}
{/snippet}

{#snippet done()}
  <p>
    {#if selected.size > 0}
      {selected.size} module{selected.size === 1 ? "" : "s"} enabled.
    {:else}
      No modules enabled — you can add some in Settings anytime.
    {/if}
  </p>
  <p class="text-muted-foreground">
    Tip: Starlume can start with Windows and live in the tray — see Settings → General.
  </p>
{/snippet}

<div class="overlay">
  <div class="card">
    <Wizard {steps} bind:index onfinish={finish} onskip={skip} {busy} />
    {#if error}
      <p class="px-4 pb-3 text-sm text-destructive">{error}</p>
    {/if}
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
    min-height: 420px;
    display: flex;
    flex-direction: column;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow: hidden;
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
</style>
