<script lang="ts">
  // The Discord module's page: Rich Presence (decision 01a2085d, board
  // "Discord Rich Presence" on the Starlume Design canvas). The switch,
  // what it shares, and a preview of what Discord gets right now, drawn
  // from the live session by the Rust worker (presence.rs).
  import { Segmented, Settings, Switch } from "@veelume/ui";
  import type { PlaceLevel, PresenceSettings, PresenceStatus } from "$lib/bindings";
  import { applySettings, loadSettings, settingsStore } from "$lib/state/settings.svelte";
  import { presenceStore, watchPresence } from "$lib/state/presence.svelte";
  import { onMount } from "svelte";

  // The art the Discord application holds, by asset key (svc-presence/art).
  const art = import.meta.glob("../../../crates/svc-presence/art/*.png", {
    eager: true,
    query: "?url",
    import: "default",
  }) as Record<string, string>;
  const picture = (key: string | null | undefined) => {
    if (!key) return null;
    if (key.startsWith("https://")) return key;
    return Object.entries(art).find(([path]) => path.endsWith(`/${key}.png`))?.[1] ?? null;
  };

  const settings = $derived(settingsStore.current);
  const presence = $derived(settings?.presence);
  const view = $derived(presenceStore.view);
  const activity = $derived(view?.activity ?? null);
  let error = $state("");

  onMount(() => {
    void loadSettings();
    return watchPresence();
  });

  async function apply(patch: Partial<PresenceSettings>) {
    if (!settings) return;
    error = "";
    const err = await applySettings({ presence: { ...settings.presence, ...patch } });
    if (err) error = err.message;
  }

  // Discord counts the timer itself; the preview does the same.
  let now = $state(Date.now());
  onMount(() => {
    const tick = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(tick);
  });
  const elapsed = $derived.by(() => {
    const start = activity?.timestamps?.start;
    if (start == null) return null;
    const s = Math.max(0, Math.floor(now / 1000 - start));
    const hms = [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60];
    return hms[0] > 0
      ? `${hms[0]}:${String(hms[1]).padStart(2, "0")}:${String(hms[2]).padStart(2, "0")}`
      : `${hms[1]}:${String(hms[2]).padStart(2, "0")}`;
  });

  const STATUS: Record<PresenceStatus, string> = {
    off: "Off. Below is what Discord would show of your session.",
    offline: "Online features are off, so nothing goes to Discord.",
    no_session: "Needs the Session Log module, with the game's log or telemetry on.",
    not_playing: "Not playing: Discord shows nothing from Starlume.",
    no_discord: "Discord isn't running. Starlume looks for it every 30 seconds.",
    shown: "Discord shows this now.",
  };
  const PLACES: { value: PlaceLevel; label: string }[] = [
    { value: "exact", label: "Exact" },
    { value: "system", label: "System" },
    { value: "off", label: "Off" },
  ];
</script>

