import type { ShipRowView } from "$lib/bindings";

/** Ground vehicles and hover bikes share one group, whoever makes them:
 *  the maker lists are for ships. */
export const GROUND = "Ground and hover";

/** The group a hull is listed under: its maker, or the ground group. */
export const groupOf = (s: ShipRowView): string => (s.ground || s.gravlev ? GROUND : (s.manufacturer ?? "Other"));

/** Makers A to Z, the ground group last. */
export const byGroup = (a: string, b: string): number => Number(a === GROUND) - Number(b === GROUND) || a.localeCompare(b);
