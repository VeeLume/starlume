import { describe, expect, it } from "vitest";
import type { ItemKindView, LoadoutSlotView, OwnedItemView, PlaceView, ShipView } from "$lib/bindings";
import { armourSets, categoryOf, isFull, isKitted, kitOf, shortPartName, subcategoryOf } from "./classify";
import { avgIngredientQuality, statDeltas } from "./crafted";
import { loadoutOf, offSaved, portLabel, portRows } from "./loadout";
import { amountOf, fmtAmount, materials, qualityMix } from "./resources";
import { buildTree, container, isFolder, pathLabel, stowed, units, type ItemNode, type Node } from "./tree";

// ── Fixtures ────────────────────────────────────────────────────────────────

let next = 1;
const kind = (name: string, item_type: string, sub_type = "UNDEFINED", guid = name): ItemKindView => ({
  guid,
  name,
  item_type,
  sub_type,
  size: 1,
  grade: 1,
});
const NB = { name: "New Babbage", parents: ["microTech", "Stanton"], key: "nb" };
const storage = (p = NB): PlaceView => ({ ...p, kind: "storage" });
const hangar = (p = NB): PlaceView => ({ ...p, kind: "hangar" });
const ON_YOU: PlaceView = { name: "On you", parents: [], kind: "carried", key: "" };

function item(k: ItemKindView, o: Partial<OwnedItemView> = {}): OwnedItemView {
  return {
    geid: String(next++),
    kind: k,
    place: storage(),
    holders: [],
    fitted: false,
    ownership: "owned",
    stack: 1,
    ammo: null,
    resource: null,
    crafted: null,
    stock: null,
    set: null,
    ...o,
  };
}
const holder = (i: OwnedItemView | ShipView) => ({
  geid: i.geid!,
  name: i.kind.name,
  item_type: i.kind.item_type,
});
function ship(name: string, o: Partial<ShipView> = {}): ShipView {
  return {
    key: `urn:${name}`,
    kind: kind(name, "NOITEM_Vehicle"),
    geid: String(next++),
    place: hangar(),
    holders: [],
    ownership: "owned",
    source: "pledge",
    rental_expiry: null,
    fitted: [],
    stock: null,
    saved: null,
    ...o,
  };
}
const slot = (path: string[], k: ItemKindView | null): LoadoutSlotView => ({ path, item: k });
const node = (t: ReturnType<typeof buildTree>, i: OwnedItemView | ShipView) =>
  t.byId.get("geid" in i && "key" in i ? `ship:${(i as ShipView).key}` : `item:${i.geid}`) as Node;

// ── Tree ────────────────────────────────────────────────────────────────────

describe("tree", () => {
  it("folds storage and hangar at one place into one location, hangar first", () => {
    const a = item(kind("Rifle", "WeaponPersonal"));
    const s = ship("Golem");
    const t = buildTree([a], [s]);
    const loc = node(t, a).parent!.parent!;
    expect(loc.name).toBe("New Babbage");
    expect(loc.kids.map((k) => k.name)).toEqual(["Personal hangar", "Storage"]);
    // One system: its planets sit straight under Everywhere.
    expect(pathLabel(node(t, a))).toBe("microTech › New Babbage › Storage › Rifle");
    expect(t.root.kids.map((k) => k.name)).toEqual(["microTech"]);
  });

  it("puts what was left in space under its planet, after the locations", () => {
    const area18 = { name: "Area18", parents: ["ArcCorp", "Stanton"], key: "a18" };
    const left = item(kind("Crate", "Misc"), { place: { name: "ArcCorp", parents: ["Stanton"], kind: "adrift", key: "arc" } });
    const there = item(kind("Rifle", "WeaponPersonal"), { place: storage(area18) });
    const t = buildTree([left, there], []);
    const arc = t.root.kids.find((k) => k.name === "ArcCorp")!;
    expect(arc.kids.map((k) => k.name)).toEqual(["Area18", "Left in space"]);
  });

  it("hangs items under their nearest holder, and carried items under On you", () => {
    const s = ship("Golem");
    const locker = item(kind("Locker", "InventoryContainer"), { place: hangar(), holders: [holder(s)] });
    const gun = item(kind("Pistol", "WeaponPersonal"), { place: hangar(), holders: [holder(s), holder(locker)] });
    const scope = item(kind("Scope", "WeaponAttachment", "IronSight"), {
      place: hangar(),
      holders: [holder(s), holder(locker), holder(gun)],
      fitted: true,
    });
    const pack = item(kind("Backpack", "Char_Armor_Backpack"), { place: ON_YOU });
    const t = buildTree([scope, gun, locker, pack], [s]);
    expect(node(t, scope).parent).toBe(node(t, gun));
    expect(isFolder(node(t, gun))).toBe(false); // a rifle with a scope is no folder
    expect(container(node(t, scope))).toBe(node(t, locker));
    expect(stowed(node(t, s), () => true).map((n) => n.name).sort()).toEqual(["Locker", "Pistol"]);
    expect(t.root.kids[0].name).toBe("On you");
    expect(units(t.root, () => true)).toBe(4);
  });

  it("files ships outside every stash under the virtual groups, bricked ones nowhere", () => {
    const away = ship("Hornet", { place: null });
    const never = ship("Cutlass", { place: null, geid: null });
    const gone = ship("Aurora", { place: null, ownership: "bricked" });
    const t = buildTree([], [away, never, gone]);
    expect(t.away.kids.map((k) => k.name)).toEqual(["Hornet"]);
    expect(t.unspawned.kids.map((k) => k.name)).toEqual(["Cutlass"]);
  });
});

