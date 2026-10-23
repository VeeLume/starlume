<script lang="ts">
  // A topic's figures in full, or only some of its blocks (the power
  // panel's context asks for one or two). Raw data, labelled: every figure
  // is the game's own, a modifier reads as the change it makes (−30 % =
  // takes 30 % less). `narrow` stacks the blocks in one column.
  import { Dialog, Segmented } from "@veelume/ui";
  import { Flame, Zap } from "lucide-svelte";
  import type { ShipPageView, TypedRangeView } from "$lib/bindings";
  import QtRoutes from "./QtRoutes.svelte";

  export type Topic = "weapons" | "shield" | "armor" | "hull" | "signatures" | "flight" | "quantum" | "storage" | "insurance";

  export type Block =
    | "dps"
    | "missiles"
    | "shield"
    | "resistance"
    | "armor"
    | "penetration"
    | "hull"
    | "doors"
    | "signatures"
    | "sources"
    | "speed"
    | "turn"
    | "boost"
    | "drive"
    | "routes"
    | "storage"
    | "insurance";

  /** A topic's blocks, in the order they show. */
  const BLOCKS: Record<Topic, Block[]> = {
    weapons: ["dps", "missiles"],
    shield: ["shield", "resistance"],
    armor: ["armor", "penetration"],
    hull: ["hull", "doors"],
    signatures: ["signatures", "sources"],
    flight: ["speed", "turn", "boost"],
    quantum: ["drive", "routes"],
    storage: ["storage"],
    insurance: ["insurance"],
  };

  let {
    topic = null,
    blocks = null,
    narrow = false,
    page,
  }: { topic?: Topic | null; blocks?: Block[] | null; narrow?: boolean; page: ShipPageView } = $props();

  const shown = $derived(new Set<Block>(blocks ?? (topic ? BLOCKS[topic] : [])));

  let structure = $state(false);
  /** The thrusters' capacitor level the boost figures are read at; the
   *  game's default until picked. */
  let boostLevel = $state<number | null>(null);
  const level = $derived(Math.min(boostLevel ?? page.summary.boostDefault, Math.max(page.summary.boost.length - 1, 0)));
  let structureTab = $state<"parts" | "thrusters" | "doors">("parts");

  const TYPES = [
    ["physical", "Physical"],
    ["energy", "Energy"],
    ["distortion", "Distortion"],
  ] as const;

  const LABELS: Record<string, string> = {
    PowerPlant: "Power plants",
    Cooler: "Coolers",
    Shield: "Shields",
    Radar: "Radar",
    QuantumDrive: "Quantum drive",
    LifeSupportGenerator: "Life support",
    WeaponGun: "Weapons",
    FlightController: "Thrusters",
  };

  const fmt = (n: number | null | undefined, d = 0) =>
    n === null || n === undefined ? "—" : n.toLocaleString("en-US", { maximumFractionDigits: d });
  const pct = (v: number) => `${Math.round(v * 100)} %`;
  /** A damage multiplier as the change it makes: 0.7 → −30 %. */
  const delta = (m: number) => {
    const d = Math.round((m - 1) * 100);
    return d === 0 ? "±0 %" : `${d > 0 ? "+" : "−"}${Math.abs(d)} %`;
  };
  /** Taking less damage is good for the ship. */
  const tone = (m: number) => (m < 0.995 ? "good" : m > 1.005 ? "bad" : "flat");
  /** Where the generators' power puts a min–max range now. */
  const at = (r: [number, number], ratio: number) => r[0] + (r[1] - r[0]) * ratio;
  const ranged = (v: TypedRangeView) => TYPES.filter(([k]) => v[k][0] !== 0 || v[k][1] !== 0);

  const s = $derived(page.summary);
  const power = $derived(page.power);
  const hull = $derived(s.hull);
  const vital = $derived(hull.parts.filter((p) => p.vital));
  const breakable = $derived(hull.parts.filter((p) => p.breakable).length);
  /** Breachable doors, the ones a boarding party meets first (most HP). */
  const breachable = $derived(hull.doors.filter((d) => d.breachable).slice(0, 6));
  /** Thrusters by kind, in the order the game lists them. */
  const thrusterKinds = $derived.by(() => {
    const out: { kind: string; parts: typeof hull.thrusters }[] = [];
    for (const t of hull.thrusters) {
      const g = out.find((x) => x.kind === t.kind);
      if (g) g.parts.push(t);
      else out.push({ kind: t.kind, parts: [t] });
    }
    return out;
  });
  /** Resistance at every shield pip level, inferred (see ShieldView). */
  const pipLevels = $derived.by(() => {
    const p = s.shield?.pips;
    if (!p || p[1] <= 0) return [];
    return Array.from({ length: p[1] + 1 }, (_, k) => k);
  });
  const cs = $derived(s.crossSection);
  const csMax = $derived(cs ? Math.max(...cs) : 0);
  const csMin = $derived(cs ? Math.min(...cs) : 0);
  const sources = $derived(power?.sources ?? []);
  const sourceMax = $derived(Math.max(1, ...sources.map((x) => Math.max(x.em, x.ir))));

  /** The guns under a turret (crewed or remote) and the rest, the pilot's. */
  const groups = $derived.by(() => {
    const turrets = page.slots.filter((x) => x.system === "turrets").map((x) => x.path.join("/") + "/");
    const sum = (crew: boolean) => {
      const gs = page.guns.filter((g) => turrets.some((t) => g.path.startsWith(t)) === crew);
      return {
        count: gs.length,
        sustained: gs.reduce((a, g) => a + (g.sustained ?? 0), 0),
        burst: gs.reduce((a, g) => a + (g.burst ?? 0), 0),
        power: gs.length ? gs.reduce((a, g) => a + g.power, 0) / gs.length : 0,
      };
    };
    return [
      { label: "Pilot", ...sum(false) },
      { label: "Turrets", ...sum(true) },
    ].filter((g) => g.count > 0);
  });
