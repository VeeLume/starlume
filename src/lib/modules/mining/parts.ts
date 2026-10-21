// A loadout's parts, edited one at a time: the part picker's and the
// sweep's options, and how a try-out reads against the saved loadout.

import type { LaserInputView, MiningCatalogView } from "$lib/bindings";
import { laserShort } from "./format";

/** A picker target: `L<head>` (the laser) or `S<head>:<slot>` (a module). */
export function target(key: string): { laser: boolean; head: number; slot: number } {
  const laser = key.startsWith("L");
  return { laser, head: Number(key.slice(1).split(":")[0]), slot: laser ? -1 : Number(key.split(":")[1]) };
}

/** The heads with one part swapped: a laser (keeping the modules it has
 *  slots for) or a module (`null`: the slot emptied). */
export function withPart(
  catalog: MiningCatalogView,
  lasers: LaserInputView[],
  key: string,
  id: string | null,
): LaserInputView[] {
  const t = target(key);
  const current = lasers[t.head] ?? null;
  if (t.laser) {
    const slots = catalog.lasers.find((l) => l.id === id)?.moduleSlots ?? 0;
    const next = {
      ...(current ?? { modules: [], owner: lasers[0]?.owner ?? 0, who: null, crew: null, distanceM: null, off: false }),
      laser: id ?? "",
    };
    next.modules = next.modules.slice(0, slots);
    return current ? lasers.map((l, i) => (i === t.head ? next : l)) : [...lasers, next];
  }
  return lasers.map((l, i) => {
    if (i !== t.head) return l;
    const modules = [...l.modules];
    if (id) modules[t.slot] = id;
    else modules.splice(t.slot, 1);
    return { ...l, modules: modules.filter(Boolean) };
  });
}

const moduleShort = (name: string) => name.replace(/\s+Module$/i, "");

/** What a try-out changed against the saved loadout, in a few words:
 *  "laser → Klein-S1", "slot 2 → empty", "3 changes". */
export function changeLabel(
  catalog: MiningCatalogView,
  saved: LaserInputView[],
  now: LaserInputView[],
): string {
  const name = (id: string | undefined, laser: boolean) =>
    !id
      ? "empty"
      : laser
        ? laserShort(catalog.lasers.find((l) => l.id === id)?.name ?? "?")
        : moduleShort(catalog.modules.find((m) => m.id === id)?.name ?? "?");
  const multi = Math.max(saved.length, now.length) > 1;
  const out: string[] = [];
  for (let i = 0; i < Math.max(saved.length, now.length); i++) {
    const a = saved[i];
    const b = now[i];
    const head = multi ? `head ${i + 1} ` : "";
    if (!b) {
      out.push(`${head}removed`);
      continue;
    }
    if (!a) {
      out.push(`${head}added`);
      continue;
    }
    if (a.laser !== b.laser) out.push(`${head}laser → ${name(b.laser, true)}`);
    for (let s = 0; s < Math.max(a.modules.length, b.modules.length); s++)
      if ((a.modules[s] ?? null) !== (b.modules[s] ?? null) && a.laser === b.laser)
        out.push(`${head}slot ${s + 1} → ${name(b.modules[s], false)}`);
  }
  if (!out.length) return "as saved";
  return out.length > 2 ? `${out.length} changes` : out.join(", ");
}
