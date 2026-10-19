<script lang="ts">
  // Onboarding: online + unofficial-API posture, after the modules so it can
  // be concrete. The master online switch, verifying the RSI account (a
  // network read, so it lives here), and only the unofficial-API features
  // the chosen modules read through, grouped by module and marked needed or
  // optional. The consent text sits on the page (`.consent`) instead of a
  // native dialog: turning the first feature on is the consent, and the
  // backend records it with the master switch. Offline makes them inert;
  // the choices are kept. Nothing here gates Next.
  import { onMount } from "svelte";
  import { Button, Settings, Switch } from "@veelume/ui";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import type { ModuleDescriptor } from "$lib/modules/types";
  import { settingsStore, applySettings } from "$lib/state/settings.svelte";
  import { scStore, verifyAccount } from "$lib/state/sc.svelte";
  import { allowFeature, unofficialTerms } from "$lib/unofficial";

  let { modules }: { modules: ModuleDescriptor[] } = $props();

  const settings = $derived(settingsStore.current);
  const offline = $derived(!settings?.online_enabled);
  const users = $derived(modules.filter((m) => m.unofficial?.length));
  const allowed = (id: string) => !!settings?.grpc_enabled && !!settings.grpc_features.includes(id);

  let features = $state<GrpcFeatureInfo[]>([]);
  let verifying = $state(false);
  let error = $state("");

  onMount(async () => {
    features = await commands.listGrpcFeatures();
  });

  const info = (id: string) => features.find((f) => f.id === id);
  const handle = $derived(scStore.account?.handle ?? scStore.status?.launcher_handle ?? null);
  const verified = $derived(!!scStore.account?.citizen_record);

  async function verify() {
    if (!handle) return;
    error = "";
    verifying = true;
    const err = await verifyAccount(handle);
    if (err) error = err;
    verifying = false;
  }

  async function setFeature(id: string, on: boolean) {
    const err = await allowFeature(id, on);
    if (err) error = err.message;
  }
</script>

{#if settings}
  <div class="privacy">
    <Settings.Section>
      <Settings.Row label="Online features" hint="Off means no network at all, except update checks.">
        <Switch
          label="Online features"
          checked={settings.online_enabled}
          onchange={(v) => applySettings({ online_enabled: v })}
        />
      </Settings.Row>
      {#if handle}
        <Settings.Row
          label="Verify your RSI account"
          hint="Reads your public RSI profile once, so a handle rename keeps your data."
        >
          {#if verified}
            <span class="status-pill good"><i></i>Verified</span>
          {:else}
            <Button variant="outline" onclick={verify} disabled={offline || verifying}>
              {verifying ? "Verifying…" : "Verify"}
            </Button>
          {/if}
        </Settings.Row>
      {/if}
    </Settings.Section>

    {#if users.length > 0}
      <section class="consent">
        <p class="label">Unofficial APIs</p>
        <p>{unofficialTerms(users.map((m) => m.name))}</p>
        {#each users as m (m.id)}
          <div class="group">
            <p class="label mod">{m.name}</p>
            {#each m.unofficial ?? [] as u (u.feature)}
              {@const f = info(u.feature)}
              <div class="feature" class:off={offline}>
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
                  disabled={offline}
                  onchange={(v) => setFeature(u.feature, v)}
                />
              </div>
            {/each}
          </div>
        {/each}
        <p class="state">
          {#if offline}
            Inert while online features are off. Your choices are kept.
          {:else if settings.grpc_enabled && settings.grpc_features.length > 0}
            Turning one on is your consent; Settings → Online & privacy takes it back.
          {:else}
            Nothing allowed yet.
          {/if}
        </p>
      </section>
    {:else}
      <p class="dim">None of your modules uses an unofficial API.</p>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}
  </div>
{/if}

<style>
  .privacy {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .group {
    display: flex;
    flex-direction: column;
    border-top: 1px solid color-mix(in srgb, var(--warn) 18%, var(--border));
  }
  .mod {
    margin: 0;
    padding: 10px 0 2px;
    font-size: var(--text-badge);
    color: var(--text-faint);
  }
  .feature {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 16px;
    align-items: center;
    padding: 8px 0;
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
</style>
