<script lang="ts">
  // First run, in the app's own frame: the window's sidebar becomes the
  // step rail, the kit `Wizard` (with a page heading in place of its Bar)
  // fills the content side. The order (design pass, 2026-10-19):
  //
  //   Welcome → Modules → [setup of each SELECTED module] → Online & privacy
  //           → In the background → Done
  //
  // Modules come before privacy so the privacy step can ask only for the
  // unofficial-API features the chosen modules read through. Modules never
  // touch this file; they contribute a step via `ModuleDescriptor.setup`
  // (the same component their What's-new page shows). The step list is
  // `$derived` from the selection, so toggling a module adds and removes its
  // setup live (the wizard's index self-clamps).
  //
  // The overlay host is this file's (the kit rule: the wizard owns the step
  // frame, never where it lives).

  import type { Snippet } from "svelte";
  import { Button, Wizard, type WizardHeaderProps, type WizardStep } from "@veelume/ui";
  import { moduleRegistry } from "$lib/modules/registry";
  import type { ModuleDescriptor } from "$lib/modules/types";
  import { settingsStore, applySettings } from "$lib/state/settings.svelte";
  import { closeOnboarding } from "$lib/state/onboarding.svelte";
  import { allModuleIds, loadVersion } from "$lib/state/guide.svelte";
  import OnboardingWelcome from "$lib/components/onboarding/OnboardingWelcome.svelte";
  import OnboardingModules from "$lib/components/onboarding/OnboardingModules.svelte";
  import OnboardingPrivacy from "$lib/components/onboarding/OnboardingPrivacy.svelte";
  import OnboardingDiscord from "$lib/components/onboarding/OnboardingDiscord.svelte";
  import OnboardingBackground from "$lib/components/onboarding/OnboardingBackground.svelte";
  import OnboardingDone from "$lib/components/onboarding/OnboardingDone.svelte";
  import brandMark from "../../../src-tauri/icons/source/icon-small.svg?url";
  import { SHOW_UNRELEASED } from "$lib/unreleased";

  // What's enabled; on a first run with nothing enabled, the modules that
  // need no unofficial API (nothing grey to agree to for them).
  const enabled = settingsStore.current?.enabled_modules ?? [];
  let selected = $state<Set<string>>(
    new Set(
      enabled.length > 0 || settingsStore.current?.onboarding_completed
        ? enabled
        : moduleRegistry.filter((m) => !m.unofficial?.some((u) => u.required)).map((m) => m.id),
    ),
  );
  let index = $state(0);
  let busy = $state(false);
  let error = $state("");

  // Per-step forward gates, written by step components through the
  // contract's imperative `setCanContinue` (from handlers/effects, never
  // during render) and read declaratively by the wizard. Absent = passable.
  let gates = $state<Record<string, boolean>>({});
  const gate = (id: string) => (ok: boolean) => (gates[id] = ok);

  const chosen = $derived(moduleRegistry.filter((m) => selected.has(m.id)));

  type Def = {
    id: string;
    /** The rail's label. */
    rail: string;
    heading: string;
    lead: string;
    /** A module's setup: nested under Modules in the rail. */
    module?: ModuleDescriptor;
    body: Snippet;
    wide?: boolean;
  };

  const defs = $derived<Def[]>([
    {
      id: "welcome",
      rail: "Welcome",
      heading: "Your Star Citizen companion",
      lead: "Starlume keeps your text patch, game data and news current in the background while you play.",
      body: welcomeStep,
    },
    {
      id: "modules",
      rail: "Modules",
      heading: "Choose your modules",
      lead: "Turn on what you want. Modules you leave off stay out of sight.",
      body: modulesStep,
      wide: true,
    },
    ...chosen
      .filter((m) => m.setup)
      .map((m) => ({
        id: `setup:${m.id}`,
        rail: m.name,
        heading: m.setup!.title,
        lead: m.setup!.lead,
        module: m,
        body: setupStep,
      })),
    {
      id: "privacy",
      rail: "Online & privacy",
      heading: "Online & privacy",
      lead: "Starlume works offline. You decide what it may reach out for.",
      body: privacyStep,
    },
    // Discord sign-in serves only the unreleased friends/community layer.
    ...(SHOW_UNRELEASED
      ? [
          {
            id: "discord",
            rail: "Sign in",
            heading: "Sign in",
            lead: "Discord sign-in, for friends and groups.",
            body: discordStep,
          },
        ]
      : []),
    {
      id: "background",
      rail: "In the background",
      heading: "In the background",
      lead: "Starlume lives in the tray. Hidden, it uses about 11 MB.",
      body: backgroundStep,
    },
    {
      id: "done",
      rail: "Done",
      heading: "You're set",
      lead: "Here is where things are.",
      body: doneStep,
    },
  ]);

  const steps = $derived<WizardStep[]>(
    defs.map((d) => ({
      id: d.id,
      title: d.heading,
      tag: d.module?.name,
      canContinue: () => gates[d.id] ?? true,
      step: d.body,
    })),
  );

  const at = $derived(Math.min(index, defs.length - 1));
  const active = $derived(defs[at]);

  function toggleModule(id: string) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

  // Finishing and skipping both offer every module (the guide then offers
  // only what a later release brings) and record this version as seen.
  async function complete(patch: { enabled_modules?: string[] }) {
    error = "";
    busy = true;
    try {
      const err = await applySettings({
        ...patch,
        onboarding_completed: true,
        modules_offered: allModuleIds(),
        guide_seen: await loadVersion(),
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
</script>

{#snippet welcomeStep()}
  <div class="ob-step"><OnboardingWelcome /></div>
{/snippet}

{#snippet modulesStep()}
  <div class="ob-step wide"><OnboardingModules {selected} ontoggle={toggleModule} /></div>
{/snippet}

<!-- Every setup step renders the ACTIVE step's component; only the current
     step is ever mounted, so the indirection is safe. -->
{#snippet setupStep()}
  {#if active.module?.setup}
    {@const Setup = active.module.setup.component}
    <div class="ob-step"><Setup setCanContinue={gate(active.id)} /></div>
  {/if}
{/snippet}

{#snippet privacyStep()}
  <div class="ob-step"><OnboardingPrivacy modules={chosen} /></div>
{/snippet}

{#snippet discordStep()}
  <div class="ob-step"><OnboardingDiscord setCanContinue={gate("discord")} /></div>
{/snippet}

{#snippet backgroundStep()}
  <div class="ob-step"><OnboardingBackground /></div>
{/snippet}

{#snippet doneStep()}
  <div class="ob-step"><OnboardingDone modules={chosen} /></div>
{/snippet}

{#snippet header(_: WizardHeaderProps)}
  <header class="ob-header" class:wide={active.wide}>
    <div class="guide-head">
      {#if active.module}<p class="label">Modules · {active.module.name}</p>{/if}
      <h1>{active.heading}</h1>
      <p>{active.lead}</p>
    </div>
  </header>
{/snippet}

<div class="ob-frame">
  <nav class="ob-rail" aria-label="Setup steps">
    <span class="ob-brand"><img src={brandMark} alt="" />Starlume</span>
    <p class="label ob-rail-k">Setup</p>
    {#each defs as d, i (d.id)}
      {@const n = defs.slice(0, i + 1).filter((x) => !x.module).length}
      <button
        class="ob-st"
        class:sub={d.module}
        class:on={i === at}
        class:done={i < at}
        aria-current={i === at ? "step" : undefined}
        onclick={() => (index = i)}
      >
        <span class="ob-dot">{d.module ? "" : i < at ? "✓" : n}</span>
        <span class="truncate">{d.rail}</span>
      </button>
    {/each}
    <!-- Skip lives in the rail, not the step header: secondary, but for
         some the most important action, so it stays in sight on every step. -->
    <div class="ob-rail-foot">
      {#if at < defs.length - 1}
        <Button variant="outline" class="w-full" onclick={() => complete({})} disabled={busy}>
          Skip setup
        </Button>
      {/if}
      <p>Nothing here is required. Settings has all of it later.</p>
    </div>
  </nav>
  <div class="ob-main">
    <Wizard
      {steps}
      bind:index
      {busy}
      {header}
      finishLabel="Open Starlume"
      onfinish={() => complete({ enabled_modules: [...selected] })}
    />
    {#if error}
      <p class="error px-6 pb-3">{error}</p>
    {/if}
  </div>
</div>

<style>
  .ob-frame {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
    background: var(--bg);
  }

  .ob-rail {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 14px 10px;
    border-right: 1px solid var(--border);
    min-height: 0;
    overflow: auto;
  }
  .ob-brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 4px 8px 14px;
    font-family: var(--font-display);
    font-size: 1.05rem;
    font-weight: var(--weight-bold);
    letter-spacing: 0.04em;
    color: var(--text-strong);
  }
  .ob-brand img {
    width: 22px;
    height: 22px;
    border-radius: 6px;
  }
  .ob-rail-k {
    margin: 0;
    padding: 0 8px 8px;
    font-size: var(--text-badge);
    color: var(--text-faint);
  }
  .ob-st {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    font-family: var(--font-body);
    font-size: 0.93rem;
    color: var(--text-dim);
    text-align: left;
    cursor: pointer;
    transition: background-color var(--transition-fast) var(--easing);
  }
  .ob-st:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
    color: var(--text);
  }
  .ob-st.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .ob-st.sub {
    height: 30px;
    padding-left: 24px;
    font-size: 0.85rem;
  }
  .ob-dot {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    border: 1px solid var(--border);
    color: var(--text-faint);
  }
  .ob-st.sub .ob-dot {
    width: 12px;
    height: 12px;
    margin-left: 4px;
  }
  .ob-st.done .ob-dot {
    background: var(--accent-fill);
    border-color: var(--accent-line);
    color: var(--accent);
  }
  .ob-st.on .ob-dot {
    border-color: var(--accent);
    color: var(--accent);
  }
  .ob-rail-foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 0 2px;
  }
  .ob-rail-foot p {
    margin: 0;
    padding: 0 8px;
    font-size: var(--text-small);
    line-height: 1.45;
    color: var(--text-faint);
  }

  .ob-main {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  /* One centred column for the heading and the step, so a wide window
   * doesn't leave everything on the left edge; Skip ends the column. Text
   * steps read best at 760px, the module list takes 1040. The kit body
   * pads 16px, so the header's inset matches it. */
  .ob-header {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    width: 100%;
    max-width: calc(var(--ob-col) + 32px);
    margin: 0 auto;
    padding: var(--content-pad) 16px 4px;
  }
  .ob-header .guide-head p {
    max-width: 70ch;
  }
  .ob-frame {
    --ob-col: 760px;
  }
  .ob-header.wide,
  .ob-step.wide {
    --ob-col: 1040px;
  }
  .ob-header .guide-head {
    flex: 1;
    max-width: none;
  }
  .ob-step {
    max-width: var(--ob-col);
    margin: 0 auto;
    padding: 4px 0 16px;
  }
</style>
