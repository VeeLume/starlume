<script lang="ts">
  // The part picker, laid over the cards (the mining picker's pattern; the
  // erkul / spviewer dialogs as references): every part that fits the port,
  // its own figures in sortable columns (the best of each column marked),
  // and what the ship looks like with it fitted — the impact column follows
  // the part's kind (free pips for a plant, cooling for a cooler, shield HP
  // for a shield, the 32 Gm jump for a quantum drive). The columns are the
  // figures parts are chosen by, per type (./figures, shared with the part
  // lines). Search filters; Esc, a click outside or Cancel closes; a row
  // fits the part.
  import { X } from "lucide-svelte";
  import { Segmented } from "@veelume/ui";
  import type { CandidateView, OfferView, TourView } from "$lib/bindings";
  import { damageType, figuresFor, SIGNALS } from "./figures";

  let {
    title,
    sub,
    list,
    error,
    count,
    coolantAsked = null,
    tour = null,
    onpick,
    onclose,
  }: {
    title: string;
    sub: string;
    list: CandidateView[] | null;
    error: string;
    /** How many ports the pick goes to. */
    count: number;
    /** The coolant the ship asks for now, /s: what a cooler has to carry
     *  on its own. */
    coolantAsked?: number | null;
    /** The ship's moon tour (which planet, how many hops): names the
     *  quantum drive's tour column. */
    tour?: TourView | null;
    onpick: (item: string | null) => void;
    onclose: () => void;
  } = $props();

  let q = $state("");
  let sort = $state<{ key: string; dir: number } | null>(null);
  let el = $state<HTMLElement>();

  const query = $derived(q.trim().toLowerCase());
  const kind = $derived(list?.[0]?.itemType ?? "");
  /** The type's figures that at least one option has. */
  const columns = $derived(figuresFor(kind).filter((f) => (list ?? []).some((c) => f.get(c.stats) !== null)));
  const impact = $derived.by(() => {
    switch (kind) {
      case "PowerPlant":
        // Power-on hands out every pip, so free pips stay 0: the supply
        // is what a plant changes.
        return { label: "Ship supply", get: (c: CandidateView) => c.impact.supply, fmt: (v: number) => `${v} pips`, better: 1 };
      case "Cooler":
        return { label: "Cooling", get: (c: CandidateView) => c.impact.cooling, fmt: (v: number) => `${Math.round(v * 100)} %`, better: -1 };
      case "Shield":
        return { label: "Ship shield", get: (c: CandidateView) => c.impact.shieldHealth, fmt: (v: number) => v.toLocaleString("en-US"), better: 1 };
      case "WeaponGun":
        // The weapons panel's figure: every gun's sustained DPS together,
        // under the power the pool gives them with this gun fitted.
        return {
          label: "Ship DPS",
          get: (c: CandidateView) => c.impact.gunDps,
          fmt: (v: number) => Math.round(v).toLocaleString("en-US"),
          better: 1,
        };
      case "QuantumDrive":
        return {
          label: "32 Gm jump",
          get: (c: CandidateView) => c.impact.quantum32Gm,
          fmt: (v: number) => `${Math.floor(v / 60)}:${String(Math.round(v % 60)).padStart(2, "0")}`,
          better: -1,
        };
      default:
        return null;
    }
  });
  /** More of the ship with the part fitted: a quantum drive's moon tour
   *  (short hops, each waiting out the last one's cooldown) and long jump
   *  (spool-up and acceleration make speed a poor guide to either), and
   *  how far the ship's fuel takes it. */
  const mmss = (v: number) => `${Math.floor(v / 60)}:${String(Math.round(v % 60)).padStart(2, "0")}`;
  const stat = (c: CandidateView, k: string) => c.stats.find((s) => s.key === k)?.value ?? null;
  type Extra = {
    key: string;
    label: string;
    hint?: string;
    get: (c: CandidateView) => number | null | undefined;
    fmt: (v: number) => string;
    better: number;
    /** A cell's tooltip. */
    title?: (c: CandidateView) => string | undefined;
  };
  /** Where to buy it (UEX's community prices): the cheapest shop, every
   *  shop on hover with how old its report is. A column only when some
   *  option has a known shop. */
  const ago = (t: number) => {
    const d = (Date.now() / 1000 - t) / 86400;
    return d < 1 ? "today" : d < 2 ? "yesterday" : `${Math.floor(d)} d ago`;
  };
  const aUEC = (v: number) =>
    v >= 1e6 ? `${(v / 1e6).toLocaleString("en-US", { maximumFractionDigits: 2 })}M` : v >= 1e4 ? `${Math.round(v / 1000)}k` : Math.round(v).toLocaleString("en-US");
  const BUY: Extra = {
    key: "_buy",
    label: "Buy aUEC",
    hint: "The cheapest shop, from UEX's player-reported prices; hover a price for every shop",
    get: (c) => c.shops[0]?.price ?? null,
    fmt: aUEC,
    better: -1,
    title: (c) => (c.shops.length ? shopsText(c.shops) : undefined),
  };
  /** The shops as a tooltip: per price (cheapest first), per system, the
   *  places by their short name; how old the reports are, once. Most parts
   *  cost the same everywhere, so this stays a few lines. */
  function shopsText(shops: OfferView[]): string {
    const byPrice = new Map<number, OfferView[]>();
    for (const s of shops) byPrice.set(Math.round(s.price), [...(byPrice.get(Math.round(s.price)) ?? []), s]);
    const lines: string[] = [];
    for (const [price, at] of byPrice) {
      lines.push(`${price.toLocaleString("en-US")} aUEC, ${at.length} shop${at.length === 1 ? "" : "s"}`);
      const bySystem = new Map<string, string[]>();
      for (const s of at) {
        const parts = s.place ? s.place.split(" · ") : [];
        const system = parts.length > 1 ? parts[parts.length - 1] : "Elsewhere";
        bySystem.set(system, [...(bySystem.get(system) ?? []), parts[0] ?? s.terminal]);
      }
      for (const [system, sites] of bySystem) lines.push(`  ${system}: ${[...new Set(sites)].join(", ")}`);
    }
    const ages = shops.map((s) => s.updated);
    const newest = ago(Math.max(...ages));
    const oldest = ago(Math.min(...ages));
    lines.push(`Reported ${newest === oldest ? newest : `${newest} to ${oldest}`}`);
    return lines.join("\n");
  }
  /** What players ask for it on UEX's marketplace: the plain item's
   *  average (not a crafted quality tier's: those are single listings, a
   *  quality-7 M7A asked 50M on 2026-10-23), now when anything is listed,
   *  else this week's or month's. Every tier and side on hover. */
  const asked = (c: CandidateView) => {
    const sell = c.market.find((m) => m.side === "sell" && m.tier === 0);
    return sell ? sell.now || sell.week || sell.month || null : null;
  };
  const MARKET: Extra = {
    key: "_market",
    label: "Market aUEC",
    hint: "What players ask on UEX's marketplace (average); hover for every quality tier and what players offer",
    get: asked,
    fmt: aUEC,
    better: -1,
    title: (c) =>
      c.market.length
        ? c.market
            .map(
              (m) =>
                `${m.side === "sell" ? "Players sell" : "Players buy"}${m.tier ? `, quality ${m.tier}` : ""}: ${aUEC(m.now || m.week || m.month)}` +
                `${m.listings ? ` (${m.listings} listed)` : m.now ? "" : m.week ? " (this week)" : " (this month)"}`,
            )
            .join("\n")
        : undefined,
  };
  const extras = $derived([
    ...kindExtras(),
    ...((list ?? []).some((c) => c.shops.length) ? [BUY] : []),
    ...((list ?? []).some((c) => asked(c) !== null) ? [MARKET] : []),
  ]);
  function kindExtras(): Extra[] {
    if (kind === "QuantumDrive")
      return [
        ...(tour
          ? [
              {
                key: "_tour",
                label: `${tour.planet} moons`,
                hint: `${tour.legs} hops, ${tour.gm.toFixed(1)} Gm in all: spool-up and travel each, and the cooldown before every next hop`,
                get: (c: CandidateView) => c.impact.quantumTour,
                fmt: mmss,
                better: -1,
              },
            ]
          : []),
        { key: "_q60", label: "60 Gm jump", get: (c) => c.impact.quantum60Gm, fmt: mmss, better: -1 },
        { key: "_range", label: "Range Gm", get: (c) => c.impact.quantumRangeGm, fmt: (v) => Math.round(v).toLocaleString("en-US"), better: 1 },
      ];
    // What the guns get of what they ask with this one fitted: a hungrier
    // gun starves them all (the pool feeds every gun alike).
    if (kind === "WeaponGun")
      return [{ key: "_gpow", label: "Gun power", get: (c) => c.impact.gunPower, fmt: (v) => `${Math.round(v * 100)} %`, better: 1 }];
    // Whether one cooler carries the ship by itself: what the ship asks
    // for now over what this cooler makes at full power (100 % or less:
    // it does).
    if (kind === "Cooler" && coolantAsked)
      return [
        {
          key: "_alone",
          label: "Alone",
          get: (c) => {
            const made = stat(c, "coolant");
            return made ? coolantAsked / made : null;
          },
          fmt: (v) => `${Math.round(v * 100)} %`,
          better: -1,
        },
      ];
    return [];
  }
  const fitted = $derived(list?.find((c) => c.fitted) ?? null);
  /** Size and class earn a column only when they tell options apart (a
   *  gun's grade never does). */
  const showSize = $derived(new Set((list ?? []).map((c) => c.size)).size > 1);
  const showGrade = $derived(
    kind !== "WeaponGun" && new Set((list ?? []).map((c) => `${c.class ?? ""}${c.grade}`)).size > 1,
  );
  const gradeOf = (g: number) => (g >= 1 && g <= 4 ? "ABCD"[g - 1] : "—");
  const classOf = (c: { class: string | null; grade: number }) =>
    [c.class ?? "", c.grade >= 1 && c.grade <= 4 ? gradeOf(c.grade) : ""].filter(Boolean).join(" ") || "—";

  const value = (c: CandidateView, key: string) => columns.find((f) => f.key === key)?.get(c.stats) ?? null;

  /** The way the game's shops shelve parts: by class, then grade
   *  (Military A first). It orders the list until a column is sorted, and
   *  breaks ties after. */
  const CLASSES = ["Military", "Stealth", "Competition", "Civilian", "Industrial"];
  const shelf = (c: CandidateView) => {
    const ci = c.class ? CLASSES.indexOf(c.class) : -1;
    return (ci < 0 ? CLASSES.length : ci) * 10 + c.grade;
  };

  /** Narrowing by what tells the options apart, as the shops' and
   *  spviewer's filters do: class, grade, a missile's tracking, a gun's
   *  damage type. A facet shows only when the list has two values of it. */
  const FACETS: { id: string; of: (c: CandidateView) => string; order: string[] }[] = [
    { id: "class", of: (c) => c.class ?? "", order: CLASSES },
    { id: "grade", of: (c) => (c.grade >= 1 && c.grade <= 4 ? "ABCD"[c.grade - 1] : ""), order: ["A", "B", "C", "D"] },
    { id: "tracking", of: (c) => SIGNALS[stat(c, "tracking") ?? -1] ?? "", order: ["IR", "EM", "CS"] },
    { id: "damage", of: (c) => (c.itemType === "WeaponGun" ? damageType(c.stats) : ""), order: ["Ballistic", "Energy", "Distortion"] },
  ];
  let picked = $state<Record<string, string>>({});
  /** Owned copies free to fit (My Items' snapshot), and the narrowing to
   *  them; shown only when the snapshot has any of these parts. */
  const ownedOf = (c: CandidateView) => (c.owned ?? []).reduce((n, o) => n + o.count, 0);
  const anyOwned = $derived((list ?? []).some((c) => ownedOf(c) > 0));
  let mine = $state("");
  const facets = $derived(
    FACETS.map((f) => {
      const have = new Set((list ?? []).map(f.of).filter(Boolean));
      return { ...f, values: f.order.filter((v) => have.has(v)) };
    }).filter((f) => f.values.length > 1),
  );

  /** Where a part type's list starts: the figure it is chosen by first
   *  (a plant's output, a gun's DPS, a drive's jump time while the jumps
   *  are known, else its speed). */
  const FIRST: Record<string, string> = {
    WeaponGun: "dps",
    Shield: "hp",
    PowerPlant: "output",
    Cooler: "coolant",
    Radar: "aimPip",
    QuantumDrive: "_impact",
    Missile: "damage",
    WeaponDefensive: "ammo",
    FuelTank: "capacity",
    QuantumFuelTank: "capacity",
  };
  const first = $derived.by(() => {
    let key = FIRST[kind];
    if (key === "_impact" && !(list ?? []).some((c) => impact?.get(c) != null)) key = "qspeed";
    if (!key) return null;
    const better = key === "_impact" ? (impact?.better ?? 1) : columns.find((f) => f.key === key)?.better;
    return better === undefined ? null : { key, dir: better < 0 ? -1 : 1 };
  });
  const order = $derived(sort ?? first);

  /** A column's figure for one option, whichever kind of column it is. */
  function getter(key: string): (c: CandidateView) => number | null {
    const extra = extras.find((x) => x.key === key);
    if (key === "_impact") return (c) => impact?.get(c) ?? null;
    if (extra) return (c) => extra.get(c) ?? null;
    return (c) => value(c, key);
  }

  const rows = $derived.by(() => {
    const shown = (list ?? [])
      .filter((c) => !query || `${c.name} ${c.recordName}`.toLowerCase().includes(query))
      .filter((c) => facets.every((f) => !picked[f.id] || f.of(c) === picked[f.id]))
      .filter((c) => !mine || ownedOf(c) > 0);
    if (!order) return [...shown].sort((a, b) => shelf(a) - shelf(b));
    const { key, dir } = order;
    const get = getter(key);
    // An option without the figure goes last whichever way the sort runs
    // (a part nobody sells is not the cheapest).
    return [...shown].sort((a, b) => {
      const va = get(a);
      const vb = get(b);
      if (va === null || vb === null) return (va === null ? 1 : 0) - (vb === null ? 1 : 0) || shelf(a) - shelf(b);
      return (vb - va) * dir || shelf(a) - shelf(b);
    });
  });
  /** One list, no group headings (the Class column says it). */
  const groups = $derived([{ label: "", rows }]);
  /** A value as the column shows it: the data's float noise (17.999998 s
   *  next to 18.000004 s) must not make a best of one "18" and not the
   *  next. */
  const shown = (v: number, digits: number) => Math.round(v * 10 ** digits) / 10 ** digits;
  const best = $derived(
    Object.fromEntries(
      columns.map((col) => {
        const vs = (list ?? [])
          .map((c) => value(c, col.key))
          .filter((v): v is number => v !== null)
          .map((v) => shown(v, col.digits));
        // All alike: nothing is best, so nothing is marked (a column of
        // green says nothing).
        if (new Set(vs).size < 2) return [col.key, null];
        return [col.key, col.better > 0 ? Math.max(...vs) : col.better < 0 ? Math.min(...vs) : null];
      }),
    ),
  );

  function sortBy(key: string, better: number) {
    sort = order?.key === key ? { key, dir: -order.dir } : { key, dir: better < 0 ? -1 : 1 };
  }

  function delta(c: CandidateView): { text: string; tone: "good" | "bad" | "" } {
    if (!impact) return { text: "", tone: "" };
    const a = impact.get(c);
    const b = fitted ? impact.get(fitted) : null;
    if (a === null || a === undefined || b === null || b === undefined || c.fitted) return { text: "", tone: "" };
    const d = a - b;
    if (Math.abs(d) < 1e-6) return { text: "±0", tone: "" };
    const mag = kind === "Cooler" ? `${Math.round(Math.abs(d) * 100)} %` : kind === "QuantumDrive" ? `${Math.round(Math.abs(d))} s` : Math.round(Math.abs(d)).toLocaleString("en-US");
    return { text: `${d > 0 ? "+" : "−"}${mag}`, tone: (d > 0) === (impact.better > 0) ? "good" : "bad" };
  }

  const num = (v: number | null, digits = 0) => (v === null ? "—" : v.toLocaleString("en-US", { maximumFractionDigits: digits }));

  function outside(e: MouseEvent) {
    const t = e.target as HTMLElement;
    if (el && t.isConnected && !el.contains(t) && !t.closest("[data-part]")) onclose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} onclick={outside} />

