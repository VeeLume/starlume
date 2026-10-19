// Shared wording and figures for the mining views: number formats, the
// difficulty word, the charge bar's steps from the throttle guide, the
// rock classes in power order, and module effects as chips.

import type {
  EvalView,
  GuideView,
  LaserInputView,
  LoadoutView,
  MiningCatalogView,
  ModifiersView,
  PickView,
  RockClassView,
} from "$lib/bindings";

export const pct = (v: number, digits = 0) => `${(v * 100).toFixed(digits)}%`;
export const int = (v: number) => Math.round(v).toLocaleString("en-US");
/** Tonnes, short: 863, 1.4k, 27k. */
export const kt = (m: number) => (m >= 10000 ? `${Math.round(m / 1000)}k` : m >= 1000 ? `${(m / 1000).toFixed(1)}k` : `${Math.round(m)}`);
/** The finder's mass scale, shared by every row so big and small rocks
 *  compare across places and ores: log, 50 t to 150k t. */
export const MASS_SCALE = {
  min: 50,
  max: 150_000,
  ticks: [
    [100, "100 t"],
    [1000, "1k"],
    [10_000, "10k"],
    [100_000, "100k"],
  ] as [number, string][],
};
/** Where a mass sits on the finder's scale, in percent. */
export const massAt = (m: number) =>
  Math.max(0, Math.min(100, (Math.log(Math.max(m, 1) / MASS_SCALE.min) / Math.log(MASS_SCALE.max / MASS_SCALE.min)) * 100));
export const secs = (v: number | null) => (v === null ? "—" : `~${Math.round(v)} s`);

/** "Raw Ouratite" and "Agricium (Ore)" read as the rock's ore: "Ouratite". */
export function oreName(element: string): string {
  return element.replace(/^Raw\s+/i, "").replace(/\s*\((Ore|Raw)\)$/i, "");
}

/** Every catalog id's display name. */
export function namesOf(catalog: MiningCatalogView): Map<string, string> {
  return new Map<string, string>([
    ...catalog.lasers.map((l) => [l.id, l.name] as [string, string]),
    ...catalog.modules.map((m) => [m.id, m.name] as [string, string]),
    ...catalog.gadgets.map((g) => [g.id, g.name] as [string, string]),
    ...catalog.elements.map((e) => [e.id, e.name] as [string, string]),
  ]);
}

/** "Mining Laser" dropped: the card's second line says what it is. */
export const laserShort = (name: string) => name.replace(/\s+Mining Laser$/i, "");

export type Tone = "good" | "warn" | "bad" | "faint" | "power";

/** Starlume's difficulty word for a setup: our player model's clean
 *  cracks, or why there are none. */
export function difficulty(e: EvalView, guide: GuideView | null): { word: string; tone: Tone } {
  if (!e.difficulty) return { word: "Can't break", tone: "faint" };
  if (guide?.tooMuchPower) return { word: "Too much power", tone: "power" };
  const c = e.difficulty.controlled;
  // A window narrower than a person misjudges the charge is never easy.
  if (c >= 0.8 && !e.narrowWindow) return { word: "Easy", tone: "good" };
  if (c >= 0.5) return { word: "Manageable", tone: "warn" };
  if (c >= 0.2) return { word: "Hard", tone: "bad" };
  return { word: "Very hard", tone: "bad" };
}

export const TONE_HUE: Record<Tone, string> = {
  good: "var(--good)",
  warn: "var(--warn)",
  bad: "var(--bad)",
  faint: "var(--text-faint)",
  power: "var(--hue-apricot)",
};

/** The rock classes ordered by power: too much, then by how cleanly our
 *  player model cracks it (the difficulty word's thresholds), then too
 *  little. */
export const CLASSES: { id: RockClassView; label: string; hue: string }[] = [
  { id: "tooMuchPower", label: "too much power", hue: "var(--hue-apricot)" },
  { id: "easy", label: "easy", hue: "var(--good)" },
  { id: "manageable", label: "manageable", hue: "var(--warn)" },
  { id: "hard", label: "hard", hue: "var(--bad)" },
  { id: "impossible", label: "can't break", hue: "var(--text-faint)" },
];

/** The rock finder's picks: which rock of a type to roll. */
export type Pick =
  | "common"
  | "small"
  | "average"
  | "large"
  | "ore"
  | "quality"
  | `class:${RockClassView}`
  | `mass:${number}`;