// ── Classification ─────────────────────────────────────────────────────────

describe("classify", () => {
  const n = (i: OwnedItemView) => buildTree([i], []).items[0];

  it("files tools the game calls weapons under Utility", () => {
    expect(categoryOf(n(item(kind("ParaMed", "WeaponPersonal", "Small"))))).toBe("Utility");
    const pistol = n(item(kind("Arclight Pistol", "WeaponPersonal", "Small")));
    expect(categoryOf(pistol)).toBe("Weapons");
    expect(subcategoryOf(pistol, "Weapons")).toBe("Pistols");
  });

  it("folds armour into sets with their weight and owned pieces", () => {
    const pieces = ["Helmet", "Core", "Arms", "Legs"].map((p, i) =>
      item(kind(`ADP Armor ${p} Black`, ["Char_Armor_Helmet", "Char_Armor_Torso", "Char_Armor_Arms", "Char_Armor_Legs"][i], "Heavy")),
    );
    const sets = armourSets(buildTree(pieces, []).items);
    expect(sets).toHaveLength(1);
    expect(sets[0].name).toBe("ADP Black");
    expect(sets[0].weight).toBe("Heavy");
    expect(sets[0].got).toBe(4);
  });

  it("groups by the game data's set, which says what makes it full", () => {
    const set = { key: "vgl_flightsuit_05_01_01:-", name: "Tailwind Flight Kruger", pieces: ["Helmet", "Undersuit"] };
    const sets = armourSets(
      buildTree(
        [
          item(kind("Tailwind Flight Helmet Kruger", "Char_Armor_Helmet", "Helmet"), { set }),
          item(kind("Tailwind Flight Suit Kruger", "Char_Armor_Undersuit"), { set }),
        ],
        [],
      ).items,
    );
    expect(sets).toHaveLength(1);
    expect(sets[0].name).toBe("Tailwind Flight Kruger");
    expect(sets[0].full).toEqual(["Helmet", "Undersuit"]);
    expect(isFull(sets[0])).toBe(true);
  });

  it("marks a weapon's attachments stock or added against its own stock loadout", () => {
    const mag = kind("P4-AR Magazine", "WeaponAttachment", "Magazine");
    const scope = kind("Myers (4x Telescopic)", "WeaponAttachment", "IronSight");
    const rifle = item(kind("P4-AR", "WeaponPersonal"), { stock: [slot(["magazine_attach"], mag)] });
    const parts = [mag, scope].map((k) => item(k, { holders: [holder(rifle)], fitted: true }));
    const t = buildTree([rifle, ...parts], []);
    const kit = kitOf(node(t, rifle));
    expect(kit.map((k) => [k.slot, k.state])).toEqual([
      ["MAG", "stock"],
      ["OPT", "added"],
    ]);
    expect(isKitted(node(t, rifle))).toBe(true);
    expect(shortPartName(scope.name)).toBe("Myers 4x");
  });

  it("leaves a looted weapon's kit as fitted (stock unknown)", () => {
    const rifle = item(kind("Looted", "WeaponPersonal"));
    const mag = item(kind("Mag", "WeaponAttachment", "Magazine"), { holders: [holder(rifle)], fitted: true });
    const t = buildTree([rifle, mag], []);
    expect(kitOf(node(t, rifle))[0].state).toBe("fitted");
    expect(isKitted(node(t, rifle))).toBe(false); // a magazine alone isn't a kit
  });
});

// ── Resources ───────────────────────────────────────────────────────────────

