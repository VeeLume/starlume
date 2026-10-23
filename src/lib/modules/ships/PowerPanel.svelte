<script lang="ts">
  // The power network as the ship's power screen shows it: free pips, the
  // signatures, the cooling bar, then one column per pool and per item
  // outside a pool, in the screen's order. A pip sets its column; the icon
  // switches the column's items off and on again, as in game; a shield
  // generator's minimum is the numbered block at the bottom of its column.
  // Every change is computed Rust-side (sc-vehicles' network), never here.
  import { Segmented } from "@veelume/ui";
  import { Atom, Box, ChevronsRight, Crosshair, Fan, Flame, HeartPulse, Rss, Shield, Zap } from "lucide-svelte";
  import type { ColumnView, ModeView, PowerView } from "$lib/bindings";

  let {
    power,
    aimAssist = null,
    crossSection = null,
    resist = [],
    onlevel,
    ontoggle,
    onmode,
    onreset,
  }: {
    power: PowerView;
    /** The radar's aim assist, m: [now, at full power]. */
    aimAssist?: [number, number] | null;
    /** The radar cross-section seen from the side, the front and above. */
    crossSection?: [number, number, number] | null;
    /** Shield resistance at the shields' power now, per damage type. */
    resist?: [string, number][];
    onlevel: (column: string, pips: number) => void;
    ontoggle: (paths: string[]) => void;
    onmode: (mode: ModeView) => void;
    onreset: () => void;
  } = $props();

  const ICONS: Record<string, typeof Zap> = {
    WeaponGun: Crosshair,
    FlightController: ChevronsRight,
    Shield,
    QuantumDrive: Atom,
    Radar: Rss,
    LifeSupportGenerator: HeartPulse,
    Cooler: Fan,
  };
  const LABELS: Record<string, string> = {
    WeaponGun: "Weapons",
    FlightController: "Thrusters",
    Shield: "Shields",
    QuantumDrive: "Quantum drive",
    Radar: "Radar",
    LifeSupportGenerator: "Life support",
    Cooler: "Cooler",
  };

  const free = $derived(Math.round((power.supply - power.used) * 10) / 10);
  const coolPct = $derived(power.cooling === null ? null : Math.round(power.cooling * 100));
  const k = (v: number) => (v >= 1000 ? `${(v / 1000).toFixed(1)}k` : Math.round(v).toLocaleString("en-US"));

  type Block = { kind: "min" | "pip"; size: number; lit: boolean; level: number };

  /** A column's blocks, bottom up: the minimum blocks (each a member's
   *  minimum, drawn as one), then one per remaining pip. */
  function blocks(c: ColumnView): Block[] {
    const out: Block[] = [];
    let at = 0;
    for (const m of c.minimums) {
      at += m;
      out.push({ kind: "min", size: m, lit: c.level >= at - 0.01, level: at });
    }
    for (let p = at; p < c.max; p++) out.push({ kind: "pip", size: 1, lit: c.level >= p + 0.99, level: p + 1 });
    return out;
  }

  function label(c: ColumnView, i: number): string {
    const base = LABELS[c.itemType] ?? c.itemType;
    if (c.itemType !== "Cooler") return base;
    const n = power.columns.slice(0, i + 1).filter((x) => x.itemType === "Cooler").length;
    return `${base} ${n}`;
  }

  function pct(c: ColumnView): string {
    if (c.excluded) return "—";
    if (c.off) return "off";
    return c.max > 0 ? `${Math.round((c.level / c.max) * 100)}%` : "";
  }
</script>

