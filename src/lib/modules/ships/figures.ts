// What a part is chosen by, per item type: the picker's columns, and the
// first of them are the part's line on the Build tab. The choice follows
// how players compare parts (Firefly note "Choosing ship components: what
// a player compares, per type, and how power gets split"). EM is left out
// as a figure of its own: it follows the power a part draws, which the
// power panel already shows; per pip it says how quiet a plant is. Shields
// are the exception: their full-power EM is compared in the shops.
import type { ShieldView, StatView } from "$lib/bindings";

export type Figure = {
  key: string;
  /** The picker's column head. */
  label: string;
  /** The part line's label; "" when the unit says what it is (1,390 m/s). */
  line: string;
  unit: string;
  /** 1: higher is better, -1: lower is better, 0: neither. */
  better: number;
  digits: number;
  /** On the part line too, or only in the picker. */
  onLine: boolean;
  get: (stats: StatView[]) => number | null;
  /** Text for a value that is not a plain number (a missile's tracking). */
  text?: (v: number) => string;
  /** What the figure means, for the picker's column head. */
  hint?: string;
};

const value = (stats: StatView[], key: string) => stats.find((s) => s.key === key)?.value ?? null;

type Opts = Partial<Pick<Figure, "line" | "digits" | "onLine" | "text" | "hint">>;

const raw = (key: string, label: string, unit: string, better: number, o: Opts = {}): Figure => ({
  key,
  label,
  line: o.line ?? label.toLowerCase(),
  unit,
  better,
  digits: o.digits ?? 0,
  onLine: o.onLine ?? true,
  text: o.text,
  hint: o.hint,
  get: (s) => value(s, key),
});

/** One figure over another (coolant per pip of power). */
const per = (key: string, label: string, num: string, den: string, unit: string, better: number, o: Opts = {}): Figure => ({
  ...raw(key, label, unit, better, { digits: 1, ...o }),
  get: (s) => {
    const a = value(s, num);
    const b = value(s, den);
    return a !== null && b ? a / b : null;
  },
});

// Power is a picker column only: the power panel shows it per system, and
// the weapons' line says what the pool gives against what the guns ask for.
const power = raw("power", "Power", "pips", -1, { digits: 1, onLine: false });
const distortion = [
  raw("distortion", "Dist. pool", "", 1, {
    onLine: false,
    hint: "Distortion damage the part soaks before it shuts down: how long it lasts under distortion fire (a trench run).",
  }),
  raw("distRecover", "Dist. recovery", "s", -1, {
    digits: 1,
    onLine: false,
    hint: "Seconds from a full distortion pool until the part runs again: the wait before the decay starts, then the drain.",
  }),
];

export const SIGNALS = ["IR", "EM", "CS"];

