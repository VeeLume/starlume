// What would do better than the loadout as it is: the solve's gadget rows,
// module kits, crew ships and a support Prospector, as one list of
// changes the calculator can apply.

import type { EvalView, LoadoutView, MiningCatalogView, MiningReportView } from "$lib/bindings";
import { addSupport, setLoadout } from "$lib/state/mining.svelte";
import { laserShort, namesOf } from "./format";

export interface Alternative {
  key: string;
  group: "Gadgets" | "Modules" | "Crew" | "Support";
  /** Gadget, module kit or crew mate. */
  name: string;
  /** What it is ("gadget", "Prospector with Arbor MH1"). */
  kind: string;
  /** A crew mate's name, for the monogram. */
  who: string | null;
  eval: EvalView;
  verb: "Use" | "Add";
  apply: () => Promise<void>;
}

const score = (e: EvalView) => e.difficulty?.controlled ?? -1;
const time = (e: EvalView) => e.difficulty?.medianTimeS ?? Infinity;

/** Clean cracks first, then the faster one. */
export function byOutcome(a: Alternative, b: Alternative): number {
  return score(b.eval) - score(a.eval) || time(a.eval) - time(b.eval) || b.eval.margin - a.eval.margin;
}

/** More clean cracks, or as many in clearly less time. */
export function improves(e: EvalView, yours: EvalView | null): boolean {
  if (!yours) return score(e) >= 0;
  const [a, b] = [score(e), score(yours)];
  return a > b + 0.02 || (a >= b && b >= 0 && time(e) < time(yours) * 0.8);
}

export function alternatives(report: MiningReportView, loadout: LoadoutView, catalog: MiningCatalogView): Alternative[] {
  const names = namesOf(catalog);
  const name = (id: string) => names.get(id) ?? "Unknown";
  const kit = (ids: string[]) => (ids.length ? ids.map(name).join(", ") : "No modules");
  const same = (a: string[], b: string[]) => [...a].sort().join() === [...b].sort().join();
  const out: Alternative[] = [];

  for (const g of report.gadgets) {
    if (g.gadget === loadout.gadget) continue;
    out.push({
      key: `g:${g.gadget}`,
      group: "Gadgets",
      name: g.gadget ? name(g.gadget) : "No gadget",
      kind: "gadget",
      who: null,
      eval: g.eval,
      verb: "Use",
      apply: () => setLoadout({ ...loadout, gadget: g.gadget }),
    });
  }
  const first = loadout.lasers[0];
  for (const [rows, active] of [
    [report.passiveKits, false],
    [report.activeKits, true],
  ] as const) {
    for (const k of rows) {
      if (!first || same(k.modules, first.modules)) continue;
      out.push({
        key: `k:${k.modules.join()}`,
        group: "Modules",
        name: kit(k.modules),
        kind: active
          ? k.runsOutS !== null
            ? `modules, active, runs out after ${Math.round(k.runsOutS)} s`
            : "modules, active"
          : "modules",
        who: null,
        eval: k.eval,
        verb: "Use",
        apply: () =>
          setLoadout({ ...loadout, lasers: loadout.lasers.map((l, i) => (i === 0 ? { ...l, modules: k.modules } : l)) }),
      });
    }
  }
  const aboard = new Set(loadout.lasers.flatMap((l) => (l.crew === null ? [] : [l.crew])));
  for (const c of report.crew) {
    if (aboard.has(c.id)) continue;
    out.push({
      key: `c:${c.id}`,
      group: "Crew",
      name: c.owner,
      kind: c.name,
      who: c.owner,
      eval: c.eval,
      verb: "Add",
      apply: () => addSupport(c.id),
    });
  }
  const ship = Math.max(-1, ...loadout.lasers.map((l) => l.owner)) + 1;
  for (const s of report.support) {
    out.push({
      key: `s:${s.laser}:${s.modules.join()}`,
      group: "Support",
      name: "Prospector",
      kind: `with ${laserShort(name(s.laser))}${s.modules.length ? `, ${kit(s.modules)}` : ""}`,
      who: null,
      eval: s.eval,
      verb: "Add",
      apply: () =>
        setLoadout({
          ...loadout,
          lasers: [...loadout.lasers, { laser: s.laser, modules: s.modules, owner: ship, who: "Prospector", crew: `support:${s.laser}` }],
        }),
    });
  }
  return out;
}