<section class="mfd" aria-label="Power management">
  <div class="mfd-h">
    <h2 class="label">Power</h2>
    <span class="grow"></span>
    <b class:over={free < 0}>{free}</b><span class="of">/ {power.supply} pips free</span>
  </div>
  <div class="sig">
    <span title="IR signature"><Flame size={16} /><b>{k(power.ir)}</b><small>IR</small></span
    >
    <span title="EM signature"><Zap size={16} /><b>{k(power.em)}</b><small>EM</small></span
    >
    <span
      title={crossSection ? `Radar cross-section, the largest side: ${crossSection.map((v, i) => `${["side", "front", "top"][i]} ${k(v)}`).join(", ")}` : "Radar cross-section"}
      ><Box size={16} /><b>{crossSection ? k(Math.max(...crossSection)) : "—"}</b><small>CS</small></span
    >
    {#if crossSection}
      <!-- Which side the radar sees how big, under the largest. -->
      <p class="split">
        {#each [["nose", crossSection[1]], ["flank", crossSection[0]], ["top", crossSection[2]]] as const as [side, v] (side)}<span
            ><small>{side}</small>{k(v)}</span
          >{/each}
      </p>
    {/if}
  </div>
  <div class="bars">
    <div class="cool" title="Coolant asked for over coolant made">
      <Fan size={16} /><span class="cap">Cooling</span>
      <span class="tr"><i class:hot={coolPct === null || coolPct > 85} style:width="{Math.min(coolPct ?? 100, 100)}%"></i></span>
      <b>{coolPct === null ? "—" : `${coolPct} %`}</b>
    </div>
    {#if aimAssist}
      <div class="cool" title="How far the radar's aim assist reaches at its power (full power: {Math.round(aimAssist[1]).toLocaleString('en-US')} m)">
        <Rss size={16} /><span class="cap">Aim assist</span>
        <span class="tr"><i style:width="{Math.min((aimAssist[0] / aimAssist[1]) * 100, 100)}%"></i></span>
        <b>{Math.round(aimAssist[0]).toLocaleString("en-US")} m</b>
      </div>
    {/if}
    {#if resist.length}
      <!-- Follows the shields' pips, so it sits where they are set. -->
      <div class="cool" title="Shield resistance at the shields' power now">
        <Shield size={16} /><span class="cap">Resists</span>
        <span class="rs">{#each resist as [type, v] (type)}<span><b>{Math.round(v * 100)} %</b><small>{type}</small></span>{/each}</span>
      </div>
    {/if}
  </div>
  <div class="cols">
    {#each power.columns as c, i (c.id)}
      {@const name = label(c, i)}
      <div class="col">
        <span class="n">{c.level > 0 && !c.excluded ? Math.round(c.level * 10) / 10 : ""}</span>
        <div class="stack">
          {#each blocks(c) as b (b.level)}
            <button
              type="button"
              class="blk"
              class:min={b.kind === "min"}
              class:on={b.lit && !c.excluded && !c.off}
              class:dead={c.excluded || c.off}
              style:height="{b.size * 11 + (b.size - 1) * 3}px"
              disabled={c.excluded || c.off}
              aria-label="{name}: {b.level} pips"
              onclick={() => onlevel(c.id, b.lit && Math.abs(c.level - b.level) < 0.01 ? b.level - b.size : b.level)}
              >{b.kind === "min" ? b.size : ""}</button
            >
          {/each}
        </div>
        <button
          type="button"
          class="ico"
          class:off={c.off}
          title="{c.off ? 'Switch on' : 'Switch off'} {name.toLowerCase()}"
          aria-label="{c.off ? 'Switch on' : 'Switch off'} {name}"
          onclick={() => ontoggle(c.items)}
        >
          {#if ICONS[c.itemType]}{@const Icon = ICONS[c.itemType]}<Icon size={18} />{:else}<Zap size={18} />{/if}
        </button>
        <small>{pct(c)}</small>
      </div>
    {/each}
  </div>
  <div class="mfd-f">
    <Segmented
      options={[
        { value: "scm", label: "SCM" },
        { value: "nav", label: "NAV" },
      ]}
      value={power.mode}
      onchange={(v) => onmode(v as ModeView)}
    />
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onreset}>Power-on</button>
  </div>
</section>

<style>
  .mfd {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-raised) 60%, transparent);
  }
  .mfd-h {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .mfd-h .label {
    margin: 0;
  }
  .mfd-h b {
    font: 700 1.15rem var(--font-mono);
    color: var(--accent);
  }
  .mfd-h b.over {
    color: var(--bad);
  }
  .of {
    font: 0.8rem var(--font-mono);
    color: var(--text-faint);
  }
  .grow {
    flex: 1;
  }
  .sig {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
    color: var(--accent);
  }
  .sig span {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sig b {
    font: 700 1.1rem var(--font-mono);
    color: var(--text-strong);
  }
  .sig small,
  .cap {
    font: 0.66rem var(--font-mono);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  /* The whole row, ending under CS: a narrow panel has no room for it in
   * the CS cell alone. */
  .split {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin: -4px 0 0;
    font: 0.74rem var(--font-mono);
    color: var(--text-dim);
  }
  .sig .split small {
    margin-right: 4px;
    font: 0.7rem var(--font-sans);
    letter-spacing: 0;
    text-transform: none;
    color: var(--text-faint);
  }
  .rs {
    grid-column: 3 / 5;
    display: flex;
    gap: 16px;
  }
  .cool .rs b {
    font: 700 0.95rem var(--font-mono);
    color: var(--text-strong);
  }
  .rs small {
    margin-left: 5px;
    font-size: 0.74rem;
    color: var(--text-dim);
  }
  .bars {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .cool {
    display: grid;
    grid-template-columns: 16px 72px minmax(0, 1fr) 64px;
    gap: 8px;
    align-items: center;
    color: var(--accent);
  }
  .tr {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-sunk);
    overflow: hidden;
  }
  .tr i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
  }
  .tr i.hot {
    background: var(--warn);
  }
  .cool b {
    font: 700 0.95rem var(--font-mono);
    color: var(--text-strong);
    text-align: right;
  }
  .cols {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 8px;
    align-items: end;
    padding-top: 6px;
  }
  .col {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .n {
    min-height: 16px;
    font: 700 0.85rem var(--font-mono);
    color: var(--text-strong);
  }
  .stack {
    display: flex;
    flex-direction: column-reverse;
    gap: 3px;
    width: 100%;
    max-width: 38px;
  }
  .blk {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    background: var(--bg-sunk);
    border: 1px solid color-mix(in srgb, var(--text) 10%, transparent);
    font: 700 0.68rem var(--font-mono);
    color: var(--text-dim);
    cursor: pointer;
  }
  .blk:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .blk.on {
    background: color-mix(in srgb, var(--accent) 70%, transparent);
    border-color: transparent;
    color: var(--bg);
  }
  .blk.min {
    background: color-mix(in srgb, var(--accent) 30%, var(--bg-sunk));
    color: var(--text-strong);
  }
  .blk.min.on {
    background: color-mix(in srgb, var(--accent) 55%, var(--bg-sunk));
  }
  .blk.dead {
    opacity: 0.35;
    cursor: default;
  }
  .ico {
    all: unset;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 30px;
    border-radius: var(--radius);
    color: var(--accent);
    cursor: pointer;
  }
  .ico:hover {
    background: var(--accent-fill-faint);
  }
  .ico.off {
    color: var(--text-faint);
    opacity: 0.5;
  }
  .col small {
    font: 0.66rem var(--font-mono);
    color: var(--text-faint);
  }
  .mfd-f {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }
</style>
