<script lang="ts" module>
  import {
    Crosshair,
    Fan,
    Fuel,
    HeartPulse,
    Radar,
    Rocket,
    Shield,
    ShieldAlert,
    Sparkles,
    Target,
    Zap,
  } from "lucide-svelte";
  import type { SystemView } from "$lib/bindings";

  export type Sys = { id: SystemView; label: string; hue: string; icon: typeof Zap };
  export const SYSTEMS: Sys[] = [
    { id: "weapons", label: "Weapons", hue: "var(--hue-apricot)", icon: Crosshair },
    { id: "turrets", label: "Turrets", hue: "var(--hue-rose)", icon: Target },
    { id: "missiles", label: "Missiles", hue: "var(--hue-sand)", icon: Rocket },
    { id: "countermeasures", label: "Countermeasures", hue: "var(--hue-sand)", icon: Sparkles },
    { id: "shields", label: "Shields", hue: "var(--hue-sky)", icon: Shield },
    { id: "powerPlants", label: "Power plants", hue: "var(--accent)", icon: Zap },
    { id: "coolers", label: "Coolers", hue: "var(--hue-teal)", icon: Fan },
    { id: "quantum", label: "Quantum", hue: "var(--accent)", icon: Sparkles },
    { id: "radar", label: "Radar", hue: "var(--hue-sky)", icon: Radar },
    { id: "lifeSupport", label: "Life support", hue: "var(--hue-teal)", icon: HeartPulse },
    { id: "fuel", label: "Fuel", hue: "var(--hue-sand)", icon: Fuel },
    { id: "docked", label: "Docked", hue: "var(--hue-sky)", icon: ShieldAlert },
  ];
  /** A column's entry: one system's panel, or one panel over several
   *  (the pilot's guns and the turrets share the weapons pool; only who
   *  aims them differs), led by the first. */
  export type Entry = SystemView | SystemView[];
  /** A group's name inside a shared panel: who controls it. */
  const GROUP: Partial<Record<SystemView, string>> = { weapons: "Pilot", turrets: "Turrets" };
  /** One figure of a system's header line: value and its unit. */
  export type Fig = { v: string; u: string; warn?: boolean; change?: Change | null };
  /** A figure's change against stock: the signed text and whether it is
   *  for the better. */
  export type Change = { text: string; good: boolean };
  /** `now` against `before` as a change, none when equal or unknown. */
  export function change(now: number | null, before: number | null | undefined, better = 1, digits = 0): Change | null {
    if (now === null || before === null || before === undefined) return null;
    const d = now - before;
    if (Math.abs(d) < 0.5 * 10 ** -digits) return null;
    const mag = Math.abs(d).toLocaleString("en-US", { maximumFractionDigits: digits });
    return { text: `${d > 0 ? "+" : "−"}${mag}`, good: d * better > 0 };
  }
  /** Offense; defence and power; travel and utility. */
  export const GRID_COLUMNS: SystemView[][] = [
    ["weapons", "turrets", "missiles", "countermeasures"],
    ["shields", "powerPlants", "coolers"],
    ["quantum", "radar", "lifeSupport", "fuel", "docked"],
  ];
</script>