<div class="dc-page">
  <header class="dc-head">
    <h1>Discord</h1>
    {#if view}<span class="dc-mark" class:on={view.status === "shown"}><i></i>{view.status === "shown" ? "On your profile" : "Not on your profile"}</span>{/if}
  </header>
  {#if settings && presence}
    <Settings.Section>
      <Settings.Row
        label="Show your game on Discord"
        hint="While Star Citizen runs, your Discord profile says what you're doing. It goes through the Discord app on this PC to Discord, so it needs online features."
      >
        <Switch
          label="Show your game on Discord"
          checked={presence.enabled && settings.online_enabled}
          disabled={!settings.online_enabled}
          onchange={(v) => apply({ enabled: v })}
        />
      </Settings.Row>
      {#if !settings.online_enabled}
        <p class="note">Online features are off, so nothing goes to Discord. <a href="/settings/online">Turn them on in Online &amp; privacy</a>.</p>
      {/if}
    </Settings.Section>

    <Settings.Section title="What it shares">
      <Settings.Row
        label="Place"
        hint="Exact names the station, moon or quantum target. System says only Stanton, Pyro or Nyx. Off shows your progress there instead."
      >
        <Segmented options={PLACES} value={presence.place} onchange={(v) => apply({ place: v as PlaceLevel })} />
      </Settings.Row>
      <Settings.Row label="Ship" hint="The ship you're in, as its RSI ship store picture, with its name on hover.">
        <Switch label="Ship" checked={presence.ship} onchange={(v) => apply({ ship: v })} />
      </Settings.Row>
      <Settings.Row label="Contracts" hint="The contract's type and who gives it. Its title and your progress on hover.">
        <Switch label="Contracts" checked={presence.contracts} onchange={(v) => apply({ contracts: v })} />
      </Settings.Row>
    </Settings.Section>

    <Settings.Section title="On your profile">
      <Settings.Row
        label="Starlume button"
        hint="A “Get Starlume” link under your activity. Others see it; Discord hides it on your own profile."
      >
        <Switch label="Starlume button" checked={presence.button} onchange={(v) => apply({ button: v })} />
      </Settings.Row>
    </Settings.Section>

    <Settings.Section title="Preview">
      <div class="prev">
        <div class="card" class:none={!activity}>
          <span class="play">Playing</span>
          <div class="body">
            <span class="art">
              {#if picture(activity?.assets?.large_image)}
                <img class="big" src={picture(activity?.assets?.large_image)} alt="" title={activity?.assets?.large_text ?? ""} />
              {:else}
                <span class="big empty"></span>
              {/if}
              {#if picture(activity?.assets?.small_image)}
                <img class="small" src={picture(activity?.assets?.small_image)} alt="" title={activity?.assets?.small_text ?? ""} />
              {/if}
            </span>
            <span class="txt">
              <b>Star Citizen</b>
              <span>{activity?.details ?? "Nothing sent"}</span>
              {#if activity?.state}<span>{activity.state}</span>{/if}
              {#if elapsed}<span class="time">{elapsed}</span>{/if}
            </span>
          </div>
          {#if activity?.buttons?.length}
            <span class="btn">{activity.buttons[0].label}</span>
          {/if}
        </div>
        <div class="side">
          <b>{view ? STATUS[view.status] : "Reading…"}</b>
          {#if view?.status === "shown" && view.user}<span>Signed in to Discord as {view.user}.</span>{/if}
          {#if activity?.assets?.large_text}<span>Big picture, on hover: <em>{activity.assets.large_text}</em></span>{/if}
          {#if activity?.assets?.small_text}<span>Small icon, on hover: <em>{activity.assets.small_text}</em></span>{/if}
        </div>
      </div>
    </Settings.Section>

    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}
  {/if}
</div>

<style>
  .dc-page {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 880px;
  }
  .dc-head {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 26px;
  }
  .dc-head h1 {
    margin: 0;
    line-height: 1;
    letter-spacing: 0.02em;
  }
  .dc-mark {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .dc-mark i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .dc-mark.on i {
    background: var(--good);
  }
  .note {
    margin: 6px 0 0;
    padding: 8px 12px;
    border-radius: var(--radius);
    background: var(--accent-fill-faint);
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .note a {
    color: var(--accent);
  }
  .prev {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    align-items: flex-start;
  }
  /* Discord's profile popout, drawn in our tokens. */
  .card {
    width: 330px;
    flex: 0 0 auto;
    box-sizing: border-box;
    padding: 10px 12px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .card.none {
    opacity: 0.55;
  }
  .play {
    font-size: var(--text-badge);
    font-weight: var(--weight-semibold);
  }
  .body {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .art {
    position: relative;
    flex: 0 0 auto;
    width: 72px;
    height: 72px;
  }
  .big {
    width: 72px;
    height: 72px;
    border-radius: 10px;
    object-fit: cover;
    display: block;
  }
  .big.empty {
    background: var(--bg-sunk);
  }
  .small {
    position: absolute;
    right: -6px;
    bottom: -6px;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 3px solid var(--bg-raised);
    background: var(--bg-raised);
  }
  .txt {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: var(--text-small);
  }
  .txt b {
    color: var(--text-strong);
  }
  .txt span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .time {
    font-family: var(--font-mono);
    color: var(--good);
  }
  .btn {
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-sunk);
    text-align: center;
    font-size: var(--text-small);
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 320px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .side b {
    font-weight: var(--weight-medium);
    color: var(--text-strong);
  }
  .side em {
    font-style: normal;
    color: var(--text);
  }
</style>