/** A pick as the generator's request, and its label. */
export function pickRequest(pick: Pick, ore: string, oreLabel: string): { view: PickView; label: string } {
  switch (pick) {
    case "common":
      return { view: { kind: "mostCommon" }, label: "Most common" };
    case "small":
      return { view: { kind: "smallest" }, label: "Lightest" };
    case "average":
      return { view: { kind: "average" }, label: "Median" };
    case "large":
      return { view: { kind: "largest" }, label: "Heaviest" };
    case "ore":
      return { view: { kind: "mostOre", element: ore }, label: `Richest in ${oreLabel}` };
    case "quality":
      return { view: { kind: "bestQuality", element: ore }, label: `Best ${oreLabel}` };
    default: {
      if (pick.startsWith("mass:")) {
        const mass = Number(pick.slice("mass:".length));
        return { view: { kind: "mass", mass }, label: `At ${int(mass)} t` };
      }
      const id = pick.slice("class:".length) as RockClassView;
      const label = CLASSES.find((c) => c.id === id)?.label ?? id;
      return { view: { kind: "class", class: id }, label: label[0].toUpperCase() + label.slice(1) };
    }
  }
}

export type StepTone = "climb" | "good" | "warn1" | "warn2" | "bad";
export interface Step {
  from: number;
  to: number;
  tone: StepTone;
  act: string;
  /** The step's number on the bar (the window's two halves share one). */
  n: number;
  /** The window kept in two settings: its lower half (a slight rise) or
   *  its upper half (a slight fall). */
  half?: "low" | "high";
}

/** "2▲", "2▼", "3". */
export const stepLabel = (s: Step) => `${s.n}${s.half === "low" ? "▲" : s.half === "high" ? "▼" : ""}`;

/** The charge bar in steps, bottom up: the climb, the window, and the way
 *  back from an overshoot. Empty when the rock can't be cracked. */
export function steps(g: GuideView): Step[] {
  const out: Omit<Step, "n">[] = [];
  // Out of reach: the bar still shows where the window is and how wide,
  // what a better loadout aims for.
  if (g.balance === null)
    return [
      { from: 0, to: g.windowLow, tone: "climb", act: "Even 100% can't raise it", n: 1 },
      { from: g.windowLow, to: g.windowHigh, tone: "good", act: "The window, out of reach", n: 2 },
    ];
  const p = (t: number) => `${Math.round(t * 100)}%`;
  let at = 0;
  // One vocabulary (decision 2026-10-17): the climb is always "Up to x%",
  // the window always "Keep at x%".
  if (g.fullUntil !== null && g.fullUntil > 0) {
    out.push({ from: 0, to: g.fullUntil, tone: "climb", act: "Up to 100%" });
    at = g.fullUntil;
  }
  const tops = [
    ...[...g.climb].sort((a, b) => a.untilCharge - b.untilCharge).map((s) => ({ at: s.untilCharge, t: s.throttle })),
    { at: g.windowLow, t: g.safeClimb ?? 0 },
  ];
  for (const top of tops) {
    if (top.at <= at) continue;
    const act = top.t <= g.throttleMinimum ? "Up at minimum" : `Up to ${p(top.t)}`;
    out.push({ from: at, to: top.at, tone: "climb", act });
    at = top.at;
  }
  // The window, kept the way people keep it (decision 2026-10-18): a slight
  // rise in its lower half, a slight fall in its upper half.
  const keep = (t: number) => (t <= g.throttleMinimum ? "Keep at minimum" : `Keep at ${p(t)}`);
  // A window that can't be kept still shows its settings (the simulated
  // tries fly them); the card's title and strip say how badly it goes.
  // Too much power is pulsed where the beam-on and beam-off windows overlap
  // (a cut drops the lasers' window modifiers 0.25 s later): fire at the
  // minimum in its lower half, cut the beam in its upper half.
  let top = g.windowHigh;
  if (g.tooMuchPower) {
    let lo = Math.max(g.windowLow, g.windowLowOff);
    let hi = Math.min(g.windowHigh, g.windowHighOff);
    if (lo >= hi) [lo, hi] = [g.windowLow, g.windowHigh];
    if (lo > g.windowLow) out.push({ from: g.windowLow, to: lo, tone: "climb", act: "Up at minimum" });
    const mid = (lo + hi) / 2;
    out.push({ from: lo, to: mid, tone: "good", act: "Fire at minimum", half: "low" });
    out.push({ from: mid, to: hi, tone: "good", act: "Cut the beam", half: "high" });
    top = hi;
  } else if (g.keepLow !== null && g.keepHigh !== null) {
    const mid = (g.windowLow + g.windowHigh) / 2;
    out.push({ from: g.windowLow, to: mid, tone: "good", act: keep(g.keepLow), half: "low" });
    out.push({ from: mid, to: g.windowHigh, tone: "good", act: keep(g.keepHigh), half: "high" });
  }
  const lost = g.lostFrom ?? 1;
  // pulsing is at the minimum already: above the overlap the beam stays off
  const minUntil = g.tooMuchPower ? top : Math.min(g.keepOnBelow ?? g.windowHigh, lost);
  if (minUntil > top) out.push({ from: top, to: minUntil, tone: "warn1", act: "Drop to minimum" });
  if (lost > minUntil) out.push({ from: minUntil, to: lost, tone: "warn2", act: g.tooMuchPower ? "Keep the beam off" : "Cut the beam" });
  if (lost < 1) out.push({ from: lost, to: 1, tone: "bad", act: "It will overcharge" });
  let n = 0;
  return out.map((s) => ({ ...s, n: s.half === "high" ? n : ++n }));
}

