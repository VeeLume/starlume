// Shared wording and figures for the mining views: number formats, the
// difficulty word, the charge bar's steps from the throttle guide, the
// rock classes in power order, and module effects as chips.

import type {
  EvalView,
  GuideView,
  LoadoutView,
  MiningCatalogView,
  ModifiersView,
  PickView,
  RockClassView,
} from "$lib/bindings";

export const pct = (v: number, digits = 0) => `${(v * 100).toFixed(digits)} %`;
export const int = (v: number) => Math.round(v).toLocaleString("en-US");
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
  if (c >= 0.8) return { word: "Easy", tone: "good" };
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

/** The rock classes ordered by power: too much, just right, too little. */
export const CLASSES: { id: RockClassView; label: string; hue: string }[] = [
  { id: "tooMuchPower", label: "too much power", hue: "var(--hue-apricot)" },
  { id: "safe", label: "safe", hue: "var(--good)" },
  { id: "okay", label: "okay", hue: "var(--warn)" },
  { id: "dangerous", label: "dangerous", hue: "var(--bad)" },
  { id: "impossible", label: "can't break", hue: "var(--text-faint)" },
];

/** The rock finder's picks: which rock of a type to roll. */
export type Pick = "common" | "small" | "average" | "large" | "ore" | "quality" | `class:${RockClassView}`;

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
}

/** The charge bar in steps, bottom up: the climb, the window, and the way
 *  back from an overshoot. Empty when the rock can't be cracked. */
export function steps(g: GuideView): Step[] {
  const out: Step[] = [];
  if (g.balance === null) return out;
  const p = (t: number) => `${Math.round(t * 100)} %`;
  let at = 0;
  if (g.fullUntil !== null && g.fullUntil > 0) {
    out.push({ from: 0, to: g.fullUntil, tone: "climb", act: "Full throttle" });
    at = g.fullUntil;
  }
  const tops = [
    ...[...g.climb].sort((a, b) => a.untilCharge - b.untilCharge).map((s) => ({ at: s.untilCharge, t: s.throttle })),
    { at: g.windowLow, t: g.safeClimb ?? 0 },
  ];
  for (const top of tops) {
    if (top.at <= at) continue;
    const act = top.t <= g.throttleMinimum ? "Climb at minimum" : `Throttle up to ${p(top.t)}`;
    out.push({ from: at, to: top.at, tone: "climb", act });
    at = top.at;
  }
  let hold = "";
  if (g.tooMuchPower) hold = "Pulse the beam at minimum";
  else if (g.holdThrottle !== null) {
    const t = g.holdThrottle;
    const at = t <= g.throttleMinimum ? "at minimum" : `about ${p(t)}`;
    hold = g.riding ? `Ride the waves ${at}` : `Hold ${at}`;
  }
  if (hold) out.push({ from: g.windowLow, to: g.windowHigh, tone: "good", act: hold });
  const lost = g.lostFrom ?? 1;
  const minUntil = Math.min(g.keepOnBelow ?? g.windowHigh, lost);
  if (minUntil > g.windowHigh) out.push({ from: g.windowHigh, to: minUntil, tone: "warn1", act: "Drop to minimum" });
  if (lost > minUntil) out.push({ from: minUntil, to: lost, tone: "warn2", act: "Cut the beam" });
  if (lost < 1) out.push({ from: lost, to: 1, tone: "bad", act: "It will overcharge" });
  return out;
}

/** A module's or gadget's effects as chips: amber hurts, green helps. */
export function effects(m: ModifiersView, powerMultiplier = 1): { text: string; good: boolean }[] {
  const out: { text: string; good: boolean }[] = [];
  const add = (label: string, v: number | null, goodWhenUp: boolean) => {
    if (v === null || Math.abs(v) < 0.05) return;
    out.push({ text: `${label} ${v > 0 ? "+" : "−"}${Math.abs(v).toFixed(0)} %`, good: v > 0 === goodWhenUp });
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

/** Your own lasers (not a crew mate's brought along). */
export const ownLasers = (l: LoadoutView) => l.lasers.filter((x) => x.who === null);

/** Two loadouts with the same lasers, modules and gadget. */
export const sameLoadout = (a: LoadoutView, b: LoadoutView) =>
  JSON.stringify([a.lasers.map((l) => [l.laser, l.modules, l.owner]), a.gadget]) ===
  JSON.stringify([b.lasers.map((l) => [l.laser, l.modules, l.owner]), b.gadget]);

/** One line for a loadout's lasers: "Pitman", "3 × Lancet MH2". */
export function lasersLine(l: LoadoutView, names: Map<string, string>): string {
  const counts = new Map<string, number>();
  for (const x of l.lasers) counts.set(x.laser, (counts.get(x.laser) ?? 0) + 1);
  return [...counts]
    .map(([id, n]) => `${n > 1 ? `${n} × ` : ""}${laserShort(names.get(id) ?? "Unknown laser")}`)
    .join(", ");
}
