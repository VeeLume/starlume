<script lang="ts">
  // Session log: the current game session (or a past one, `?s=<id>`).
  //
  // Three parts, the same in every state (running, ended, none), so nothing
  // moves when the state changes:
  //   1. the title line: the session in one line beside the page title;
  //   2. the status strip: four boxes of one height saying what is true now
  //      (where, ship, mission, wallet);
  //   3. the log: the session folded into rows on the kit surface.
  //
  // Reads the session store only (frontend rules 1, 2): the session lives
  // Rust-side, the store mirrors it and re-reads on `session:changed`.
  // Display names come from Rust too (`session_names`, cached in the store):
  // the page never shows one of the game's internal names. Search, filter,
  // sort and the opened session are in the URL (rule 7); expansion is
  // page-local. The folding itself is `readSession` (model.ts, tested).
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    Surface,
    Expand,
    createBrowseState,
    createExpansion,
    type FacetDef,
    type Status,
  } from "@veelume/ui";
  import RichText from "$lib/components/catalog/RichText.svelte";
  import type { Session } from "$lib/bindings";
  import { channelHue } from "$lib/channels";
  import {
    loadNames,
    openSession,
    replaySession,
    rereadSession,
    sessionStore,
    watchSession,
  } from "$lib/state/session.svelte";
  import {
    FACETS,
    amount,
    clock,
    count,
    day,
    duration,
    namesFrom,
    readSession,
    sessionLength,
    signed,
    walletLine,
    type Facet,
    type LogRow,
    type SessionReading,
  } from "$lib/modules/session/model";

  const browse = createBrowseState({
    q: { kind: "text" },
    // A past session's id; empty = the current one.
    s: { kind: "one", default: "", narrows: false },
    show: { kind: "one", default: "all" },
    // Empty = the session's own order: newest first while it runs, oldest
    // first (a story) once it is over.
    sort: { kind: "one", default: "", narrows: false },
  });
  const expanded = createExpansion("many");

  const pastId = $derived(browse.values.s);
  /** `undefined` = not read yet, `null` = none. */
  const shown = $derived<Session | null | undefined>(
    pastId
      ? sessionStore.opened(pastId)
      : sessionStore.state
        ? sessionStore.current
        : undefined,
  );

  $effect(() => {
    if (pastId) void openSession(pastId);
  });
  // Ask for the names this session holds and the store lacks (a no-op once
  // it has them; a live session asks again when a new name appears).
  $effect(() => {
    if (shown) void loadNames(shown);
  });

  // A live session's durations grow; nothing else on the page needs a clock.
  let now = $state(Date.now());
  onMount(() => {
    const stop = watchSession();
    const tick = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      stop();
      clearInterval(tick);
    };
  });

  const reading = $derived<SessionReading | null>(
    shown ? readSession(shown, namesFrom(sessionStore.names(shown)), now) : null,
  );
  const st = $derived(reading?.status ?? null);

  // Which source is off, for the box that stays empty because of it. A past
  // session says what fed it; the current one, what is switched on.
  const logOn = $derived(pastId ? (shown?.sources.log ?? true) : (sessionStore.state?.log_enabled ?? true));
  const telemetryOn = $derived(
    pastId ? (shown?.sources.telemetry ?? true) : (sessionStore.state?.telemetry_enabled ?? true),
  );

  const facets: FacetDef<LogRow>[] = [
    {
      id: "show",
      label: "Show",
      mode: "one",
      options: [
        { value: "all", label: "All" },
        ...FACETS.map((f) => ({
          value: f.value,
          label: f.label,
          test: (r: LogRow) => r.facets.includes(f.value),
        })),
      ],
    },
  ];

  const newest = {
    value: "newest",
    label: "Newest first",
    compare: (a: LogRow, b: LogRow) => b.at - a.at,
  };
  const oldest = {
    value: "oldest",
    label: "Oldest first",
    compare: (a: LogRow, b: LogRow) => a.at - b.at,
  };

  const descriptor = $derived({
    sources: () => reading,
    derive: (r: SessionReading | null): LogRow[] => r?.rows ?? [],
    searchIn: (r: LogRow) => [r.title, ...r.search],
    facets,
    // The first sort is the default.
    sorts: shown?.live ? [newest, oldest] : [oldest, newest],
  });

  const status = $derived<Status>(shown === undefined ? "loading" : "ready");

  const facetCount = (f: Facet) => reading?.rows.filter((r) => r.facets.includes(f)).length ?? 0;

  // The wallet line's box, in SVG user units (stretched to the box's width).
  const GRAPH_W = 300;
  const GRAPH_H = 36;
  const line = $derived(shown ? walletLine(shown, now, GRAPH_W, GRAPH_H) : null);

  const wallet = $derived(shown?.wallet ?? null);
  const walletNote = $derived.by(() => {
    if (!wallet) return null;
    if (wallet.zero_since != null) {
      return {
        text: `reads 0 since ${clock(wallet.zero_since)}`,
        title:
          "The game reports a balance of 0. It is not counted as a loss unless a purchase explains it; the figures are from before.",
      };
    }
    if (wallet.opened_after_zero) {
      return {
        text: "opened after a 0",
        title:
          "The first balance read was 0 and was skipped as a loading value, so the opening balance is the first one above 0.",
      };
    }
    return null;
  });

  const today = $derived(day(now));
  /** The shown session is over: an empty box will not fill any more. */
  const over = $derived(!!shown && !shown.live);

  /** A stay or a mission's length in the Took column. */
  function took(ms: number): string {
    return ms < 60_000 ? "<1 min" : duration(ms);
  }

  const STEP_GLYPH = { done: "✓", current: "●", open: "○", withdrawn: "✕" } as const;

  // Reading a past session's log again: page-local, one at a time.
  let rereading = $state(false);
  let rereadNote = $state("");
  // A note is about the session it was asked for.
  $effect(() => {
    void pastId;
    rereadNote = "";
  });
  // Dev builds: play a past session back as the live one, step by step, so
  // the page and Discord Rich Presence can be checked without playing.
  const dev = import.meta.env.DEV;
  const replay = $derived(pastId ? null : (sessionStore.state?.replay ?? null));
  let replayError = $state("");
  async function startReplay(id: string) {
    replayError = (await replaySession(id, 0)) ?? "";
    if (!replayError) await goto("/session");
  }
  async function stepTo(step: number) {
    if (!replay) return;
    replayError = (await replaySession(replay.id, Math.max(0, Math.min(replay.total, step)))) ?? "";
  }
  async function stopReplay() {
    replayError = (await replaySession(null, 0)) ?? "";
  }
  function replayKeys(e: KeyboardEvent) {
    if (!replay || e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
    const by = e.shiftKey ? 10 : 1;
    if (e.key === "ArrowRight") void stepTo(replay.step + by);
    else if (e.key === "ArrowLeft") void stepTo(replay.step - by);
    else return;
    e.preventDefault();
  }

  async function reread(id: string) {
    rereading = true;
    rereadNote = "";
    try {
      rereadNote = await rereadSession(id);
    } finally {
      rereading = false;
    }
  }
</script>

<svelte:window onkeydown={replayKeys} />

<div class="ss-page">
  <header class="ss-head">
    <h1>Session Log</h1>
    {#if shown}
      <span class="ss-mark" class:on={shown.live}><i></i>{shown.live ? "Running" : "Ended"}</span>
      <span class="ss-now">
        <b>{duration(sessionLength(shown, shown.live ? now : shown.last_activity_at))}</b>
        {#if shown.live}
          <span>since {clock(shown.started_at)}</span>
        {:else}
          <span>{clock(shown.started_at)} to {clock(shown.ended_at ?? shown.last_activity_at)}</span>
        {/if}
        {#if day(shown.started_at) !== today}
          <span>{day(shown.started_at)}</span>
        {/if}
        {#if shown.shards.length > 0}
          <span>{count(shown.shards.length, "shard")}</span>
        {/if}
        {#if shown.channel}
          <span class="hue-chip" style:--hue={channelHue(shown.channel)}>{shown.channel}</span>
        {/if}
      </span>
    {:else if shown === null}
      <span class="ss-mark"><i></i>{pastId ? "Not kept" : "No session"}</span>
    {/if}
    <span class="ss-links">
      {#if rereadNote}
        <span class="ss-note">{rereadNote}</span>
      {/if}
      {#if pastId && shown && shown.sources.log}
        <button
          class="btn subtle ss-reread"
          disabled={rereading}
          title="Read this session's Game.log again, with what this version of Starlume reads"
          onclick={() => reread(shown.id)}
        >
          {rereading ? "Reading Game.log…" : "Read Game.log again"}
        </button>
      {/if}
      {#if dev && pastId && shown && shown.timeline.length > 0}
        <button class="btn subtle ss-reread" title="Play this session back as the live one, step by step (dev builds)" onclick={() => startReplay(shown.id)}>
          Replay
        </button>
      {/if}
      {#if pastId}
        <a href="/session">Current session</a>
      {/if}
      <a href="/session/history">Past sessions</a>
    </span>
  </header>

  {#if replay}
    <div class="ss-replay" role="group" aria-label="Replay">
      <span class="ss-replay-lab">Replay</span>
      <button class="btn subtle" title="Back 10 (Shift+Left)" disabled={replay.step === 0} onclick={() => stepTo(replay.step - 10)}>«</button>
      <button class="btn subtle" title="Back one (Left)" disabled={replay.step === 0} onclick={() => stepTo(replay.step - 1)}>‹</button>
      <span class="ss-replay-at mono">{replay.step} of {replay.total}</span>
      <button class="btn subtle" title="Forward one (Right)" disabled={replay.step >= replay.total} onclick={() => stepTo(replay.step + 1)}>›</button>
      <button class="btn subtle" title="Forward 10 (Shift+Right)" disabled={replay.step >= replay.total} onclick={() => stepTo(replay.step + 10)}>»</button>
      <span class="ss-replay-what">
        {#if replay.kind}{replay.kind.replaceAll("_", " ")}{#if replay.at != null}<span>at {clock(replay.at)}</span>{/if}{:else}before the first entry{/if}
      </span>
      {#if replayError}<span class="ss-note">{replayError}</span>{/if}
      <button class="btn subtle ss-replay-stop" onclick={stopReplay}>Stop</button>
    </div>
  {/if}

  <div class="ss-strip">
    <section class="ss-box" aria-label="Where">
      <div class="ss-lab"><p class="label">Where</p></div>
      {#if st?.where}
        {#if st.where.leftAt != null}
          <!-- Left the last place; the log says where only at the next one. -->
          <div class="ss-big">{st.where.system ?? "Under way"}</div>
          <div class="ss-bottom">
            <span class="ss-sm"><span>left {st.where.name}</span></span>
            <span class="ss-sm f"><span>at {clock(st.where.leftAt)}</span></span>
          </div>
        {:else}
          <div class="ss-big">
            {st.where.name}{#if st.where.system}<span class="sys">{st.where.system}</span>{/if}
          </div>
          <div class="ss-bottom">
            <span class="ss-sm">
              {#each st.where.jurisdictions as j (j)}<span>{j}</span>{/each}
              {#if st.where.armistice}<span>armistice</span>{/if}
            </span>
            <span class="ss-sm f">
              <span>arrived {clock(st.where.arrivedAt)}</span>
              {#if !shown?.live}<span>last place</span>{/if}
            </span>
          </div>
        {/if}
      {:else}
        <div class="ss-big quiet">No place yet</div>
        <div class="ss-bottom">
          {#if !logOn}
            <a class="ss-sm" href="/settings/game-data"><span>Reading Game.log is off</span></a>
          {:else}
            <span class="ss-sm f"><span>{over ? "none in this session" : "shows at the first stop"}</span></span>
          {/if}
        </div>
      {/if}
    </section>

    <section class="ss-box" aria-label="Ship">
      <div class="ss-lab"><p class="label">Ship</p></div>
      {#if st?.ship.aboard}
        <div class="ss-big">{st.ship.aboard.name}</div>
        <div class="ss-bottom">
          <span class="ss-sm"><span>aboard</span></span>
          <span class="ss-sm f">
            <span>since {clock(st.ship.aboard.since)}</span>
            <span>{took(now - st.ship.aboard.since)}</span>
          </span>
        </div>
      {:else if st?.ship.last}
        <div class="ss-big">On foot</div>
        <div class="ss-bottom">
          <span class="ss-sm"><span>left {st.ship.last.name}</span></span>
          <span class="ss-sm f">
            <span>at {clock(st.ship.last.leftAt)}</span>
            {#if shown?.live}
              <span>in it {took(st.ship.last.took)}</span>
            {:else}
              <span>{count(st.ship.stays, "stay")}</span>
            {/if}
          </span>
        </div>
      {:else}
        <div class="ss-big quiet">No ship yet</div>
        <div class="ss-bottom">
          {#if !telemetryOn}
            <a class="ss-sm" href="/settings/game-data"><span>Reading telemetry is off</span></a>
          {:else}
            <span class="ss-sm f"><span>{over ? "none in this session" : "shows when you sit in one"}</span></span>
          {/if}
        </div>
      {/if}
    </section>

    <section class="ss-box" aria-label="Mission">
      <div class="ss-lab">
        <p class="label">Mission</p>
        {#if st?.mission.running}
          {#if st.mission.running.objectiveNo > 0}
            <span class="ss-r">objective {st.mission.running.objectiveNo}</span>
          {/if}
        {:else if st && st.mission.count > 0}
          <span class="ss-r">{st.mission.count} this session</span>
        {/if}
      </div>
      {#if st?.mission.running}
        {@const m = st.mission.running}
        <div class="ss-big"><RichText text={m.title} /></div>
        <div class="ss-bottom">
          {#if m.objective}
            <span class="ss-sm obj">→ <RichText text={m.objective} /></span>
          {/if}
          <span class="ss-sm f">
            {#if m.objectiveSince != null}<span>since {clock(m.objectiveSince)}</span>{/if}
            {#if m.acceptedAt != null}<span>accepted {clock(m.acceptedAt)}</span>{/if}
          </span>
        </div>
      {:else}
        <div class="ss-big quiet">No mission</div>
        <div class="ss-bottom">
          {#if st?.mission.last}
            {@const m = st.mission.last}
            <span class="ss-sm"><span>last: <RichText text={m.title} /></span></span>
            <span class="ss-sm f">
              <span>{m.outcome} {clock(m.endedAt)}</span>
              {#if m.amount != null}<span class="mono">{signed(m.amount)}</span>{/if}
            </span>
          {:else if !logOn}
            <a class="ss-sm" href="/settings/game-data"><span>Reading Game.log is off</span></a>
          {:else}
            <span class="ss-sm f"><span>{over ? "none in this session" : "shows when you accept one"}</span></span>
          {/if}
        </div>
      {/if}
    </section>

    <section class="ss-box b-wallet" aria-label="Wallet">
      <div class="ss-lab">
        <p class="label">Wallet</p>
        {#if walletNote}
          <span class="ss-note" title={walletNote.title}>{walletNote.text}</span>
        {/if}
        {#if wallet?.opening}
          <span class="ss-r mono">net {signed(wallet.net)}</span>
        {/if}
      </div>
      {#if wallet?.latest}
        <div class="ss-wal">
          <span class="ss-big fig">{amount(wallet.latest.balance)}</span>
          <span class="unit">aUEC</span>
        </div>
      {:else}
        <div class="ss-wal">
          <span class="ss-big fig quiet">–</span>
          {#if !telemetryOn}
            <a class="unit" href="/settings/game-data">Reading telemetry is off</a>
          {:else}
            <span class="unit">{over ? "no balance in this session" : "no balance read yet"}</span>
          {/if}
        </div>
      {/if}
      <div class="ss-graph">
        {#if line}
          <svg
            viewBox="0 0 {GRAPH_W} {GRAPH_H}"
            preserveAspectRatio="none"
            role="img"
            aria-label="The balance over the session, {wallet ? count(wallet.changes.length, 'change') : ''}"
          >
            <line class="base" x1="0" y1={line.base} x2={GRAPH_W} y2={line.base} />
            <path class="area" d={line.area} />
            <path class="step" d={line.step} />
          </svg>
          <div class="ticks" aria-hidden="true">
            {#each line.ticks as t (t.pos)}
              <span
                class:first={t.pos === 0}
                class:last={t.pos === 1}
                style:left="{t.pos * 100}%">{t.label}</span
              >
            {/each}
          </div>
        {/if}
      </div>
    </section>
  </div>

  {#if sessionStore.error}
    <p class="error">{sessionStore.error}</p>
  {/if}

  <Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
    <Surface.List {status}>
      {#snippet headerLeading()}
        <div class="pills ss-pills" role="group" aria-label="Show">
          <button
            type="button"
            class="pill"
            class:active={browse.values.show === "all"}
            aria-pressed={browse.values.show === "all"}
            onclick={() => browse.set("show", "all")}
          >
            All <span class="count">{reading?.rows.length ?? 0}</span>
          </button>
          {#each FACETS as f (f.value)}
            <button
              type="button"
              class="pill"
              class:active={browse.values.show === f.value}
              aria-pressed={browse.values.show === f.value}
              onclick={() => browse.set("show", f.value)}
            >
              {f.label} <span class="count">{facetCount(f.value)}</span>
            </button>
          {/each}
        </div>
      {/snippet}
      {#snippet row(r: LogRow)}
        <Expand.Row
          title={r.title}
          open={expanded.has(r.key)}
          ontoggle={() => expanded.toggle(r.key)}
        >
          {#snippet gutter()}
            <span class="ss-t k-{r.kind}">{clock(r.at)}</span>
          {/snippet}
          {#snippet meta()}
            {#if r.meta.length > 0}
              <span class="ss-meta">
                {#each r.meta as part, i (i)}
                  <span class:jur={part.chip}>{part.text}</span>
                {/each}
              </span>
            {/if}
          {/snippet}
          {#snippet right()}
            <span class="ss-cols">
              <span class="ss-d">
                {#if r.took != null}
                  {#if r.running}<i></i>{/if}{took(r.took)}
                {/if}
              </span>
              <span class="ss-v" class:plus={(r.amount ?? 0) > 0}>
                {r.amount != null ? signed(r.amount) : ""}
              </span>
            </span>
          {/snippet}
          <div class="ss-detail">
            {#if r.steps.length > 0}
              <ol class="ss-steps">
                {#each r.steps as step, i (i)}
                  <li class={step.state}>
                    <span class="mono">{clock(step.at)}</span>
                    <span class="g">{STEP_GLYPH[step.state]}</span>
                    <span><RichText text={step.text} /></span>
                    <span class="state">{step.state === "current" ? "current" : ""}</span>
                  </li>
                {/each}
              </ol>
            {/if}
            {#if r.facts.length > 0}
              <Expand.Facts facts={r.facts} />
            {/if}
            {#if r.sources.length > 0}
              <Expand.Facts facts={r.sources} />
            {/if}
            {#if r.steps.length + r.facts.length + r.sources.length === 0}
              <p class="dim">Nothing more is recorded for this row.</p>
            {/if}
          </div>
        </Expand.Row>
      {/snippet}
      {#snippet empty()}
        <p class="ss-empty dim">
          {#if shown === null && pastId}
            This session is no longer kept.
          {:else if shown === null}
            No session yet. One starts when the game does.
          {:else}
            Nothing in this session matches.
          {/if}
        </p>
      {/snippet}
    </Surface.List>
  </Surface.Root>

  <footer class="ss-foot">
    {#if reading && shown}
      <span>{count(reading.entries, "log entry", "log entries")}, read as {count(reading.rows.length, "row")}</span>
      {#if shown.timeline_dropped > 0}
        <span>{shown.timeline_dropped} older entries dropped</span>
      {/if}
      {#if reading.changes > 0}
        <span>
          {count(reading.changes, "balance change")},
          {reading.explained === reading.changes
            ? "all explained by a row"
            : `${reading.explained} explained by a row`}
        </span>
        <span class="ss-grow"></span>
        <span class="net">{signed(shown.wallet.net)}</span>
      {/if}
    {/if}
  </footer>
</div>

<style>
  .ss-page {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .ss-grow {
    flex: 1 1 0;
    min-width: 0;
  }
  /* app.css gives every list under `main` bullets and an indent, for the
   * hand-rolled pages; the kit's row list is not one of those. */
  .ss-page :global(ul) {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  /* ── Title line: one centre line; the session line is one text size ── */
  .ss-head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 14px;
    min-height: 26px;
  }
  .ss-head h1 {
    margin: 0;
    line-height: 1;
    letter-spacing: 0.02em;
  }
  .ss-links {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 14px;
  }
  .ss-replay {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border: 1px dashed var(--accent-line);
    border-radius: var(--radius);
    background: var(--accent-fill-faint);
    font-size: var(--text-small);
  }
  .ss-replay-lab {
    font-weight: var(--weight-semibold);
    color: var(--accent);
    margin-right: 6px;
  }
  .ss-replay-at {
    min-width: 72px;
    text-align: center;
  }
  .ss-replay-what {
    display: inline-flex;
    gap: 8px;
    margin-left: 8px;
    color: var(--text-strong);
  }
  .ss-replay-what span {
    color: var(--text-dim);
  }
  .ss-replay-stop {
    margin-left: auto;
  }
  .ss-reread {
    font-size: var(--text-small);
  }
  .ss-note {
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .ss-head a {
    font-size: var(--text-small);
    color: var(--accent);
    white-space: nowrap;
  }
  .ss-mark,
  .ss-now {
    display: inline-flex;
    align-items: center;
    font-size: var(--text-small);
    line-height: 1;
    color: var(--text-dim);
  }
  .ss-mark {
    gap: 6px;
    white-space: nowrap;
  }
  .ss-mark i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .ss-mark.on i {
    background: var(--good);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--good) 22%, transparent);
  }
  .ss-now {
    flex-wrap: wrap;
    gap: 6px 14px;
  }
  .ss-now > * {
    white-space: nowrap;
  }
  .ss-now b {
    font: inherit;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }

  /* ── Status strip: four boxes, one fixed height in every state ── */
  .ss-strip {
    display: grid;
    grid-template-columns:
      minmax(0, 1.1fr) minmax(0, 1.1fr) minmax(0, 1.25fr)
      minmax(290px, 1.3fr);
    gap: 10px;
    flex: none;
  }
  .ss-box {
    height: 132px;
    padding: 11px 14px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    overflow: hidden;
  }
  .ss-box.b-wallet {
    gap: 2px;
  }
  .ss-lab {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 18px;
  }
  .ss-lab .label {
    flex: none;
    margin: 0;
  }
  .ss-r {
    margin-left: auto;
    font-size: 0.75rem;
    color: var(--text-faint);
    white-space: nowrap;
  }
  .ss-r.mono {
    font-family: var(--font-mono);
    color: var(--text-dim);
    word-break: normal;
  }
  .ss-note {
    min-width: 0;
    font-size: 0.72rem;
    color: var(--warn);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: help;
  }
  .ss-big {
    margin-top: 4px;
    font-size: 1.05rem;
    font-weight: var(--weight-semibold);
    line-height: 1.3;
    color: var(--text-strong);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .ss-big .sys {
    color: var(--text-dim);
    font-weight: var(--weight-normal);
    margin-left: 6px;
  }
  .ss-big.quiet {
    color: var(--text-dim);
  }
  .ss-big.fig {
    font-family: var(--font-display);
    font-size: 1.75rem;
    line-height: 1.1;
    letter-spacing: 0.01em;
    font-variant-numeric: tabular-nums;
  }
  .ss-bottom {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .ss-sm {
    display: flex;
    gap: 12px;
    min-height: 1.45em;
    font-size: var(--text-small);
    line-height: 1.45;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ss-sm > span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ss-sm.f {
    color: var(--text-faint);
  }
  .ss-sm.obj {
    display: block;
    color: var(--text);
  }
  .ss-sm .mono {
    word-break: normal;
  }
  a.ss-sm,
  .ss-wal a.unit {
    color: var(--accent);
  }
  .ss-wal {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .ss-wal .ss-big {
    margin: 0;
    flex: none;
    display: block;
  }
  .ss-wal .unit {
    font-size: 0.8rem;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* The balance over the session: a plain step line, no marks or labels on
   * it; three time ticks under it. The box keeps its height without one. */
  .ss-graph {
    margin-top: auto;
    height: 52px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .ss-graph svg {
    display: block;
    width: 100%;
    height: 36px;
    overflow: visible;
  }
  .ss-graph .base {
    stroke: var(--border);
    stroke-width: 1;
    stroke-dasharray: 3 3;
    vector-effect: non-scaling-stroke;
  }
  .ss-graph .area {
    fill: color-mix(in srgb, var(--accent) 9%, transparent);
  }
  .ss-graph .step {
    fill: none;
    stroke: color-mix(in srgb, var(--accent) 70%, transparent);
    stroke-width: 1.5;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }
  .ss-graph .ticks {
    position: relative;
    height: 13px;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    line-height: 13px;
    color: var(--text-faint);
  }
  .ss-graph .ticks span {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    white-space: nowrap;
  }
  .ss-graph .ticks .first {
    transform: none;
  }
  .ss-graph .ticks .last {
    transform: translateX(-100%);
  }

  /* ── The log's rows (inside the kit's Expand.Row slots) ── */
  .ss-t {
    /* The kit's gutter is flush with the list's edge: step in from it. */
    position: relative;
    left: 4px;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: color-mix(in srgb, var(--hue, var(--text-faint)) 55%, var(--text-faint));
    font-variant-numeric: tabular-nums;
  }
  .k-place {
    --hue: var(--hue-sky);
  }
  .k-ship {
    --hue: var(--hue-teal);
  }
  .k-mission {
    --hue: var(--hue-rose);
  }
  .k-buy,
  .k-money {
    --hue: var(--hue-sand);
  }
  .ss-meta {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 14px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .ss-meta .jur {
    font-size: 0.72rem;
    color: var(--text-dim);
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    white-space: nowrap;
  }
  .ss-cols {
    display: inline-grid;
    grid-template-columns: 76px 104px;
    gap: 12px;
    align-items: baseline;
    font-family: var(--font-mono);
    text-align: right;
    white-space: nowrap;
  }
  .ss-d {
    display: inline-flex;
    justify-content: flex-end;
    align-items: center;
    gap: 6px;
    font-size: 0.78rem;
    color: var(--text-dim);
  }
  .ss-d i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--good);
  }
  .ss-v {
    font-size: 0.84rem;
    color: var(--text);
  }
  .ss-v.plus {
    color: var(--good);
  }
  .ss-detail {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 2px;
  }
  .ss-steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    max-width: 72ch;
  }
  .ss-steps li {
    display: grid;
    grid-template-columns: 44px 18px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: baseline;
    padding: 3px 0;
    font-size: 0.875rem;
    color: var(--text-dim);
  }
  .ss-steps .mono {
    color: var(--text-faint);
    word-break: normal;
  }
  .ss-steps .g {
    color: var(--text-faint);
    text-align: center;
  }
  .ss-steps li.current {
    color: var(--text-strong);
  }
  .ss-steps li.current .g {
    color: var(--hue-rose);
  }
  .ss-steps li.withdrawn {
    color: var(--text-faint);
  }
  .ss-steps .state {
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .ss-empty {
    margin: 0;
    padding: 24px;
    text-align: center;
  }

  /* ── Foot: what the rows were read from, and the wallet's total ── */
  .ss-foot {
    flex: none;
    display: flex;
    align-items: center;
    gap: 18px;
    height: 20px;
    padding: 0 12px;
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
  }
  .ss-foot > span:not(.ss-grow):not(.net) {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ss-foot .net {
    flex: none;
    font-family: var(--font-mono);
    font-size: 0.84rem;
    color: var(--text-strong);
  }

  @media (max-width: 1100px) {
    .ss-strip {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
    .ss-box.b-wallet {
      grid-column: auto;
    }
  }
  @media (max-width: 720px) {
    /* The filter button's panel has the same choice; search needs the room. */
    .ss-pills {
      display: none;
    }
    .ss-cols {
      grid-template-columns: 96px;
    }
    .ss-d {
      display: none;
    }
  }
  @media (max-width: 560px) {
    .ss-strip {
      grid-template-columns: minmax(0, 1fr);
    }
    .ss-box {
      height: 112px;
    }
  }
</style>