<script lang="ts">
  // The loadout as system panels in fixed columns (by default three:
  // offense; defence and power; travel and utility), so opening one panel
  // never moves the others. A panel leads with its system's figure (the guns' DPS, the
  // shields' HP, the coolers' coolant); its rows are the parts, one line
  // each: size, name, grade, the part's lead figure, power, and its key
  // figures under the name (./figures, shared with the picker). Every part
  // is its own line; mounts fold: a turret reads "4× CF-337" until it opens
  // on its guns, a rack "16× StrikeForce II", and the pilot's guns of one
  // kind "4× CF-557".
  // A locked part's size badge is dimmed; a part on the power network has
  // a power button (the power panel's icon does the same).
  import { ChevronDown, ChevronRight, Power } from "lucide-svelte";
  import type { ColumnView, GunView, SlotView } from "$lib/bindings";
  import { damageType, lineFigures } from "./figures";

  let {
    slots,
    off,
    powered,
    guns,
    selected = null,
    onpick,
    ontoggle,
    onhover,
    weaponPool = null,
    columns = GRID_COLUMNS,
    lines = null,
    stock = null,
  }: {
    slots: SlotView[];
    /** Port paths (`/`-joined) switched off. */
    off: string[];
    /** Port paths on the power network. */
    powered: Set<string>;
    /** Each gun's damage under the current power, by port path. */
    guns: Map<string, GunView>;
    selected?: string | null;
    /** Open the picker on these ports (a folded line's every part). */
    onpick: (slots: SlotView[]) => void;
    ontoggle: (paths: string[]) => void;
    onhover: (system: SystemView | null) => void;
    /** The power panel's weapons pool: what it gives the guns. */
    weaponPool?: ColumnView | null;
    /** The panels each column shows, top to bottom. */
    columns?: Entry[][];
    /** The ship-level figures a system's header line adds (missiles,
     *  shields, quantum). Given, every panel carries its own figures in a
     *  line under its header, the guns' power share among them; absent,
     *  the guns' power is the bar it was. */
    lines?: Partial<Record<SystemView, Fig[]>> | null;
    /** The same ship as it comes from the factory (same power): headers
     *  show their change against it. Null: nothing changed. */
    stock?: { slots: SlotView[]; guns: Map<string, GunView> } | null;
  } = $props();

  const sysOf = (id: SystemView) => SYSTEMS.find((s) => s.id === id)!;
  const COLUMNS = $derived(
    columns.map((col) =>
      col.map((e) => {
        const members = (Array.isArray(e) ? e : [e]).map(sysOf);
        return { sys: members[0], members };
      }),
    ),
  );

  const key = (s: SlotView) => s.path.join("/");
  const num = (v: number, d = 0) => v.toLocaleString("en-US", { maximumFractionDigits: d });
  const stat = (s: SlotView, k: string) => s.stats.find((x) => x.key === k)?.value ?? null;

  /** A mount holds the parts a row lists (gimbal, turret, rack, dock). */
  const mount = (s: SlotView) =>
    s.itemType === "Turret" || s.itemType === "TurretBase" || s.itemType === "MissileLauncher" || s.itemType === "DockingCollar";

  /** The parts below a mount, depth first, mounts skipped. */
  function leaves(s: SlotView): SlotView[] {
    return s.children.flatMap((c) => (mount(c) ? leaves(c) : c.system === "other" ? [] : [c]));
  }

  const isOff = (s: SlotView) => off.some((o) => key(s) === o || key(s).startsWith(o + "/"));
  const gunDps = (ss: SlotView[], g = guns) => ss.reduce((n, s) => n + (g.get(key(s))?.sustained ?? 0), 0);
  const gunBurst = (ss: SlotView[], g = guns) => ss.reduce((n, s) => n + (g.get(key(s))?.burst ?? 0), 0);

  /** A panel's header figure as a number, for its change against stock
   *  (the text is `summary`'s). */
  function headNum(sys: SystemView, cards: SlotView[], g: Map<string, GunView>): number | null {
    const parts = cards.flatMap((c) => (mount(c) ? leaves(c) : [c]));
    const sum = (k: string, ss = parts) => ss.reduce((n, s) => n + (stat(s, k) ?? 0), 0);
    switch (sys) {
      case "weapons":
      case "turrets":
        return gunDps(parts, g);
      case "missiles":
        return parts.filter((p) => p.itemType === "Missile").length;
      case "shields":
        return sum("hp");
      case "powerPlants":
        return sum("output");
      case "coolers":
        return sum("coolant");
      case "fuel":
        return sum("capacity", parts.filter((p) => p.itemType === "FuelTank"));
      default:
        return null;
    }
  }
  /** The stock ship's cards of these systems. */
  const stockCards = (ids: SystemView[]) => (stock ? stock.slots.filter((c) => ids.includes(c.system)) : []);
  function headChange(sys: SystemView, ids: SystemView[], cards: SlotView[]): Change | null {
    if (!stock) return null;
    const digits = sys === "weapons" || sys === "turrets" || sys === "coolers" ? 1 : 0;
    return change(headNum(sys, cards, guns), headNum(sys, stockCards(ids), stock.guns), 1, digits);
  }

  /** The stat a part's lead figure is (a gun leads with its DPS, no stat). */
  const LEAD: Record<string, string> = {
    PowerPlant: "output",
    Cooler: "coolant",
    Shield: "hp",
    QuantumDrive: "qspeed",
    Radar: "aim",
    WeaponDefensive: "ammo",
    FuelTank: "capacity",
    QuantumFuelTank: "capacity",
  };
  /** A part's line. A gun's fire time is the one under its power now (an
   *  energy gun loads fewer shots on fewer pips), not the item's own. */
  const facts = (s: SlotView) => {
    const fs = lineFigures(s.itemType ?? "", s.stats, LEAD[s.itemType ?? ""] ?? null);
    const g = s.itemType === "WeaponGun" ? guns.get(key(s)) : undefined;
    if (!g || g.fires === null) return fs;
    return fs.map((f) => (f.label === "fires" ? { ...f, text: `${num(g.fires!, 1)} s` } : f));
  };

  /** The figure a part leads with, by what it is. */
  function figure(s: SlotView): string {
    switch (s.itemType) {
      case "WeaponGun":
        return guns.has(key(s)) ? `${num(gunDps([s]), 1)} dps` : "";
      case "PowerPlant":
        return stat(s, "output") !== null ? `${num(stat(s, "output")!)} pips` : "";
      case "Cooler":
        return stat(s, "coolant") !== null ? `${num(stat(s, "coolant")!, 1)} /s` : "";
      case "Shield":
        return stat(s, "hp") !== null ? `${num(stat(s, "hp")!)} HP` : "";
      case "QuantumDrive":
        return stat(s, "qspeed") !== null ? `${num(stat(s, "qspeed")!)} km/s` : "";
      case "Radar":
        return stat(s, "aim") !== null ? `${num(stat(s, "aim")!)} m` : "";
      case "WeaponDefensive":
        return stat(s, "ammo") !== null ? num(stat(s, "ammo")!) : "";
      case "FuelTank":
      case "QuantumFuelTank":
        return stat(s, "capacity") !== null ? `${num(stat(s, "capacity")!, 1)} SCU` : "";
      default:
        return "";
    }
  }


  /** Class and grade as the game says them ("Civilian C"). */
  const tier = (s: SlotView) =>
    s.itemType === "WeaponGun" || s.itemType === "Missile" ? "" : [s.class, s.grade >= 1 && s.grade <= 4 ? "ABCD"[s.grade - 1] : ""].filter(Boolean).join(" ");

  /** A countermeasure launcher's kind, from its name (`Joker Defcon - Noise
   *  Launcher` → `Noise`). */
  const cmKind = (s: SlotView) => (s.name ?? "").split(" - ").at(-1)?.replace(/ Launcher$/, "") ?? "";

  /** What a panel's header leads with. */
  function summary(sys: SystemView, cards: SlotView[]): { fig: string; sub: string } {
    const parts = cards.flatMap((c) => (mount(c) ? leaves(c) : [c]));
    const sizes = (ss: SlotView[]) => {
      const by = new Map<number, number>();
      for (const s of ss) if (s.size) by.set(s.size, (by.get(s.size) ?? 0) + 1);
      return [...by.entries()].sort((a, b) => b[0] - a[0]).map(([size, n]) => `${n} × S${size}`).join(" + ");
    };
    const sum = (k: string) => parts.reduce((n, s) => n + (stat(s, k) ?? 0), 0);
    switch (sys) {
      case "weapons":
      case "turrets":
        return { fig: `${num(gunDps(parts), 1)} dps`, sub: sizes(parts.filter((p) => p.itemType === "WeaponGun")) };
      case "missiles":
        return { fig: `${parts.filter((p) => p.itemType === "Missile").length}`, sub: sizes(parts) };
      case "countermeasures":
        return { fig: parts.map((p) => num(stat(p, "ammo") ?? 0)).join(" / "), sub: parts.map(cmKind).join(" / ").toLowerCase() };
      case "shields":
        return { fig: `${num(sum("hp"))} HP`, sub: sizes(parts) };
      case "powerPlants":
        return { fig: `${num(sum("output"))} pips`, sub: sizes(parts) };
      case "coolers":
        return { fig: `${num(sum("coolant"), 1)} /s`, sub: sizes(parts) };
      case "fuel": {
        const h = parts.filter((p) => p.itemType === "FuelTank").reduce((n, p) => n + (stat(p, "capacity") ?? 0), 0);
        const q = parts.filter((p) => p.itemType === "QuantumFuelTank").reduce((n, p) => n + (stat(p, "capacity") ?? 0), 0);
        return { fig: `${num(h, 1)} SCU`, sub: q ? `hydrogen, ${num(q, 1)} SCU quantum` : "hydrogen" };
      }
      default:
        return { fig: "", sub: sizes(parts) };
    }
  }

  /** Rows of a panel: a mount is one row over its parts (folded while
   *  they are all alike); fuel intakes, which hold nothing, are left out.
   *  The pilot's guns of one kind fold the same way, into one row where
   *  the first of them sits (`fold`), "4× CF-557" until opened. */
  type Row = { head: SlotView; parts: SlotView[]; mount: boolean; fold?: boolean };
  function rows(cards: SlotView[]): Row[] {
    const base: Row[] = cards
      .filter((c) => c.itemType !== "FuelIntake")
      .map((c) => (mount(c) ? { head: c, parts: leaves(c), mount: true } : { head: c, parts: [c], mount: false }));
    const gun = (r: Row) => (!r.mount || gimbal(r)) && r.parts.length === 1 && r.parts[0].itemType === "WeaponGun" && !!r.parts[0].item;
    const kind = new Map<string, SlotView[]>();
    for (const r of base) if (gun(r)) kind.set(r.parts[0].item!, [...(kind.get(r.parts[0].item!) ?? []), r.parts[0]]);
    return base.flatMap((r): Row[] => {
      if (!gun(r)) return [r];
      const same = kind.get(r.parts[0].item!)!;
      if (same.length === 1) return [r];
      return same[0] === r.parts[0] ? [{ head: r.head, parts: same, mount: true, fold: true }] : [];
    });
  }
  let open = $state<Record<string, boolean>>({});
  const alike = (ps: SlotView[]) => ps.length > 1 && ps.every((p) => p.item === ps[0].item);

  /** What the guns on the weapons pool ask for against what the pool
   *  gives them now: below 100 % every gun's sustained damage is capped. */
  const gunPower = $derived.by(() => {
    if (!weaponPool) return null;
    const members = new Set(weaponPool.items);
    const all = (ss: SlotView[]): SlotView[] => ss.flatMap((s) => [s, ...all(s.children)]);
    const need = all(slots)
      .filter((s) => members.has(key(s)) && s.itemType === "WeaponGun" && !isOff(s))
      .reduce((n, s) => n + (stat(s, "power") ?? 0), 0);
    if (need <= 0) return null;
    const given = weaponPool.off || weaponPool.excluded ? 0 : weaponPool.level;
    return { need, given, share: Math.min(given / need, 1) };
  });

  const gunsIn = (cards: SlotView[]) => cards.flatMap((c) => (mount(c) ? leaves(c) : [c])).filter((p) => p.itemType === "WeaponGun");

  /** A panel's header line: what its header figure leaves out. */
  function line(sys: SystemView, cards: SlotView[], members: SystemView[]): Fig[] {
    const out: Fig[] = [];
    if (sys === "weapons" || sys === "turrets") {
      const parts = gunsIn(cards);
      const burst = gunBurst(parts);
      const before = stock ? gunBurst(gunsIn(stockCards(members)), stock.guns) : null;
      if (burst > 0) out.push({ v: num(burst), u: "burst", change: change(burst, before) });
      // spviewer's figure: a 30 s engagement from cold, full guns.
      const windowed = (ss: SlotView[], g: Map<string, GunView>) => ss.reduce((n, s) => n + (g.get(key(s))?.window ?? 0), 0);
      const win = windowed(parts, guns);
      if (win > 0)
        out.push({
          v: num(win),
          u: "over 30 s",
          change: change(win, stock ? windowed(gunsIn(stockCards(members)), stock.guns) : null),
        });
      if (members.length === 1 && sys === "turrets") out.push({ v: `${parts.length}`, u: "guns" });
      if (gunPower && members.includes(poolBox))
        out.push({ v: `${num(gunPower.given, 1)} of ${num(gunPower.need, 1)}`, u: `pips, ${Math.round(gunPower.share * 100)} %`, warn: gunPower.share < 1 });
    } else if (sys === "fuel") {
      const sub = summary(sys, cards).sub;
      if (sub) out.push({ v: "", u: sub });
    }
    return [...out, ...(lines?.[sys] ?? [])];
  }

  const bySystem = $derived(new Map(SYSTEMS.map((s) => [s.id, slots.filter((c) => c.system === s.id)])));
  /** The box that carries the weapons pool's share: the pilot's guns, or
   *  the turrets on a ship without. */
  const poolBox = $derived(bySystem.get("weapons")?.length ? "weapons" : "turrets");

  /** A part's name for its place: a gun in a gimbal goes by the gimbal's
   *  port ("Weapon - Top Left", "Turret Weapon Slot 1"), not its own
   *  ("Slot"). */
  /** Siblings sometimes share a label in the game data (the Mk V's
   *  `hardpoint_gun_s5_body_left` and `_body_right` both read "Weapon -
   *  Bottom Left"); the port name is the real key, so a shared label gets
   *  the port's own words after it ("· body right"). */
  const portHint = (s: SlotView) =>
    (s.path.at(-1) ?? "")
      .replace(/^hardpoint_/, "")
      .replace(/^(gun|weapon|turret)_/, "")
      .replace(/^s\d+_/, "")
      .replaceAll("_", " ");
  const labels = $derived.by(() => {
    const out = new Map<string, string>();
    const walk = (ss: SlotView[], above: string | null) => {
      const seen = new Map<string, number>();
      for (const s of ss) if (s.label && s.label !== "Slot") seen.set(s.label, (seen.get(s.label) ?? 0) + 1);
      for (const s of ss) {
        const shared = !!s.label && (seen.get(s.label) ?? 0) > 1;
        const own = s.label && s.label !== "Slot" ? (shared ? `${s.label} · ${portHint(s)}` : s.label) : above;
        if (own) out.set(key(s), own);
        walk(s.children, mount(s) ? own : null);
      }
    };
    walk(slots, null);
    return out;
  });
  /** A gimbal holding one gun is that gun's row, not a fold. */
  const gimbal = (r: Row) => r.mount && r.head.subType === "GunTurret" && r.parts.length === 1;
