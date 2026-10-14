// My Items wording: how the finder's wire values read on screen.

import type { HolderView, PlaceView } from "$lib/bindings";

/** Item-type groups for the Type facet — the game's ~50 item types folded
 *  into what a player looks for. Order is the facet's order. */
const GROUPS: [string, (t: string) => boolean][] = [
  ["Personal weapons", (t) => t === "WeaponPersonal"],
  ["Attachments & ammo", (t) => ["WeaponAttachment", "MiningModifier", "RemovableChip"].includes(t)],
  ["Armour", (t) => t.startsWith("Char_Armor")],
  ["Clothing", (t) => t.startsWith("Char_Clothing")],
  [
    "Ship weapons",
    (t) =>
      [
        "WeaponGun",
        "Turret",
        "UtilityTurret",
        "MissileLauncher",
        "Missile",
        "WeaponMining",
        "TractorBeam",
        "ToolArm",
      ].includes(t),
  ],
  [
    "Ship components",
    (t) =>
      [
        "PowerPlant",
        "Cooler",
        "Shield",
        "QuantumDrive",
        "JumpDrive",
        "Radar",
        "LifeSupportGenerator",
        "FlightController",
      ].includes(t),
  ],
  ["Consumables", (t) => ["FPS_Consumable", "Food", "Drink"].includes(t)],
  ["Cargo & containers", (t) => ["Cargo", "Container", "InventoryContainer"].includes(t)],
  ["Liveries", (t) => t === "Paints"],
  ["Decor", (t) => t.startsWith("Flair_") || t === "Char_Flair"],
];

export const TYPE_GROUPS = [...GROUPS.map(([g]) => g), "Other"];

export function typeGroup(itemType: string): string {
  return GROUPS.find(([, test]) => test(itemType))?.[0] ?? "Other";
}

/** "Lorville (Hurston, Stanton)". */
export function placeLabel(p: PlaceView): string {
  return p.parents.length > 0 ? `${p.name} (${p.parents.join(", ")})` : p.name;
}

/** "Lorville · RSI Hermes › VariPuck S3 Gimbal Mount". */
export function whereLabel(place: PlaceView, holders: HolderView[]): string {
  return holders.length > 0
    ? `${place.name} · ${holders.map((h) => h.name).join(" › ")}`
    : place.name;
}

export const OWNERSHIP_LABEL: Record<string, string> = {
  owned: "Owned",
  rented: "Rented",
  unowned: "Not imprinted",
  bricked: "Bricked",
};

export const SOURCE_LABEL: Record<string, string> = {
  pledge: "Pledge",
  in_game: "In-game",
  rental: "Rental",
  unknown: "Unknown source",
};

/** "hardpoint_weapon_left" → "weapon left"; "$IP_battery" → "battery". */
export function portLabel(port: string): string {
  return port
    .replace(/^\$IP_/, "")
    .replace(/^hardpoint_/i, "")
    .replace(/_/g, " ");
}