/** A module's or gadget's effects as chips: red hurts, green helps. */
export function effects(m: ModifiersView, powerMultiplier = 1): { text: string; good: boolean }[] {
  const out: { text: string; good: boolean }[] = [];
  const add = (label: string, v: number | null, goodWhenUp: boolean) => {
    if (v === null || Math.abs(v) < 0.05) return;
    out.push({ text: `${label} ${v > 0 ? "+" : "−"}${Math.abs(v).toFixed(0)}%`, good: v > 0 === goodWhenUp });
  };
  if (Math.abs(powerMultiplier - 1) > 0.0005) add("power", (powerMultiplier - 1) * 100, true);
  add("resistance", m.resistance, false);
  add("instability", m.instability, false);
  add("window", m.windowSize, true);
  add("charge rate", m.windowRate, true);
  add("shatter", m.shatterDamage, false);
  add("cluster", m.clusterFactor, true);
  add("overcharge rate", m.catastrophicRate, false);
  return out;
}

/** Your own lasers (not a crew mate's brought along); a MOLE seat's name
 *  does not make a head someone else's ship. */
export const ownLasers = (l: LoadoutView) => l.lasers.filter((x) => x.crew === null);

/** A fraction as a signed percent ("+38%", "−35%"); "—" for no change. */
export function signedPct(v: number): string {
  const r = Math.round(v * 100);
  return r === 0 ? "—" : `${r > 0 ? "+" : "−"}${Math.abs(r)}%`;
}

/** Each laser's operator: its seat or crew mate's name, "You" for the first
 *  one you work, and "Head 2", "Head 3" for a ship's other unnamed lasers. */
export function operatorNames(lasers: LaserInputView[]): string[] {
  const first = lasers.findIndex((l) => l.who === null && l.crew === null);
  return lasers.map((l, i) => {
    if (l.who) return l.who;
    if (i === first) return "You";
    const sameShip = lasers.filter((x) => x.owner === l.owner);
    return `Head ${sameShip.indexOf(l) + 1}`;
  });
}

/** Where an operator's laser sits: "head 2" on a ship with several, the
 *  crew loadout's name ("Prospector") for a crew mate's ship. */
export function operatorPlace(
  lasers: LaserInputView[],
  i: number,
  crewName: (id: string) => string | undefined,
): string {
  const l = lasers[i];
  const sameShip = lasers.filter((x) => x.owner === l.owner);
  if (sameShip.length > 1) return `head ${sameShip.indexOf(l) + 1}`;
  return l.crew ? (crewName(l.crew) ?? "") : "";
}

/** A wave-table row in the steps' words: Keep at, Up to, the minimum, or
 *  (any other throttle) what its bars show, so a label never contradicts
 *  them. The guide's labels (hold, climb limit) stay out of the UI. */