</script>

<div class="pane" class:narrow>
  {#if shown.has("dps")}
    <div class="block wide">
      <h4>Damage per second</h4>
      <table>
        <thead><tr><th></th><th>guns</th><th title="Over a long fight: the capacitor's regen or the heat cycle">sustained</th><th title="While firing, before heat or the capacitor stop it">burst</th><th title="The power the weapons pool gives the guns now">power</th></tr></thead>
        <tbody>
          {#each groups as g (g.label)}
            <tr><td>{g.label}</td><td class="num">{g.count}</td><td class="num">{fmt(g.sustained)}</td><td class="num">{fmt(g.burst)}</td><td class="num dim">{pct(g.power)}</td></tr>
          {:else}
            <tr><td colspan="5" class="dim">No guns.</td></tr>
          {/each}
        </tbody>
      </table>
      <p class="foot">At the pips the weapons get now; more weapon pips raise sustained damage.</p>
    </div>
  {/if}
  {#if shown.has("missiles")}
    {#if s.missiles}
      {@const m = s.missiles}
      <div class="block">
        <h4>Missiles</h4>
        <dl>
          <dt>Fitted</dt>
          <dd>{m.count}</dd>
          {#if m.maxArmed !== null}
            <dt title="Missiles armed at once (the missile controller's limit)">Armed at once</dt>
            <dd>{m.maxArmed}</dd>
          {/if}
          {#if m.cooldown !== null}
            <dt title="Between launches">Launch cooldown</dt>
            <dd>{fmt(m.cooldown, 1)}<small>s</small></dd>
          {/if}
          {#if m.controllers > 1}
            <dt>Controllers</dt>
            <dd>{m.controllers}<small>one per seat</small></dd>
          {/if}
        </dl>
      </div>
    {:else}
      <p class="foot">No missiles fitted.</p>
    {/if}
  {/if}
  {#if shown.has("shield")}
    {#if s.shield}
      {@const sh = s.shield}
      <div class="block">
        <h4>{sh.face ?? "Shield"}{sh.generators > 1 ? `, ${sh.generators} generators` : ""}</h4>
        <dl>
          <dt>HP</dt>
          <dd>{fmt(s.shieldHealth)}</dd>
          <dt>Regen</dt>
          <dd>{fmt(power?.shieldRegen ?? s.shieldRegenMax)}<small>HP/s now</small> {fmt(s.shieldRegenMax)}<small>full</small></dd>
          <dt>Regen delay</dt>
          <dd>{fmt(sh.damagedDelay, 1)}<small>s after a hit</small> {fmt(sh.downedDelay, 1)}<small>s face down</small></dd>
          {#if sh.maxReallocation !== null && sh.face !== "Bubble"}
            <dt>Reallocation</dt>
            <dd>{pct(sh.maxReallocation)}<small>of HP between faces</small></dd>
          {/if}
        </dl>
      </div>
    {:else}
      <p class="foot">No shield generator fitted.</p>
    {/if}
  {/if}
  {#if shown.has("resistance") && s.shield}
    {@const sh = s.shield}
      {#if sh.resistance}
        {@const r = sh.resistance}
        <div class="block">
          <h4>Resistance</h4>
          <table>
            <thead><tr><th></th><th title="At the generators' power now">now</th><th>range</th></tr></thead>
            <tbody>
              {#each ranged(r) as [k, label] (k)}
                <tr><td>{label}</td><td class="num">{pct(at(r[k], sh.ratio))}</td><td class="num dim">{pct(r[k][0])} to {pct(r[k][1])}</td></tr>
              {/each}
            </tbody>
          </table>
          {#if pipLevels.length}
            <table class="pips">
              <thead>
                <tr><th>pips</th>{#each pipLevels as k (k)}<th class:cur={sh.pips && Math.round(sh.pips[0]) === k}>{k}</th>{/each}</tr>
              </thead>
              <tbody>
                {#each ranged(r) as [key, label] (key)}
                  <tr>
                    <td>{label}</td>
                    {#each pipLevels as k (k)}
                      <td class="num" class:cur={sh.pips && Math.round(sh.pips[0]) === k}>{Math.round(at(r[key], k / pipLevels[pipLevels.length - 1]) * 100)}</td>
                    {/each}
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
          <p class="foot">
            In %, by the shields' pips (min + (max − min) × pips ÷ most pips: read from the client, the pip ratio inferred). Zero for a
            moment after the faces are reassigned.
          </p>
        </div>
      {/if}
  {/if}
  {#if shown.has("armor")}
    {#if s.armor}
      {@const a = s.armor}
      <div class="block wide">
        <h4>{fmt(a.health)} HP armour</h4>
        <table>
          <thead>
            <tr>
              <th></th>
              {#if a.deflection}<th title="A hit at or below this does nothing (scaled by the armour's health)">deflects</th>{/if}
              {#if a.hullDamage}<th title="Damage the hull takes per point that gets through">hull takes</th>{/if}
              {#if a.durability}<th title="Damage the armour itself takes per point">armour takes</th>{/if}
            </tr>
          </thead>
          <tbody>
            {#each TYPES as [k, label] (k)}
              <tr>
                <td>{label}</td>
                {#if a.deflection}<td class="num">{a.deflection[k] ? fmt(a.deflection[k]) : "—"}</td>{/if}
                {#if a.hullDamage}<td class="num {tone(a.hullDamage[k])}">{delta(a.hullDamage[k])}</td>{/if}
                {#if a.durability}<td class="num {tone(a.durability[k])}">{delta(a.durability[k])}</td>{/if}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <p class="foot">No hull armour on record.</p>
    {/if}
  {/if}
  {#if shown.has("penetration")}
    {@const a = s.armor}
      <div class="block">
        <h4>Penetration</h4>
        <dl>
          <dt title="A hit that goes through: damage to a relay's first fuse">Fuses take</dt>
          <dd class={tone(hull.fuseMultiplier)}>{delta(hull.fuseMultiplier)}</dd>
          <dt title="A hit that goes through: damage to every other component">Components take</dt>
          <dd class={tone(hull.componentMultiplier)}>{delta(hull.componentMultiplier)}</dd>
          {#each a ? [["IR", a.signals[0]], ["EM", a.signals[1]], ["Cross-section", a.signals[2]]] : [] as [n, m] (n)}
            {#if Math.abs((m as number) - 1) > 0.005}
              <dt>{n}</dt>
              <dd class={tone(m as number)}>{delta(m as number)}</dd>
            {/if}
          {/each}
        </dl>
      </div>
  {/if}
  {#if shown.has("hull")}
    <div class="block">
      <h4>{hull.parts.length} parts, {breakable} break off</h4>
      <dl>
        <dt>HP</dt>
        <dd>{fmt(s.hullHealth)}<small>all parts</small></dd>
        {#each vital as v (v.name)}
          <dt title="The ship is lost when this part is">Vital part</dt>
          <dd>{fmt(v.hp)}<small>{v.name}</small></dd>
        {/each}
        {#if hull.dimensions}
          {@const d = hull.dimensions}
          <dt>Size</dt>
          <dd>{fmt(d[1], 1)}<small>long</small> {fmt(d[0], 1)}<small>wide</small> {fmt(d[2], 1)}<small>high, m</small></dd>
        {/if}
        {#if s.flight}
          <dt>Mass</dt>
          <dd>{fmt(s.flight.mass / 1000, 1)}<small>t loaded</small>{#if hull.hullMass} {fmt(hull.hullMass / 1000, 1)}<small>t hull</small>{/if}</dd>
        {/if}
      </dl>
      <button type="button" class="btn subtle more" onclick={() => (structure = true)}>Structure</button>
    </div>
  {/if}
  {#if shown.has("doors") && breachable.length}
      <div class="block">
        <h4>Doors to breach</h4>
        <dl>
          {#each breachable as d (d.name + d.hp)}
            <dt>{d.name}{d.count > 1 ? ` ×${d.count}` : ""}</dt>
            <dd>{fmt(d.hp)}<small>HP</small></dd>
          {/each}
        </dl>
        <p class="foot">Breachable by the game's own door names; every door with HP is under Structure.</p>
      </div>
  {/if}
  {#if shown.has("signatures")}
    <div class="block">
      <h4>Signatures</h4>
      <div class="sig">
        <span><Flame size={14} /><b>{fmt(power?.ir)}</b><small>IR</small></span>
        <span><Zap size={14} /><b>{fmt(power?.em)}</b><small>EM</small></span>
      </div>
      {#if cs}
        <table>
          <thead><tr><th>Cross-section</th><th>seen from</th></tr></thead>
          <tbody>
            {#each [["Front", cs[1]], ["Side", cs[0]], ["Top", cs[2]]] as [side, v] (side)}
              {@const big = v === csMax && csMax !== csMin}
              {@const small = v === csMin && csMax !== csMin}
              <tr><td>{side}</td><td class="num" class:bad={big} class:good={small}>{fmt(v as number)}{big ? " largest" : small ? " smallest" : ""}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
  {#if shown.has("sources") && sources.length}
      <div class="block">
        <h4>Where they come from</h4>
        <div class="src" role="table" aria-label="Where the signatures come from">
          <span class="sh" role="row"><span role="columnheader"></span><span role="columnheader"><Zap size={11} /> EM</span><span role="columnheader"><Flame size={11} /> IR</span></span>
          {#each sources as x (x.itemType)}
            <span class="sr" role="row">
              <span role="cell">{LABELS[x.itemType] ?? x.itemType}</span>
              <span role="cell" class="bar em"><i style:width="{(x.em / sourceMax) * 100}%"></i><b>{x.em ? fmt(x.em) : "—"}</b></span>
              <span role="cell" class="bar ir"><i style:width="{(x.ir / sourceMax) * 100}%"></i><b>{x.ir ? fmt(x.ir) : "—"}</b></span>
            </span>
          {/each}
        </div>
        <p class="foot">IR is the heat the coolers shed, shown against what heats them.</p>
      </div>
  {/if}
  {#if shown.has("speed") && s.flight}
    {@const f = s.flight}
    <div class="block">
      <h4>Speed</h4>
      <dl>
        <dt>SCM</dt>
        <dd>{fmt(f.scmSpeed)}<small>m/s</small></dd>
        <dt>NAV</dt>
        <dd>{fmt(f.navSpeed)}<small>m/s</small></dd>
        <dt>Boost</dt>
        <dd>{fmt(f.boostForward)}<small>forward</small> {fmt(f.boostBackward)}<small>back, m/s</small></dd>
        <dt>Hydrogen</dt>
        <dd>{fmt(f.hydrogenCapacity, 1)}<small>SCU</small></dd>
      </dl>
    </div>
  {/if}
  {#if shown.has("turn") && s.flight}
    {@const f = s.flight}
    {@const b = s.boost[level]}
    <div class="block">
      <h4>Turn rate</h4>
      <table>
        <thead><tr><th></th><th>normal</th><th title="At the thrusters' capacitor level {level} of 6">boosted at {level}</th><th>boost max</th></tr></thead>
        <tbody>
          {#each ["Pitch", "Yaw", "Roll"] as axis, i (axis)}
            <tr>
              <td>{axis}</td>
              <td class="num">{fmt(f.rotation[i])} °/s</td>
              <td class="num">{b ? `${fmt(f.rotation[i] * b.turn[i])} °/s` : "—"}</td>
              <td class="num dim">{fmt(f.rotationBoosted[i])} °/s</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
  {#if shown.has("boost") && s.boost.length}
      <div class="block wide">
        <h4>Boost by capacitor level</h4>
        <table class="pips">
          <thead>
            <tr>
              <th>level</th>
              {#each s.boost as _, k (k)}
                <th class:cur={k === level}><button type="button" class="lv" onclick={() => (boostLevel = k)}>{k}</button></th>
              {/each}
            </tr>
          </thead>
          <tbody>
            <tr><td title="Empty to full, with the delay after use">Regen</td>{#each s.boost as x, k (k)}<td class="num" class:cur={k === level}>{fmt(x.regenS, 1)} s</td>{/each}</tr>
            <tr><td title="Full to empty, at rest">Deplete</td>{#each s.boost as x, k (k)}<td class="num" class:cur={k === level}>{x.depleteS === null ? "—" : `${fmt(x.depleteS, 1)} s`}</td>{/each}</tr>
            <tr><td>Pitch, yaw</td>{#each s.boost as x, k (k)}<td class="num" class:cur={k === level}>×{x.turn[0].toFixed(2)}</td>{/each}</tr>
            <tr><td>Roll</td>{#each s.boost as x, k (k)}<td class="num" class:cur={k === level}>×{x.turn[2].toFixed(2)}</td>{/each}</tr>
          </tbody>
        </table>
        <p class="foot">The thrusters' capacitor pips, 0 to 6; the game starts at {s.boostDefault}. Turn multipliers at full boost ramp.</p>
      </div>
  {/if}
  {#if shown.has("drive") && s.quantum}
    {@const q = s.quantum}
    <div class="block">
      <h4>{q.drive}</h4>
      <dl>
        <dt>Speed</dt>
        <dd>{fmt(q.speed / 1000)}<small>km/s</small></dd>
        <dt>Spool</dt>
        <dd>{fmt(q.spoolUp, 1)}<small>s</small></dd>
        <dt>Fuel</dt>
        <dd>{fmt(q.fuelPerGm, 3)}<small>SCU per Gm</small> {fmt(q.fuelCapacity, 1)}<small>SCU tank</small></dd>
        {#if q.rangeGm}
          <dt>Range</dt>
          <dd>{fmt(q.rangeGm, 1)}<small>Gm on a full tank</small></dd>
        {/if}
      </dl>
    </div>
  {/if}
  {#if shown.has("routes") && s.quantum}
    <div class="block wide">
      <QtRoutes drive={s.quantum} />
    </div>
  {/if}
  {#if shown.has("storage")}
    {@const st = s.storage}
    <div class="block">
      <h4>Storage</h4>
      {#if st}
        <dl>
          <dt>Cargo</dt>
          <dd>{fmt(st.cargo)}<small>SCU</small></dd>
          {#if st.lockers}<dt>Lockers</dt><dd>{fmt(st.lockers, 1)}<small>SCU</small></dd>{/if}
          {#if st.inventory}<dt>Ship inventory</dt><dd>{fmt(st.inventory, 1)}<small>SCU</small></dd>{/if}
          {#if st.suitLockers}<dt>Suit lockers</dt><dd>{st.suitLockers}</dd>{/if}
          {#if st.weaponRacks}<dt>Weapon racks</dt><dd>{st.weaponRacks}<small>with</small> {st.rackSlots}<small>slots</small></dd>{/if}
        </dl>
      {:else}
        <p class="foot">No storage on record.</p>
      {/if}
    </div>
  {/if}
  {#if shown.has("insurance")}
    <div class="block">
      <h4>Insurance claim</h4>
      {#if s.insurance}
        {@const ins = s.insurance}
        <dl>
          <dt>Claim</dt>
          <dd>{fmt(ins.waitMinutes, 1)}<small>min</small></dd>
          <dt>Expedited</dt>
          <dd>{fmt(ins.mandatoryMinutes, 1)}<small>min for</small> {fmt(ins.expediteFee)}<small>aUEC</small></dd>
        </dl>
        <p class="foot">Before modifiers.</p>
      {:else}
        <p class="foot">This ship cannot be insured.</p>
      {/if}
    </div>
  {/if}
</div>

<Dialog open={structure} onclose={() => (structure = false)} title="Structure" class="structure">
  <div class="tabs">
    <Segmented
      options={[
        { value: "parts", label: `Hull (${hull.parts.length})` },
        { value: "thrusters", label: `Thrusters (${hull.thrusters.length})` },
        { value: "doors", label: `Doors (${hull.doors.reduce((a, d) => a + d.count, 0)})` },
      ]}
      value={structureTab}
      onchange={(v) => (structureTab = v as typeof structureTab)}
    />
  </div>
  <div class="tree">
    {#if structureTab === "parts"}
      {#each hull.parts as p, i (i)}
        <span class="node" style:--depth={p.depth} class:vital={p.vital}>
          <span class="pn"
            >{p.name}{#if p.vital}<small class="vit">vital</small>{/if}{#if p.breakable}<small class="brk">breaks off</small>{/if}</span
          >
          <b>{p.hp ? fmt(p.hp) : "—"}</b>
        </span>
      {/each}
    {:else if structureTab === "thrusters"}
      {#each thrusterKinds as g (g.kind)}
        <span class="node grp"><span class="pn">{g.kind} ({g.parts.length})</span><b></b></span>
        {#each g.parts as t, i (i)}
          <span class="node" style:--depth={1}><span class="pn">{t.place}</span><b>{fmt(t.hp)}</b></span>
        {/each}
      {:else}
        <p class="foot">No thrusters on record.</p>
      {/each}
    {:else}
      {#each hull.doors as d (d.name + d.hp)}
        <span class="node"
          ><span class="pn">{d.name}{d.count > 1 ? ` ×${d.count}` : ""}{#if d.breachable}<small class="brk">breachable</small>{/if}</span><b
            >{fmt(d.hp)}</b
          ></span
        >
      {:else}
        <p class="foot">No doors with HP.</p>
      {/each}
    {/if}
  </div>
  <p class="foot pad">
    {structureTab === "parts"
      ? "The hull's HP is the parts' sum; the ship is lost with a vital part."
      : structureTab === "thrusters"
        ? "Thrusters carry their own HP, outside the hull's."
        : "Doors with HP, interior and fitted; breachable as the game's door names say."}
  </p>
</Dialog>

<style>
  .pane {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 14px 28px;
    align-content: start;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .block.wide {
    grid-column: span 2;
  }
  .pane.narrow {
    grid-template-columns: minmax(0, 1fr);
  }
  .narrow .block.wide {
    grid-column: auto;
  }
  h4 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 0.74rem;
    font-weight: var(--weight-bold);
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: color-mix(in srgb, var(--hue-sky) 60%, var(--text-strong));
  }
  dl {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 5px 14px;
    margin: 0;
    font-size: var(--text-small);
  }
  dt {
    color: var(--text-faint);
  }
  dd {
    margin: 0;
    text-align: right;
    font-family: var(--font-mono);
    color: var(--text-strong);
  }
  dd small {
    margin: 0 8px 0 4px;
    font-family: var(--font-sans);
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  dd small:last-child {
    margin-right: 0;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-small);
  }
  th {
    font-size: 0.68rem;
    font-weight: 600;
    text-align: right;
    color: var(--text-faint);
    padding: 0 0 4px;
    border-bottom: 1px solid var(--border);
  }
  th:first-child {
    text-align: left;
  }
  td {
    padding: 3px 0;
    color: var(--text);
  }
  td + td {
    padding-left: 10px;
  }
  .num {
    text-align: right;
    font-family: var(--font-mono);
    color: var(--text-strong);
  }
  .dim {
    color: var(--text-faint);
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
  .flat {
    color: var(--text-faint);
  }
  .foot {
    margin: 0;
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  .more {
    align-self: flex-start;
  }
  .sig {
    display: flex;
    gap: 18px;
  }
  .sig span {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-faint);
  }
  .sig b {
    font: 700 1.05rem var(--font-mono);
    color: var(--text-strong);
  }
  .sig small {
    font-size: 0.7rem;
  }
  .src {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) minmax(0, 1fr);
    gap: 4px 10px;
    font-size: var(--text-small);
  }
  .sh,
  .sr {
    display: contents;
  }
  .sh span {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 0.68rem;
    font-weight: 600;
    color: var(--text-faint);
  }
  .bar {
    position: relative;
    display: flex;
    align-items: center;
    min-height: 18px;
  }
  .bar i {
    position: absolute;
    inset: 2px auto 2px 0;
    border-radius: 3px;
  }
  .em i {
    background: color-mix(in srgb, var(--hue-sky) 35%, transparent);
  }
  .ir i {
    background: color-mix(in srgb, var(--hue-rose) 35%, transparent);
  }
  .bar b {
    position: relative;
    padding-left: 5px;
    font: 600 0.72rem var(--font-mono);
    color: var(--text-strong);
  }
  :global(.structure) {
    max-width: 560px !important;
  }
  .tree {
    display: flex;
    flex-direction: column;
    overflow: auto;
    padding: 8px 16px;
    font-size: var(--text-small);
  }
  .node {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 12px;
    padding: 3px 0 3px calc(var(--depth) * 18px);
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }
  .node b {
    font: 600 0.78rem var(--font-mono);
    color: var(--text-strong);
  }
  .node.vital .pn {
    font-weight: 700;
    color: var(--text-strong);
  }
  .tabs {
    padding: 10px 16px 0;
  }
  .node.grp .pn {
    font-weight: 700;
    color: var(--text-strong);
  }
  .vit {
    margin-left: 8px;
    font-size: 0.66rem;
    color: var(--bad);
  }
  .pips th,
  .pips td {
    text-align: center;
    padding-left: 4px;
  }
  .pips th:first-child,
  .pips td:first-child {
    text-align: left;
    padding-left: 0;
  }
  .lv {
    all: unset;
    cursor: pointer;
    padding: 0 4px;
  }
  .lv:hover {
    color: var(--text-strong);
  }
  .pips .cur {
    color: var(--hue-sky);
    font-weight: 700;
  }
  .brk {
    margin-left: 8px;
    font-size: 0.66rem;
    color: var(--warn);
  }
  .pad {
    padding: 8px 16px 14px;
  }
</style>
