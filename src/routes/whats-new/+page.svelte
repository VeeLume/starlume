<script lang="ts">
  // What's new: the short guide after an update (design pass, 2026-10-19).
  // A page per module the user hasn't been offered, with its switch, the
  // unofficial-API features it reads through (the consent in place) and its
  // setup (the same component onboarding shows); then the release's big
  // changes ($lib/guides); then Done, which marks it seen. Later leaves it
  // pending, waiting in the bell. On the kit Wizard, its Bar replaced by the
  // pages as tabs.
  //
  // `?v=0.2.0` shows one release's changes from Settings → About, without
  // module pages and without marking anything.
  import { onMount, type Snippet } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { Button, Settings, Switch, Wizard, type WizardHeaderProps, type WizardStep } from "@veelume/ui";
  import { X } from "lucide-svelte";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import type { ModuleDescriptor } from "$lib/modules/types";
  import { RELEASE_GUIDES, type GuideChange } from "$lib/guides";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import { scStore } from "$lib/state/sc.svelte";
  import { guide, loadVersion, markGuideSeen, deferGuide } from "$lib/state/guide.svelte";
  import { allowFeature, unofficialTerms } from "$lib/unofficial";

  const settings = $derived(settingsStore.current);
  const history = $derived(page.url.searchParams.get("v"));

  let features = $state<GrpcFeatureInfo[]>([]);
  let index = $state(0);
  let busy = $state(false);
  let error = $state("");

  onMount(async () => {
    await Promise.all([loadSettings(), loadVersion()]);
    features = await commands.listGrpcFeatures();
  });

  // The modules this guide offers, fixed once settings are in: turning one
  // on here must not move the pages under the user.
  let offered = $state<ModuleDescriptor[] | null>(null);
  $effect(() => {
    if (offered === null && settings && !history) offered = guide.newModules;
  });

  const releases = $derived(
    history ? RELEASE_GUIDES.filter((g) => g.version === history) : guide.newReleases,
  );
  const changes = $derived(
    releases.flatMap((g) => g.changes.map((c) => ({ ...c, version: g.version }))),
  );
  const title = $derived(
    history
      ? `What's new in Starlume ${history}`
      : releases.length === 1 && !offered?.length
        ? `What's new in Starlume ${releases[0].version}`
        : `What's new in Starlume ${guide.version ?? ""}`,
  );

  type Page = { id: string; tab: string; module?: ModuleDescriptor; body: Snippet };
  const pages = $derived<Page[]>([
    ...(offered ?? []).map((m) => ({ id: `module:${m.id}`, tab: m.name, module: m, body: modulePage })),
    ...(changes.length ? [{ id: "changes", tab: "Also changed", body: changesPage }] : []),
    ...(history ? [] : [{ id: "done", tab: "Done", body: donePage }]),
  ]);
  const steps = $derived<WizardStep[]>(pages.map((p) => ({ id: p.id, title: p.tab, step: p.body })));
  const at = $derived(Math.min(index, Math.max(0, pages.length - 1)));
  const current = $derived(pages[at]);

  const isOn = (id: string) => !!settings?.enabled_modules.includes(id);
  const allowed = (id: string) => !!settings?.grpc_enabled && !!settings.grpc_features.includes(id);
  const info = (id: string) => features.find((f) => f.id === id);

  async function setModule(id: string, on: boolean) {
    const current = settings?.enabled_modules ?? [];
    const err = await applySettings({
      enabled_modules: on ? [...current.filter((m) => m !== id), id] : current.filter((m) => m !== id),
    });
    if (err) error = err.message;
  }

  async function setFeature(id: string, on: boolean) {
    const err = await allowFeature(id, on);
    if (err) error = err.message;
  }

  function showsUp(m: ModuleDescriptor): string {
    const parts = [
      ...(m.nav ?? []).map((n) => `${n.label} in the sidebar`),
      ...(m.homeCards ?? []).map((c) => `a ${c.title} card on Home`),
    ];
    return parts.join(", ");
  }

  async function done() {
    busy = true;
    try {
      await markGuideSeen((offered ?? []).map((m) => m.id));
      void goto("/");
    } finally {
      busy = false;
    }
  }

  function later() {
    deferGuide();
    void goto("/");
  }

  const kindLabel: Record<GuideChange["kind"], string> = { new: "New", changed: "Changed", setting: "Setting" };
</script>