export function waveRole(
  label: string | null,
  big: number,
  usual: number,
  win: number,
  keepStep: number | null,
  upStep: number | null,
): { step: number | null; tone: "good" | "warn" | ""; say: string } {
  if (label === "keep low") return { step: keepStep, tone: "good", say: "Keep at ▲ lower half" };
  if (label === "keep high") return { step: keepStep, tone: "good", say: "Keep at ▼ upper half" };
  if (label === "hold") return { step: keepStep, tone: "good", say: "Keep at" };
  if (label === "climb limit") return { step: upStep, tone: "", say: "Up to" };
  if (label === "minimum") return { step: null, tone: "", say: "minimum throttle" };
  if (usual > win) return { step: null, tone: "warn", say: "usual waves jump out" };
  if (big > win) return { step: null, tone: "warn", say: "big waves jump out" };
  if (big <= 0.0005) return { step: null, tone: "", say: "no lift: the charge drains" };
  return { step: null, tone: "", say: "stays inside" };
}

/** The plan's step numbers (1-based) for Keep at (the window) and the last
 *  Up to (the climb into it). */
export function stepNumbers(steps: Step[], g: GuideView): { keep: number | null; up: number | null } {
  const keep = steps.find((s) => s.tone === "good");
  const up = steps.findLast((s) => s.tone === "climb");
  return { keep: keep?.n ?? null, up: up?.n ?? null };
}

// ── Figures in aligned columns ──────────────────────────────────────────────
// The picker, the gadget table and the head tiles show modifiers the same
// way: a signed figure, green where it helps cracking, red where it hurts.

export type FxKey = "power" | "resistance" | "instability" | "window" | "charge" | "overcharge";

/** Each column: its short label, its word in a line, and which way helps. */
export const FX_COLUMNS: { key: FxKey; label: string; word: string; up: boolean }[] = [
  { key: "power", label: "power", word: "power", up: true },
  { key: "resistance", label: "resist.", word: "resistance", up: false },
  { key: "instability", label: "instab.", word: "instability", up: false },
  { key: "window", label: "window", word: "window", up: true },
  { key: "charge", label: "charge", word: "charge rate", up: true },
  { key: "overcharge", label: "overch.", word: "overcharge rate", up: false },
];

/** A part's figures by column, in percent; 0 where it has none. */
export function fxOf(m: ModifiersView, powerMultiplier = 1): Record<FxKey, number> {
  const r = (v: number | null) => (v === null || Math.abs(v) < 0.05 ? 0 : Math.round(v));
  return {
    power: Math.round((powerMultiplier - 1) * 100),
    resistance: r(m.resistance),
    instability: r(m.instability),
    window: r(m.windowSize),
    charge: r(m.windowRate),
    overcharge: r(m.catastrophicRate),
  };
}

/** "+25", "−16". */
export const signed = (v: number) => `${v > 0 ? "+" : "−"}${Math.abs(v)}`;

/** Whether a figure helps cracking. */
export const helps = (key: FxKey, v: number) => (v > 0) === (FX_COLUMNS.find((c) => c.key === key)?.up ?? true);

// ── Module families ─────────────────────────────────────────────────────────

/** "Focus III Module" → family "Focus", tier 3; "Rieger-C2" → "Rieger", 2;
 *  "FLTR-XL" → "FLTR", 3. A name without a tier is tier 1. */
export function familyOf(name: string): { family: string; tier: number; short: string } {
  const short = name.replace(/\s+Module$/i, "");
  const m = short.match(/^(.*?)(?:[\s-](I{1,3}|IV|C[1-3]|L|XL))$/);
  if (!m) return { family: short, tier: 1, short };
  const tier = { I: 1, II: 2, III: 3, IV: 4, C1: 1, C2: 2, C3: 3, L: 2, XL: 3 }[m[2]] ?? 1;
  return { family: m[1], tier, short };
}

/** One line for a loadout's lasers: "Pitman", "3 × Lancet MH2". */
export function lasersLine(l: LoadoutView, names: Map<string, string>): string {
  const counts = new Map<string, number>();
  for (const x of l.lasers) counts.set(x.laser, (counts.get(x.laser) ?? 0) + 1);
  return [...counts]
    .map(([id, n]) => `${n > 1 ? `${n} × ` : ""}${laserShort(names.get(id) ?? "Unknown laser")}`)
    .join(", ");
}
