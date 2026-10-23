// A ship's seats in words: what each crewed seat does (sc-vehicles reads it
// from the seats' control priorities, not their names).
import type { SeatsView } from "$lib/bindings";

const plural = (n: number, one: string, many: string) => (n === 1 ? one : `${n} ${many}`);

/** The crewed seats by role, most important first ("Pilot", "2 manned
 *  turrets"). Roles a seat shares count for each. */
export function crewParts(s: SeatsView): string[] {
  const out: string[] = [];
  if (s.pilot) out.push(plural(s.pilot, "Pilot", "pilots"));
  if (s.copilot) out.push(plural(s.copilot, "Copilot", "copilots"));
  if (s.mannedTurrets) out.push(plural(s.mannedTurrets, "Manned turret", "manned turrets"));
  if (s.remoteTurrets) out.push(plural(s.remoteTurrets, "Remote turret", "remote turrets"));
  if (s.mining) out.push(plural(s.mining, "Mining", "mining seats"));
  if (s.salvage) out.push(plural(s.salvage, "Salvage", "salvage seats"));
  return out;
}

/** One line for a tooltip. */
export function crewTitle(s: SeatsView): string {
  const parts = crewParts(s);
  const extra = s.passengers ? `, plus ${s.passengers} passenger ${s.passengers === 1 ? "seat" : "seats"}` : "";
  return `${s.crew} crewed ${s.crew === 1 ? "seat" : "seats"}${parts.length ? `: ${parts.join(", ")}` : ""}${extra}`;
}