export const FIGURES: Record<string, Figure[]> = {
  WeaponGun: [
    raw("dps", "DPS", "", 1, { onLine: false }),
    raw("alpha", "Alpha", "", 1, { digits: 1 }),
    raw("speed", "Speed", "m/s", 1, { line: "" }),
    raw("burst", "Fires for", "s", 1, { line: "fires", digits: 1 }),
    raw("magazine", "Magazine", "s", 1, { line: "magazine", digits: 0 }),
    raw("ammo", "Ammo", "", 1, { onLine: false }),
    raw("pen", "Penetration", "m", 1, { line: "pen", digits: 2 }),
    raw("rpm", "Fire rate", "rpm", 1, { onLine: false }),
    power,
    raw("range", "Range", "m", 1, { onLine: false }),
  ],
  Shield: [
    raw("hp", "HP", "", 1),
    raw("delay", "Back up after hit", "s", -1, { line: "after hit", digits: 1 }),
    raw("downDelay", "Back up when down", "s", -1, { line: "when down", digits: 1 }),
    raw("regen", "Regen", "/s", 1, { line: "" }),
    per("regenTime", "Refills in", "hp", "regen", "s", -1, { onLine: false }),
    power,
    raw("minPower", "Min power", "pips", -1, { onLine: false }),
    // At full power, as the shops list it (spviewer's "EM Max").
    raw("em", "EM max", "", -1, { onLine: false }),
    raw("health", "Item health", "", 1, { onLine: false }),
    ...distortion,
  ],
  PowerPlant: [raw("output", "Output", "pips", 1), per("emPip", "EM per pip", "em", "output", "", -1, { digits: 0 }), ...distortion],
  Cooler: [raw("coolant", "Coolant", "/s", 1, { digits: 1 }), per("coolPip", "Per pip", "coolant", "power", "/s", 1), power, ...distortion],
  Radar: [raw("aim", "Aim assist", "m", 1), per("aimPip", "Per pip", "aim", "power", "m", 1, { digits: 0 }), power, ...distortion],
  QuantumDrive: [
    raw("qspeed", "Speed", "km/s", 1),
    raw("spool", "Spool", "s", -1, { digits: 1 }),
    // After a jump that reached full speed; a shorter jump waits less.
    raw("cooldown", "Max cooldown", "s", -1, { digits: 1, onLine: false }),
    power,
    ...distortion,
  ],
  LifeSupportGenerator: [power, ...distortion],
  Missile: [
    raw("tracking", "Tracking", "", 0, { line: "", text: (v) => SIGNALS[v] ?? "—" }),
    raw("damage", "Damage", "", 1, { hint: "For a cluster missile: the warhead plus every submunition, all hitting" }),
    raw("cluster", "Cluster", "", 0, { onLine: false, hint: "Submunitions a cluster missile releases" }),
    raw("lockRange", "Lock range", "m", 1, { line: "lock" }),
    raw("speed", "Speed", "m/s", 1, { line: "" }),
    raw("lockTime", "Lock time", "s", -1, { digits: 2, onLine: false }),
    raw("lockAngle", "Lock angle", "°", 1, { onLine: false }),
    raw("lockMin", "Lock min", "m", -1, { onLine: false }),
    raw("arm", "Arms after", "s", -1, { digits: 2, onLine: false }),
  ],
  WeaponDefensive: [raw("ammo", "Ammo", "", 1)],
  FuelTank: [raw("capacity", "Capacity", "SCU", 1, { digits: 1 })],
  QuantumFuelTank: [raw("capacity", "Capacity", "SCU", 1, { digits: 1 })],
};

/** The figures for a type; parts without a list compare by power. */
export const figuresFor = (itemType: string): Figure[] => FIGURES[itemType] ?? [power];

/** A figure's value as shown, with its unit ("1 pip", not "1 pips"). */
export function show(f: Figure, v: number): string {
  if (f.text) return f.text(v);
  const shown = v.toLocaleString("en-US", { maximumFractionDigits: f.digits });
  const unit = f.unit === "pips" && shown === "1" ? "pip" : f.unit;
  return `${shown}${unit ? ` ${unit}` : ""}`;
}

/** A gun's damage type, by the larger part of its damage per shot. */
export function damageType(stats: StatView[]): string {
  const split = [
    ["Ballistic", value(stats, "dmgPhysical") ?? 0],
    ["Energy", value(stats, "dmgEnergy") ?? 0],
    ["Distortion", value(stats, "dmgDistortion") ?? 0],
  ] as const;
  const top = split.reduce((a, b) => (b[1] > a[1] ? b : a));
  return top[1] > 0 ? top[0] : "";
}

/** A part's line: its line figures past the lead one (the card already
 *  shows that). A gun with rounds adds its magazine and penetration; one
 *  with a capacitor has neither. */
export function lineFigures(itemType: string, stats: StatView[], lead: string | null): { label: string; text: string }[] {
  const ballistic = itemType === "WeaponGun" && value(stats, "ammo") !== null;
  return figuresFor(itemType)
    .filter((f) => f.onLine && f.key !== lead)
    .filter((f) => itemType !== "WeaponGun" || ballistic || (f.key !== "pen" && f.key !== "magazine"))
    .map((f) => ({ f, v: f.get(stats) }))
    .filter((x): x is { f: Figure; v: number } => x.v !== null)
    .map(({ f, v }) => ({ label: f.line, text: show(f, v) }));
}

/** Shield resistance where the generators' power puts it now, per damage
 *  type; a type the shield doesn't resist at any power is left out. */
export function shieldResist(sh: ShieldView | null): [string, number][] {
  const r = sh?.resistance;
  if (!sh || !r) return [];
  const types = [
    ["physical", "Physical"],
    ["energy", "Energy"],
    ["distortion", "Distortion"],
  ] as const;
  return types
    .filter(([k]) => r[k][0] !== 0 || r[k][1] !== 0)
    .map(([k, label]) => [label, r[k][0] + (r[k][1] - r[k][0]) * sh.ratio]);
}