describe("resources", () => {
  const res = (name: string, type: string, quality: number, scu: number, stack: number, place = storage()) =>
    item(kind(name, type, type === "Cargo" ? "Cargo" : "Harvestable"), {
      resource: { name, quality, scu },
      stack,
      place,
    });

  it("measures bulk in SCU and pieces by count, never mixed", () => {
    const t = buildTree([res("Iron", "Cargo", 325, 4, 2), res("Glacosite", "Misc", 567, 0.001, 4)], []);
    const [iron, glac] = t.items as ItemNode[];
    expect(amountOf(iron)).toBe(8);
    expect(amountOf(glac)).toBe(4);
    expect(fmtAmount(true, 0.027)).toBe("0.03 SCU");
    expect(fmtAmount(true, 0.004)).toBe("<0.01 SCU");
    expect(fmtAmount(false, 1)).toBe("1 pc");
  });

  it("folds stacks per material with best quality, Q500+ amount and places", () => {
    const baijini = { name: "Baijini Point", parents: ["ArcCorp", "Stanton"], key: "bp" };
    const t = buildTree(
      [res("Glacosite", "Misc", 972, 0.001, 2), res("Glacosite", "Misc", 410, 0.001, 3, storage(baijini))],
      [],
    );
    const [m] = materials(t.items);
    expect(m.total).toBe(5);
    expect(m.best).toBe(972);
    expect(m.good).toBe(2);
    expect([...m.places].sort()).toEqual(["Baijini Point", "New Babbage"]);
    expect(materials(t.items, 500)[0].total).toBe(2);
  });

  it("gives the heat strip each band's share of the material", () => {
    const t = buildTree([res("Iron", "Cargo", 325, 3, 1), res("Iron", "Cargo", 874, 1, 1)], []);
    const mix = qualityMix(t.items);
    expect(mix[3].share).toBeCloseTo(0.75);
    expect(mix[8].share).toBeCloseTo(0.25);
    expect(mix[0].amount).toBe(0);
  });
});

// ── Crafted ─────────────────────────────────────────────────────────────────

describe("crafted", () => {
  it("folds the recoil trio and the temperature pair, and judges each roll", () => {
    const d = statDeltas({
      ingredients: [],
      properties: [
        { crc: 1, name: "Recoil Kick", factor: 0.7, factor_up_is_better: false },
        { crc: 2, name: "Recoil Handling", factor: 0.7, factor_up_is_better: false },
        { crc: 3, name: "Impact Force", factor: 1.148, factor_up_is_better: true },
        { crc: 4, name: "Min Temp", factor: 0.924, factor_up_is_better: true },
        { crc: 5, name: "Max Temp", factor: 0.924, factor_up_is_better: true },
        { crc: 6, name: "Reload Speed", factor: 1.1, factor_up_is_better: null },
      ],
    });
    expect(d).toEqual([
      { name: "Recoil", pct: -30, good: 1 },
      { name: "Impact Force", pct: 14.8, good: 1 },
      { name: "Temp. range", pct: -7.6, good: -1 },
      { name: "Reload Speed", pct: 10, good: 0 },
    ]);
  });

  it("averages resource ingredient quality by amount, leaving item counts out", () => {
    const q = avgIngredientQuality({
      ingredients: [
        { kind: "resource", name: "Taranite", quality: 800, amount: 60000 },
        { kind: "resource", name: "Iron", quality: 400, amount: 20000 },
        { kind: "item", name: "Hadanite", quality: 100, amount: 1 },
      ],
      properties: [],
    });
    expect(q).toBe(700);
  });
});

// ── Loadout ─────────────────────────────────────────────────────────────────

describe("loadout", () => {
  const gun = kind("FR-76", "WeaponGun");
  const other = kind("CF-337", "WeaponGun");
  const gimbal = kind("VariPuck", "Turret");
  const missile = kind("Ignite II", "Missile");

  it("compares fitted with saved per port; missiles are unknown, not missing", () => {
    const s = ship("Hornet", {
      stock: [slot(["hardpoint_weapon_left"], gun), slot(["hardpoint_missile"], missile)],
      saved: [slot(["hardpoint_weapon_left"], other), slot(["hardpoint_missile"], missile)],
      fitted: [{ geid: "9", port: "hardpoint_weapon_left", depth: 0, kind: gun }],
    });
    const rows = portRows(s)!;
    expect(rows.map((r) => [r.status, r.custom])).toEqual([
      ["off", true],
      ["unknown", false],
    ]);
    expect(offSaved(s)).toBe(1);
  });

  it("measures against stock when no saved loadout is recorded", () => {
    const s = ship("Hornet", {
      stock: [slot(["hardpoint_weapon_left"], gun)],
      fitted: [{ geid: "9", port: "hardpoint_weapon_left", depth: 0, kind: gun }],
    });
    expect(portRows(s)![0].status).toBe("ok");
  });

  it("puts a gimbal above its gun and folds identical hardpoints", () => {
    const hp = (side: string) => [slot([side], gimbal), slot([side, "hardpoint_class_2"], gun)];
    const s = ship("Hornet", {
      stock: [...hp("hardpoint_left"), ...hp("hardpoint_right")],
      fitted: ["hardpoint_left", "hardpoint_right"].flatMap((p) => [
        { geid: `${p}g`, port: p, depth: 0, kind: gimbal },
        { geid: `${p}w`, port: "hardpoint_class_2", depth: 1, kind: gun },
      ]),
    });
    const l = loadoutOf(s);
    expect(l.hardpoints).toHaveLength(1);
    expect(l.hardpoints[0]).toMatchObject({ section: "Weapons", n: 2 });
    expect(l.hardpoints[0].mount?.name).toBe("VariPuck");
    expect(l.off).toBe(0);
  });

  it("labels ports readably", () => {
    expect(portLabel(["hardpoint_mining_arm", "hardpoint_mining_laser"])).toBe("Mining arm › mining laser");
  });
});