<div class="pick" bind:this={el} role="dialog" aria-label="Pick a part">
  <div class="ph">
    <span class="ttl">{title}<small>{sub}</small>{#if count > 1}<small class="many">to {count} ports</small>{/if}</span>
    <!-- svelte-ignore a11y_autofocus -->
    <input class="input" bind:value={q} placeholder="Find a part" aria-label="Find" autofocus />
    {#if anyOwned}
      <Segmented
        options={[
          { value: "", label: "All" },
          { value: "mine", label: "Owned" },
        ]}
        value={mine}
        onchange={(v) => (mine = v)}
      />
    {/if}
    {#each facets as f (f.id)}
      <Segmented
        options={[{ value: "", label: "All" }, ...f.values.map((v) => ({ value: v, label: v }))]}
        value={picked[f.id] ?? ""}
        onchange={(v) => (picked[f.id] = v)}
      />
    {/each}
    <span class="grow"></span>
    <button type="button" class="x" onclick={onclose} aria-label="Close"><X size={15} /></button>
  </div>

  {#if error}
    <p class="err">{error}</p>
  {:else if !list}
    <p class="faint">Reading what fits…</p>
  {:else}
    {@const grid = `grid-template-columns: minmax(180px, 260px)${showSize ? " 40px" : ""}${showGrade ? " 112px" : ""} repeat(${columns.length + extras.length}, 78px)${impact ? " 104px" : ""}`}
    <div class="body">
    <div class="row h" style={grid}>
      <span>Part</span>{#if showSize}<span>Size</span>{/if}{#if showGrade}<span>Class</span>{/if}
      {#each columns as col (col.key)}
        <button type="button" class="hc" class:on={order?.key === col.key} title={col.hint} onclick={() => sortBy(col.key, col.better)}
          >{col.label}{col.unit ? ` ${col.unit}` : ""}{order?.key === col.key ? (order.dir > 0 ? " ↓" : " ↑") : ""}</button
        >
      {/each}
      {#each extras as x (x.key)}
        <button type="button" class="hc" class:on={order?.key === x.key} title={x.hint} onclick={() => sortBy(x.key, x.better)}
          >{x.label}{order?.key === x.key ? (order.dir > 0 ? " ↓" : " ↑") : ""}</button
        >
      {/each}
      {#if impact}
        <button type="button" class="hc" class:on={order?.key === "_impact"} onclick={() => sortBy("_impact", impact.better)}
          >{impact.label}{order?.key === "_impact" ? (order.dir > 0 ? " ↓" : " ↑") : ""}</button
        >
      {/if}
    </div>
    {#each groups as g (g.label)}
      {#if g.label}<div class="grp"><span>{g.label}</span><small>{g.rows.length}</small></div>{/if}
      {#each g.rows as c (c.item)}
        {@const d = delta(c)}
        {@const imp = impact?.get(c) ?? null}
        {@const own = ownedOf(c)}
        <button type="button" class="row" class:cur={c.fitted} style={grid} onclick={() => !c.fitted && onpick(c.item)}>
          <span class="nm"
            ><b>{c.name}</b>{#if kind === "WeaponGun" && damageType(c.stats)}<small class="dt">{damageType(c.stats)}</small>{/if}{#if c.fitted}<small
                class="fit">fitted</small
              >{/if}{#if own}<small class="own" title={(c.owned ?? []).map((o) => `${o.count} at ${o.place}`).join("\n")}
                >{own} owned</small
              >{/if}</span
          >
          {#if showSize}<span class="v">S{c.size}</span>{/if}
          {#if showGrade}<span class="v cls">{classOf(c)}</span>{/if}
          {#each columns as col (col.key)}
            {@const v = value(c, col.key)}
            <span class="v" class:best={v !== null && col.better !== 0 && shown(v, col.digits) === best[col.key] && (list?.length ?? 0) > 1}
              >{v !== null && col.text ? col.text(v) : num(v, col.digits)}</span
            >
          {/each}
          {#each extras as x (x.key)}
            {@const xv = x.get(c)}
            <span class="v" title={x.title?.(c)}>{xv === null || xv === undefined ? "—" : x.fmt(xv)}</span>
          {/each}
          {#if impact}
            <span class="imp">{imp === null ? "—" : impact.fmt(imp)}<small class={d.tone}>{c.fitted ? "now" : d.text}</small></span>
          {/if}
        </button>
      {/each}
    {/each}
    {#if rows.length === 0}<p class="faint">Nothing matches.</p>{/if}
    </div>
  {/if}

  <div class="pf">
    <span class="faint">{list?.length ?? 0} parts fit</span>
    {#if extras.some((x) => x.key === "_buy" || x.key === "_market")}<span class="faint">Prices: UEX player reports</span>{/if}
    <span class="faint">Click a column to sort</span>
    <span class="grow"></span>
    <button type="button" class="btn" onclick={() => onpick(null)}>Leave empty</button>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
  </div>
</div>

<style>
  /* A panel at its table's own width, its list scrolling inside it: it
   * never pushes the page, and it eases in instead of jumping. */
  .pick {
    display: flex;
    flex-direction: column;
    width: fit-content;
    max-width: 100%;
    min-width: 560px;
    max-height: min(720px, calc(100vh - 120px));
    padding: 12px 16px 12px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--accent-border);
    box-shadow: var(--shadow-panel);
    animation: ease-in 140ms var(--easing);
  }
  @keyframes ease-in {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin-inline: -8px;
    padding-inline: 8px;
  }
  .grp {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 10px 0 2px;
    padding-bottom: 3px;
    border-bottom: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    font-family: var(--font-display);
    font-size: 0.72rem;
    font-weight: var(--weight-bold);
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: var(--text-dim);
  }
  .grp small {
    font-family: var(--font-mono);
    font-weight: var(--weight-normal);
    letter-spacing: 0;
    color: var(--text-faint);
  }
  .body .row.h {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-raised);
  }
  .ph {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }
  .ttl {
    font-weight: var(--weight-semibold);
  }
  .ttl small {
    margin-left: 8px;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
  }
  .ttl small.many {
    padding: 0 6px;
    border-radius: var(--radius-chip);
    border: 1px solid var(--accent-border);
    color: var(--accent);
  }
  .ph .input {
    flex: 0 1 220px;
    font-size: var(--text-small);
    padding-block: 4px;
  }
  .grow {
    flex: 1;
  }
  .x {
    display: inline-flex;
    padding: 2px;
    border: 0;
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }
  .row {
    display: grid;
    gap: 0 10px;
    align-items: center;
    width: calc(100% + 16px);
    padding: 5px 8px;
    margin-inline: -8px;
    border: 0;
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: 0.84rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .row.cur {
    background: var(--accent-fill-weak);
    box-shadow: inset 0 0 0 1px var(--accent-border);
    cursor: default;
  }
  .row.h {
    cursor: default;
    padding-block: 0 6px;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    font-size: 0.64rem;
    letter-spacing: 0.03em;
    text-transform: var(--label-transform);
    color: var(--text-faint);
  }
  .row.h:hover {
    background: none;
  }
  /* A long head ("Back up after hit s") wraps inside its column instead
   * of running into the next; the heads sit on the bottom line. */
  .row.h {
    align-items: end;
  }
  .hc {
    all: unset;
    cursor: pointer;
    text-align: right;
    line-height: 1.25;
    overflow-wrap: break-word;
  }
  .hc.on {
    color: var(--accent);
  }
  .nm {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
  }
  .nm b {
    font-weight: var(--weight-semibold);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fit {
    font: 600 0.6rem var(--font-mono);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
  }
  /* Free copies in your storage (My Items): one you can fit right away. */
  .own {
    font: 600 0.6rem var(--font-mono);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--good);
  }
  /* A gun's damage type, beside its name. */
  .dt {
    font-size: 0.72rem;
    color: var(--text-dim);
  }
  .v,
  .imp {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    text-align: right;
    white-space: nowrap;
  }
  .v.best {
    color: var(--good);
  }  .v.cls {
    font-family: var(--font-sans);
    font-size: 0.74rem;
    text-align: left;
    color: var(--text);
  }
  .imp {
    color: var(--text-strong);
  }
  .imp small {
    margin-left: 6px;
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .imp small.good {
    color: var(--good);
  }
  .imp small.bad {
    color: var(--bad);
  }
  .faint {
    font-size: 0.78rem;
    color: var(--text-faint);
  }
  .err {
    color: var(--bad);
  }
  .pf {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }
</style>
