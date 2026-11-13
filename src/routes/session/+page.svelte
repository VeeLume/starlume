<script lang="ts">
  // Session log: the current game session (or a past one, `?s=<id>`) as a
  // compact header and a timeline on the kit surface. A first plain page in
  // the catalog idiom; the designed version comes from a mock.
  //
  // Reads the session store only (frontend rules 1, 2): the session lives
  // Rust-side, the store mirrors it and re-reads on `session:changed`.
  // Search, filter, sort and the opened session are in the URL (rule 7);
  // expansion is page-local.
  import { onMount } from "svelte";
  import {
    Surface,
    Expand,
    createBrowseState,
    createExpansion,
    type FacetDef,
    type Fact,
    type Status,
  } from "@veelume/ui";
  import RichText from "$lib/components/catalog/RichText.svelte";
  import type { Session } from "$lib/bindings";
  import { openSession, sessionStore, watchSession } from "$lib/state/session.svelte";
  import {
    ENTRY_GROUPS,
    auec,
    clock,
    dayTime,
    duration,
    missingSources,
    sessionLength,
    signedAuec,
    sourceName,
    timelineRows,
    type TimelineRow,
  } from "$lib/modules/session/model";

  const browse = createBrowseState({
    q: { kind: "text" },
    // A past session's id; empty = the current one.
    s: { kind: "one", default: "", narrows: false },
    show: { kind: "many" },
    sort: { kind: "one", default: "newest", narrows: false },
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

  // A live session's length grows; nothing else on the page needs a clock.
  let now = $state(Date.now());
  onMount(() => {
    const stop = watchSession();
    const tick = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      stop();
      clearInterval(tick);
    };
  });

  const facets: FacetDef<TimelineRow>[] = [
    {
      id: "show",
      label: "Show",
      mode: "many",
      options: ENTRY_GROUPS.map((g) => ({
        value: g.value,
        label: g.label,
        test: (r: TimelineRow) => r.group === g.value,
      })),
    },
  ];

  const descriptor = {
    sources: () => shown ?? null,
    derive: (session: Session | null): TimelineRow[] => (session ? timelineRows(session) : []),
    searchIn: (r: TimelineRow) => [r.title, r.subject, r.note],
    facets,
    sorts: [
      {
        value: "newest",
        label: "Newest first",
        compare: (a: TimelineRow, b: TimelineRow) => b.at - a.at,
      },
      {
        value: "oldest",
        label: "Oldest first",
        compare: (a: TimelineRow, b: TimelineRow) => a.at - b.at,
      },
    ],
  };

  const status = $derived<Status>(shown === undefined ? "loading" : "ready");

  const sources = $derived(sessionStore.state);
  const shard = $derived(shown?.shards[shown.shards.length - 1] ?? null);
  const build = $derived(
    shown ? [shown.channel, shown.branch, shown.build].filter((p) => p).join(" · ") : "",
  );

  function facts(r: TimelineRow): Fact[] {
    const out: Fact[] = [
      { label: "When", value: dayTime(r.at) },
      { label: "Source", value: sourceName(r.source) },
    ];
    const k = r.kind;
    if (k?.type === "trade") {
      out.push(
        { label: "Shop", value: k.shop, mono: true },
        { label: "Quantity", value: `${k.quantity}${k.quantity_unit ? ` ${k.quantity_unit}` : ""}` },
        { label: "Resource", value: k.resource_guid, mono: true },
      );
    } else if (k?.type === "shop_purchase") {
      out.push(
        { label: "Shop", value: k.shop, mono: true },
        { label: "Item", value: k.item_guid, mono: true },
      );
    } else if (k?.type === "vehicle_entered" || k?.type === "vehicle_left") {
      out.push({ label: "Vehicle", value: k.name, mono: true });
    } else if (
      (k?.type === "mission_accepted" || k?.type === "objective" || k?.type === "mission_ended") &&
      k.mission_id
    ) {
      out.push({ label: "Mission", value: k.mission_id, mono: true });
    }
    if (r.stated != null) out.push({ label: "Log says", value: signedAuec(r.stated) });
    if (r.change) {
      out.push({
        label: "Balance",
        value: `${auec(r.change.from)} → ${auec(r.change.to)}`,
      });
    }
    return out;
  }
</script>

<div class="flex h-full min-h-0 flex-col gap-3">
  <header class="head">
    <div class="head-line">
      <h1>Session log</h1>
      {#if shown}
        <span class="status-pill" class:good={shown.live}>
          <i></i>{shown.live ? "Live" : "Ended"}
        </span>
      {/if}
      <span class="links">
        {#if pastId}
          <a href="/session">Current session</a>
        {/if}
        <a href="/session/history">Past sessions</a>
      </span>
    </div>

    {#if sources && !pastId}
      {#if !sources.log_enabled && !sources.telemetry_enabled}
        <p class="dim">
          Nothing is recorded: reading Game.log and reading the game's telemetry are both
          off. Turn on either in <a href="/settings/game-data">Settings → Game data</a>.
        </p>
      {:else if !sources.log_enabled}
        <p class="dim">
          Reading Game.log is off, so sessions have no shards, places, missions or
          purchases. Turn it on in <a href="/settings/game-data">Settings → Game data</a>.
        </p>
      {:else if !sources.telemetry_enabled}
        <p class="dim">
          Reading the game's telemetry is off, so sessions have no wallet, vehicles or
          deaths. Turn it on in <a href="/settings/game-data">Settings → Game data</a>.
        </p>
      {/if}
    {/if}

    {#if shown}
      <p class="dim">
        Started {dayTime(shown.started_at)}{build ? ` · ${build}` : ""}
        {#if pastId && missingSources(shown.sources)}
          · {missingSources(shown.sources)}
        {/if}
      </p>
      <div class="stat-tiles">
        <div class="stat-tile">
          <b>{duration(sessionLength(shown, now))}</b>
          <span>Duration</span>
        </div>
        <div class="stat-tile">
          <b title={shard?.shard}>{shard?.shard ?? "–"}</b>
          <span>Shard</span>
        </div>
        <div class="stat-tile">
          <b>{shown.wallet.opening ? auec(shown.wallet.opening.balance) : "–"}</b>
          <span>Opening balance</span>
        </div>
        <div class="stat-tile">
          <b>{shown.wallet.latest ? auec(shown.wallet.latest.balance) : "–"}</b>
          <span>Latest balance</span>
        </div>
        <div class="stat-tile">
          <b>{shown.wallet.opening ? signedAuec(shown.wallet.net) : "–"}</b>
          <span>Net</span>
        </div>
      </div>

      {#if shown.shards.length > 1}
        <div class="tags">
          {#each shown.shards as stay (stay.joined_at)}
            <span class="tag">
              {stay.shard} · {duration((stay.left_at ?? now) - stay.joined_at)}
            </span>
          {/each}
        </div>
      {/if}

      {#if shown.wallet.zero_since != null}
        <p class="dim">
          The game reports a balance of 0 since {clock(shown.wallet.zero_since)}. It is not
          counted as a loss unless a purchase explains it; the figures above are from
          before.
        </p>
      {/if}
      {#if shown.wallet.opened_after_zero}
        <p class="dim">
          The first balance read was 0 and was skipped as a loading value, so the opening
          balance is the first one above 0.
        </p>
      {/if}
      {#if shown.timeline_dropped > 0}
        <p class="dim">
          {shown.timeline_dropped} older entries were dropped to keep this session small.
        </p>
      {/if}
    {:else if shown === null}
      <p class="dim">
        {pastId
          ? "This session is no longer kept."
          : "No session yet. One starts when the game does."}
      </p>
    {/if}
    {#if sessionStore.error}
      <p class="error">{sessionStore.error}</p>
    {/if}
  </header>

  <Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
    <Surface.List {status}>
      {#snippet row(r: TimelineRow)}
        <Expand.Row
          title={r.title}
          open={expanded.has(r.key)}
          ontoggle={() => expanded.toggle(r.key)}
        >
          {#snippet meta()}
            {#if r.subject}
              <RichText text={r.subject} />
            {/if}
            {#if r.note}
              <span class="badge">{r.note}</span>
            {/if}
            {#if r.localeDependent}
              <span class="badge" title="Read from English game text">HUD text</span>
            {/if}
          {/snippet}
          {#snippet right()}
            {#if r.change}
              <span class="metric">{signedAuec(r.change.delta)}</span>
            {:else if r.stated != null}
              <span class="metric">{signedAuec(r.stated)}</span>
            {/if}
            <span class="metric">{clock(r.at)}</span>
          {/snippet}
          <Expand.Facts facts={facts(r)} />
        </Expand.Row>
      {/snippet}
      {#snippet empty()}
        <p class="dim">Nothing recorded in this session.</p>
      {/snippet}
    </Surface.List>
  </Surface.Root>
</div>

<style>
  .head {
    display: grid;
    gap: 8px;
  }
  .head-line {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .links {
    margin-left: auto;
    display: flex;
    gap: 14px;
  }
</style>