</script>

{#snippet lead(s: SlotView | null)}
  <span class="sz" class:locked={s && !s.editable} title={s && !s.editable ? "Can't be changed" : undefined}>{s?.size ? `S${s.size}` : ""}</span>
{/snippet}

{#snippet power(on: boolean, paths: string[], label: string)}
  <button type="button" class="pw" class:offb={!on} title={on ? "Switch off" : "Switch on"} aria-label="{on ? 'Switch off' : 'Switch on'} {label}" onclick={() => ontoggle(paths)}
    ><Power size={13} /></button
  >
{/snippet}

{#snippet part(s: SlotView, sub: boolean)}
  {@const k = key(s)}
  {@const offNow = isOff(s)}
  <div class="ln" class:sel={selected === k} class:off={offNow} class:sub>
    <span class="chev"></span>
    {@render lead(s)}
    <button type="button" class="nm" disabled={!s.editable} data-part onclick={() => onpick([s])}>
      <b>{s.itemType === "WeaponDefensive" ? `${cmKind(s)} launcher` : (s.name ?? "Empty")}</b>
      {#if tier(s)}<span class="tier">{tier(s)}</span>{/if}
      {#if s.itemType === "WeaponGun" && damageType(s.stats)}<span class="tier">{damageType(s.stats)}</span>{/if}
      {#if sub && labels.get(k)}<span class="place">{labels.get(k)}</span>{/if}
      {#if facts(s).length}
        <span class="facts">{#each facts(s) as f (f.label + f.text)}<span class="kv">{#if f.label}<i>{f.label}</i>{/if}{f.text}</span>{/each}</span>
      {/if}
    </button>
    <span class="fig">{figure(s)}</span>
    {#if powered.has(k)}{@render power(!offNow, [k], s.name ?? "")}{:else}<span></span>{/if}
  </div>
  {#each s.children.filter((c) => !mount(c) && c.system !== "other" && c.item) as c (key(c))}
    {@render part(c, true)}
  {/each}
{/snippet}

<div class="systems" style:--cols={COLUMNS.length}>
  {#each COLUMNS as col, ci (ci)}
    <div class="col">
      {#each col as { sys, members } (sys.id)}
        {@const cards = members.flatMap((m) => bySystem.get(m.id) ?? [])}
        {@const ids = members.map((m) => m.id)}
        {#if cards.length}
          {@const sum = summary(sys.id, cards)}
          {@const Icon = sys.icon}
          <section class="sys" style:--hue={sys.hue} onmouseenter={() => onhover(sys.id)} onmouseleave={() => onhover(null)} aria-label={sys.label}>
            <header>
              <Icon size={14} />
              <b>{sys.label}</b>
              <span class="sfig">{sum.fig}</span>
              {#if lines}
                {@const ch = headChange(sys.id, ids, cards)}
                {#if ch}<small class="chg" class:good={ch.good} class:bad={!ch.good}>{ch.text}</small>{/if}
              {/if}
            </header>
            {#if lines}
              {@const figs = line(sys.id, cards, ids)}
              {#if figs.length}
                <p class="sline">
                  {#each figs as f, i (i)}<span class:warn={f.warn}
                      >{#if f.v}<b>{f.v}</b>{/if}{f.u}{#if f.change}<small class="chg" class:good={f.change.good} class:bad={!f.change.good}
                          >{f.change.text}</small
                        >{/if}</span
                    >{/each}
                </p>
              {/if}
            {:else if gunPower && sys.id === poolBox}
              {@const g = gunPower}
              <div class="need" title="The weapons pool feeds every gun, pilot and turret alike">
                <span>Power</span>
                <span class="tr"><i class:short={g.share < 1} style:width="{g.share * 100}%"></i></span>
                <b>{num(g.given, 1)} of {num(g.need, 1)} pips</b>
                <small class:short={g.share < 1}>{Math.round(g.share * 100)} %</small>
              </div>
            {/if}
            {#if members.length > 1}
              {#each members as m (m.id)}
                {@const own = bySystem.get(m.id) ?? []}
                {#if own.length}
                  {@const gs = gunsIn(own)}
                  <div class="grp">
                    <b>{GROUP[m.id] ?? m.label}</b>
                    {#if gs.length}<span>{gs.length} guns</span>{/if}
                    <span class="fig">{gs.length ? `${num(gunDps(gs), 1)} dps` : ""}</span>
                  </div>
                  {#each rows(own) as r (key(r.head))}{@render row(r)}{/each}
                {/if}
              {/each}
            {:else}
              {#each rows(cards) as r (key(r.head))}{@render row(r)}{/each}
            {/if}
          </section>
        {/if}
      {/each}
    </div>
  {/each}
</div>

{#snippet row(r: Row, sub = false)}
  {#if r.fold}
    {@const k = `fold:${key(r.head)}`}
    {@const fold = !open[k]}
    {@const lead0 = r.parts[0]}
    {@const offN = r.parts.filter(isOff).length}
    {@const paths = r.parts.filter((p) => powered.has(key(p))).map(key)}
    <div class="ln" class:off={offN === r.parts.length} class:sub>
      <button type="button" class="chev" onclick={() => (open[k] = !open[k])} aria-expanded={!fold} aria-label={fold ? "Show each" : "Fold"}>
        {#if fold}<ChevronRight size={13} />{:else}<ChevronDown size={13} />{/if}
      </button>
      {@render lead(lead0)}
      <button type="button" class="nm" onclick={() => (open[k] = !open[k])}>
        <b>{r.parts.length}× {lead0.name}</b>
        {#if damageType(lead0.stats)}<span class="tier">{damageType(lead0.stats)}</span>{/if}
        {#if offN && offN < r.parts.length}<span class="place">{offN} off</span>{/if}
        {#if facts(lead0).length}
          <span class="facts">{#each facts(lead0) as f (f.label + f.text)}<span class="kv">{#if f.label}<i>{f.label}</i>{/if}{f.text}</span>{/each}</span>
        {/if}
      </button>
      <span class="fig">{num(gunDps(r.parts), 1)} dps</span>
      {#if paths.length}{@render power(offN < r.parts.length, paths, lead0.name ?? "")}{:else}<span></span>{/if}
    </div>
    {#if !fold}
      {#each r.parts as p (key(p))}{@render part(p, true)}{/each}
      {#if r.parts.every((p) => p.editable)}
        <button type="button" class="all" data-part onclick={() => onpick(r.parts)}>Swap all {r.parts.length}</button>
      {/if}
    {/if}
  {:else if gimbal(r)}
    {@render part(r.parts[0], sub)}
  {:else if r.mount}
    {@const k = key(r.head)}
    {@const fold = !open[k]}
    {@const mOff = isOff(r.head)}
    {@const dps = gunDps(r.parts)}
    <div class="ln" class:off={mOff} class:sub>
      <button type="button" class="chev" onclick={() => (open[k] = !open[k])} aria-expanded={!fold} aria-label={fold ? "Show each" : "Fold"}>
        {#if fold}<ChevronRight size={13} />{:else}<ChevronDown size={13} />{/if}
      </button>
      {@render lead(r.parts[0] ?? null)}
      <button type="button" class="nm" onclick={() => (open[k] = !open[k])}>
        <b>{r.head.label ?? r.head.name}</b>
        <span class="place">{r.parts.length}× {alike(r.parts) || r.parts.length === 1 ? r.parts[0]?.name : "mixed parts"}</span>
        {#if r.parts[0]?.itemType === "WeaponGun" && (alike(r.parts) || r.parts.length === 1) && damageType(r.parts[0].stats)}<span class="tier">{damageType(r.parts[0].stats)}</span>{/if}
        {#if (alike(r.parts) || r.parts.length === 1) && r.parts[0] && facts(r.parts[0]).length}
          <span class="facts">{#each facts(r.parts[0]) as f (f.label + f.text)}<span class="kv">{#if f.label}<i>{f.label}</i>{/if}{f.text}</span>{/each}</span>
        {/if}
      </button>
      <span class="fig">{dps > 0 ? `${num(dps, 1)} dps` : ""}</span>
      {#if powered.has(k) || r.parts.some((p) => powered.has(key(p)))}{@render power(!mOff, [k], r.head.label ?? r.head.name ?? "")}{:else}<span></span>{/if}
    </div>
    {#if !fold}
      {#if r.head.itemType === "MissileLauncher"}
        <div class="racks">
          {#each r.parts as p (key(p))}
            <button type="button" class="slotchip" class:sel={selected === key(p)} disabled={!p.editable} data-part onclick={() => onpick([p])} title={p.label}>
              <small>{p.label?.replace(/^Missile Slot /, "") ?? ""}</small><b>{p.name}</b>
            </button>
          {/each}
        </div>
      {:else}
        {#each r.parts as p (key(p))}{@render part(p, true)}{/each}
      {/if}
      {#if r.parts.length > 1 && r.parts.every((p) => p.editable)}
        <button type="button" class="all" data-part onclick={() => onpick(r.parts)}>Swap all {r.parts.length}</button>
      {/if}
    {/if}
  {:else}
    {@render part(r.head, sub)}
  {/if}
{/snippet}

<style>
  .systems {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
    gap: 12px;
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .sys {
    display: flex;
    flex-direction: column;
    padding: 8px 10px 6px;
    border: 1px solid color-mix(in srgb, var(--hue) 22%, var(--border));
    border-radius: var(--radius-lg);
    background: linear-gradient(
      160deg,
      color-mix(in srgb, var(--hue) 9%, var(--bg-raised)) 0%,
      color-mix(in srgb, var(--bg-raised) 60%, transparent) 55%
    );
  }
  /* One line: glyph, system, its figure. */
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 2px 6px;
    color: color-mix(in srgb, var(--hue) 85%, var(--text-strong));
  }
  header b {
    flex: 1;
    font-family: var(--font-display);
    font-size: 0.78rem;
    font-weight: var(--weight-bold);
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: color-mix(in srgb, var(--hue) 35%, var(--text-dim));
  }
  /* Brightness ladder: a panel's one bright thing is its header figure;
   * names and the figure a row is compared by are plain text; the line
   * figures under a name are dim, their labels and the chips faint. */
  .sfig {
    font: 700 0.95rem var(--font-mono);
    color: color-mix(in srgb, var(--hue) 55%, var(--text-strong));
    white-space: nowrap;
  }
  /* The header line: the system's other figures, quieter than its lead
   * one, separated by a dot. */
  .sline {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 0;
    margin: -4px 0 0;
    padding: 0 2px 6px 24px;
    font-size: 0.78rem;
    color: var(--text-dim);
  }
  .sline span + span::before {
    content: "·";
    margin: 0 8px;
    color: var(--text-faint);
  }
  .sline b {
    margin-right: 4px;
    font: 500 0.8rem var(--font-mono);
    color: var(--text);
  }
  .sline .warn,
  .sline .warn b {
    color: var(--warn);
  }
  /* A figure's change against stock, after it: small, toned. */
  .chg {
    margin-left: 5px;
    font: 500 0.72rem var(--font-mono);
    white-space: nowrap;
  }
  .chg.good {
    color: var(--good);
  }
  .chg.bad {
    color: var(--bad);
  }
  /* A group inside a shared panel (Pilot, Turrets): who controls the
   * parts under it, and what they deal. */
  .grp {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto 24px;
    gap: 6px;
    align-items: baseline;
    padding: 8px 2px 4px 20px;
    border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  }
  .grp b {
    font: var(--weight-bold) 0.7rem var(--font-display);
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: color-mix(in srgb, var(--hue) 45%, var(--text-dim));
  }
  .grp span {
    font-size: 0.78rem;
    color: var(--text-dim);
  }
  .grp .fig {
    font-weight: 500;
    color: var(--text-dim);
  }
  /* The guns' power: what the pool gives against what they ask for. */
  .need {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    gap: 8px;
    align-items: center;
    padding: 2px 2px 8px;
    font-size: 0.8rem;
    color: var(--text-dim);
  }
  .need .tr {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-sunk);
    overflow: hidden;
  }
  .need .tr i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
  }
  .need .tr i.short {
    background: var(--warn);
  }
  .need b {
    font: 600 0.8rem var(--font-mono);
    color: var(--text-strong);
  }
  .need small {
    font: 600 0.8rem var(--font-mono);
    color: var(--good);
  }
  .need small.short {
    color: var(--warn);
  }
  /* A part line: chevron, size, name (and grade), figure, power. Every
   * line shares the columns, so sizes, names and figures align down a
   * panel; names wrap, never cut. */
  .ln {
    display: grid;
    grid-template-columns: 14px 26px minmax(0, 1fr) auto 24px;
    gap: 6px;
    align-items: center;
    min-height: 30px;
    padding: 2px 0;
    border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  }
  .ln.sub {
    padding-left: 12px;
  }
  .ln.sel {
    background: var(--accent-fill-weak);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .ln.off .nm,
  .ln.off .fig,
  .ln.off .sz {
    opacity: 0.42;
  }
  .chev {
    all: unset;
    display: flex;
    color: var(--text-faint);
    cursor: pointer;
  }
  .sz {
    font: 700 0.64rem var(--font-mono);
    text-align: center;
    padding: 1px 0;
    border-radius: var(--radius-chip);
    color: color-mix(in srgb, var(--hue) 80%, var(--text));
    background: color-mix(in srgb, var(--hue) 14%, transparent);
  }
  .sz:empty {
    background: none;
  }
  .sz.locked {
    color: var(--text-faint);
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .nm {
    all: unset;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 8px;
    min-width: 0;
    padding: 3px 0;
  }
  .nm:not(:disabled) {
    cursor: pointer;
  }
  .nm:not(:disabled):hover b {
    color: color-mix(in srgb, var(--hue) 70%, var(--text-strong));
  }
  .nm b {
    font-size: 0.86rem;
    font-weight: var(--weight-medium);
    color: var(--text);
    text-wrap: pretty;
  }
  /* Second-line text is read at a glance: body-sized, dimmed but not
   * faint. */
  .place {
    font-size: 0.8rem;
    color: var(--text-dim);
  }
  /* The part's key figures, on their own line under the name. */
  .facts {
    flex-basis: 100%;
    display: flex;
    flex-wrap: wrap;
    gap: 0 14px;
  }
  .kv {
    font: 0.78rem var(--font-mono);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .kv i {
    margin-right: 5px;
    font: normal 0.76rem var(--font-sans);
    color: var(--text-faint);
  }
  /* Grade and damage type: a quiet tag, no box. */
  .tier {
    font: 500 0.66rem var(--font-mono);
    letter-spacing: 0.04em;
    color: color-mix(in srgb, var(--hue) 40%, var(--text-faint));
    white-space: nowrap;
  }
  .fig {
    font: 500 0.82rem var(--font-mono);
    color: var(--text);
    text-align: right;
    white-space: nowrap;
  }
  .pw {
    all: unset;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius);
    color: var(--good);
    cursor: pointer;
  }
  .pw:hover {
    background: var(--accent-fill-faint);
  }
  .pw.offb {
    color: var(--text-faint);
  }
  .racks {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 4px;
    padding: 4px 0 4px 12px;
  }
  .slotchip {
    all: unset;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 3px 6px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    cursor: pointer;
  }
  .slotchip:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--hue) 50%, var(--border));
  }
  .slotchip.sel {
    border-color: var(--accent);
  }
  .slotchip small {
    font: 0.6rem var(--font-mono);
    color: var(--text-faint);
  }
  .slotchip b {
    font-size: 0.72rem;
    font-weight: var(--weight-medium);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .all {
    all: unset;
    align-self: flex-end;
    padding: 2px 0 4px;
    font-size: var(--text-small);
    color: var(--accent);
    cursor: pointer;
  }
  .all:hover {
    text-decoration: underline;
  }
</style>