{#snippet modulePage()}
  {@const m = current.module!}
  {@const on = isOn(m.id)}
  <div class="page">
    <div class="main">
      <span class="hue-chip" style:--hue="var(--hue-teal)">New module</span>
      <div class="guide-head">
        <h1>{m.name}</h1>
        <p>{m.description}</p>
      </div>
      <ul class="pitch">
        {#each m.pitch as p (p)}<li>{p}</li>{/each}
      </ul>
      <div class="turn" class:on>
        <div>
          <b>Turn on {m.name}</b>
          {#if showsUp(m)}<span>{showsUp(m)}.</span>{/if}
        </div>
        <Switch label={`Turn on ${m.name}`} checked={on} onchange={(v) => setModule(m.id, v)} />
      </div>
      {#if on && m.unofficial?.length}
        <section class="consent">
          <p class="label">Unofficial APIs</p>
          <p>{unofficialTerms([m.name])}</p>
          {#each m.unofficial as u (u.feature)}
            {@const f = info(u.feature)}
            <div class="feature" class:off={!settings?.online_enabled}>
              <div>
                <b>{f?.name ?? u.feature}</b>
                <span class="tag">{u.required ? "needed" : "optional"}</span>
                {#if f}<span class="hint">{f.description}</span>{/if}
                {#if u.required && !allowed(u.feature)}
                  <span class="hint warn">{m.name} stays empty without it.</span>
                {/if}
              </div>
              <Switch
                label={f?.name ?? u.feature}
                checked={allowed(u.feature)}
                disabled={!settings?.online_enabled}
                onchange={(v) => setFeature(u.feature, v)}
              />
            </div>
          {/each}
          {#if !settings?.online_enabled}
            <p class="state">Online features are off; Settings → Online & privacy turns them on.</p>
          {/if}
        </section>
      {/if}
      {#if on && m.setup}
        {@const Setup = m.setup.component}
        <Setup setCanContinue={() => {}} />
      {/if}
    </div>
    <aside class="side">
      <p class="label">Needs</p>
      <div class="req">
        {#if m.needsGame}
          {#if (scStore.status?.installs.length ?? 0) > 0}
            <span class="ok">✓</span><span>Your Star Citizen install</span>
          {:else}
            <span class="faint">–</span><span>A Star Citizen install<small>Not found. The module waits for one.</small></span>
          {/if}
        {/if}
        {#each m.unofficial ?? [] as u (u.feature)}
          <span class={allowed(u.feature) ? "ok" : "u"}>{allowed(u.feature) ? "✓" : "!"}</span>
          <span>{info(u.feature)?.name ?? u.feature}{u.required ? "" : " (optional)"}<small>An unofficial API</small></span>
        {/each}
        {#if !m.needsGame && !m.unofficial?.length}
          <span class="ok">✓</span><span>Nothing</span>
        {/if}
      </div>
    </aside>
  </div>
{/snippet}

{#snippet changesPage()}
  <div class="page one">
    <div class="main">
      <div class="guide-head">
        <h1>{offered?.length ? "Also changed" : "What changed"}</h1>
        <p>The changes that touch something you use or set. The full list is in the release notes.</p>
      </div>
      <div class="changes">
        {#each changes as c (c.version + c.title)}
          <div class="change">
            <span class="kind {c.kind}">{kindLabel[c.kind]}</span>
            <div>
              <b>{c.title}</b>
              {#if releases.length > 1}<span class="ver">{c.version}</span>{/if}
              <p>{c.body}</p>
              {#if c.yours && settings}
                {@const v = c.yours(settings)}
                {#if v}<p class="yours">Yours: <b>{v}</b></p>{/if}
              {/if}
            </div>
            {#if c.href}
              <Button variant="outline" onclick={() => goto(c.href!)}>{c.action ?? "Open"}</Button>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  </div>
{/snippet}

{#snippet donePage()}
  {@const turnedOn = (offered ?? []).filter((m) => isOn(m.id)).map((m) => m.name)}
  <div class="page one">
    <div class="main">
      <div class="guide-head">
        <h1>That's all</h1>
        <p>
          {#if turnedOn.length}
            {turnedOn.join(" and ")} {turnedOn.length > 1 ? "are" : "is"} in the sidebar now.
          {:else if offered?.length}
            Nothing turned on. Settings → Modules has them when you want them.
          {:else}
            You're up to date.
          {/if}
        </p>
      </div>
      <p class="state">This guide stays in Settings → About.</p>
    </div>
  </div>
{/snippet}

{#snippet header({ index: i, skip }: WizardHeaderProps)}
  <header class="top">
    <p class="label">{title}</p>
    <span class="grow"></span>
    {#if pages.length > 1}
      <div class="tabs-u" role="tablist">
        {#each pages as p, n (p.id)}
          <button role="tab" aria-selected={n === i} class:on={n === i} onclick={() => (index = n)}>
            {#if p.module && isOn(p.module.id)}<span class="ok">✓</span>{/if}{p.tab}
          </button>
        {/each}
      </div>
    {/if}
    {#if skip}
      <Button variant="ghost" onclick={skip}>Later</Button>
    {:else}
      <button class="x" title="Close" onclick={() => goto("/settings/about")}><X size={16} /></button>
    {/if}
  </header>
{/snippet}

<div class="wn">
  {#if settings && (history || offered !== null)}
    {#if pages.length > 0}
      <Wizard
        {steps}
        bind:index
        {busy}
        {header}
        finishLabel={history ? "Close" : "Done"}
        onfinish={history ? () => goto("/settings/about") : done}
        onskip={history ? undefined : later}
      />
    {:else}
      <div class="page one">
        <div class="main">
          <div class="guide-head"><h1>You're up to date</h1><p>Nothing new since your last look.</p></div>
          <Button variant="outline" onclick={() => goto("/settings/about")}>Earlier releases</Button>
        </div>
      </div>
    {/if}
  {/if}
  {#if error}<p class="error px-4 pb-3">{error}</p>{/if}
</div>

<style>
  /* Centred at a reading width, so a wide window doesn't split the page
   * into text on the left and the Needs panel on the far right. */
  .wn {
    width: 100%;
    max-width: 1120px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    min-height: calc(100vh - 2 * var(--content-pad));
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    background: var(--bg-raised);
    overflow: hidden;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
    padding: 14px 18px 0;
  }
  .top .label {
    margin: 0;
  }
  .grow {
    flex: 1;
  }
  .tabs-u {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--border);
  }
  .tabs-u button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px 7px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    font-size: var(--text-control);
    color: var(--text-dim);
    cursor: pointer;
  }
  .tabs-u button:hover:not(.on) {
    color: var(--text);
  }
  .tabs-u button.on {
    color: var(--text);
    border-bottom-color: var(--accent);
    font-weight: var(--weight-semibold);
  }
  .x {
    display: grid;
    place-items: center;
    padding: 4px;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
  }
  .x:hover {
    color: var(--text);
  }

  .page {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 28px;
    align-items: start;
    padding: 8px 6px 12px;
  }
  .page.one {
    grid-template-columns: minmax(0, 1fr);
    max-width: 820px;
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
    max-width: 680px;
  }
  .main > .hue-chip {
    align-self: flex-start;
  }
  .turn {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    background: var(--bg);
  }
  .turn.on {
    border-color: var(--accent-border);
  }
  .turn > div {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .turn b {
    font-family: var(--font-display);
    font-size: 1rem;
    color: var(--text-strong);
  }
  .turn span {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .feature {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 16px;
    align-items: center;
    padding: 8px 0;
    border-top: 1px solid color-mix(in srgb, var(--warn) 18%, var(--border));
  }
  .feature.off {
    opacity: 0.5;
  }
  .feature b {
    font-weight: var(--weight-medium);
  }
  .tag {
    margin-left: 6px;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
  }
  .hint {
    display: block;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .hint.warn {
    color: color-mix(in srgb, var(--warn) 85%, var(--text));
  }
  .state {
    margin: 0;
    font-size: var(--text-small);
    color: var(--text-faint);
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    background: var(--bg);
  }
  .side .label {
    margin: 0;
  }
  .req {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    gap: 6px 8px;
  }
  .req small {
    display: block;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .ok {
    color: var(--good);
  }
  .u {
    color: var(--warn);
  }
  .faint {
    color: var(--text-faint);
  }

  .changes {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
  }
  .change {
    display: grid;
    grid-template-columns: 76px minmax(0, 1fr) auto;
    gap: 14px;
    align-items: start;
    padding: 12px 2px;
    border-bottom: 1px solid var(--border);
  }
  .change b {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .change p {
    margin: 2px 0 0;
    font-size: var(--text-control);
    color: var(--text-dim);
  }
  .change .yours {
    margin-top: 6px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .change .yours b {
    font-weight: var(--weight-medium);
    color: var(--text-dim);
  }
  .ver {
    margin-left: 8px;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    color: var(--text-faint);
  }
  .kind {
    padding-top: 3px;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .kind.new {
    color: var(--hue-teal);
  }
  .kind.changed {
    color: var(--hue-sand);
  }
  .kind.setting {
    color: var(--hue-sky);
  }
  @media (max-width: 900px) {
    .page {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
