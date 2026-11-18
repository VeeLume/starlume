import { describe, expect, it } from "vitest";
import type { MissionEntryView, RepRequirementView } from "$lib/bindings";
import {
  blueprintCell,
  blueprintChance,
  ladderOf,
  needsNote,
  oddsText,
  ownRequirement,
  paysCell,
  runsFor,
  timeCell,
  whereCell,
  windowText,
} from "./model";

// The Headhunters ladder on LIVE 4.10.3 (FactionReputationScope).
const HH = [
  ["Neutral", 0],
  ["Jr. Contractor", 800],
  ["Contractor", 2200],
  ["Sr. Contractor", 5800],
  ["Veteran Contractor", 15000],
  ["Head Contractor", 38000],
  ["Elite Contractor", 95250],
].map(([name, min_reputation]) => ({ name: name as string, min_reputation: min_reputation as number }));

const req = (faction: string, min: string | null, max: string | null, minIdx: number | null = null): RepRequirementView => ({
  faction,
  min_rank: min,
  max_rank: max,
  min_rank_index: minIdx,
  max_rank_index: null,
  exclude: false,
  scope: "d31bd373-35ab-4ea4-b0a7-0ccc867ef082",
  ladder: HH,
});

// Simple Hit in Bloom, as cooked (trimmed to what the row reads).
function simpleHit(over: Partial<MissionEntryView> = {}): MissionEntryView {
  return {
    mission_id: "m1",
    title: "Simple Hit",
    debug_name: "HH_Pyro_RegionB_LRT_Criminals_KillShip",
    description: null,
    category: { name: "Bounty Hunter", icon: "" },
    faction: { guid: "hh", name: "Headhunters" },
    difficulty: null,
    payout: { calculated: true, fixed: null, estimate: 58000, buy_in: 0, time_to_complete: 14 },
    once_only: false,
    shareable: false,
    illegal: true,
    cooldown_seconds: null,
    abandon_cooldown_seconds: null,
    scrip: [],
    reputation: [{ faction_guid: "hh", amount: 75 }],
    item_rewards: [],
    blueprint_rewards: [
      {
        pool_name: "BP_REWARDS_HeadHuntersMercenaryShipRegionB",
        chance: 1,
        blueprints: ["HeatSink", "Dominance-3", "Dominance-2", "HeatSafe", "Dominance-1"].map((n, i) => ({
          blueprint_record_guid: `bp${i}`,
          name: n,
          weight: 1,
        })),
      },
    ],
    event_rewards: [],
    rep_required: [req("Headhunters", "Jr. Contractor", "Elite Contractor", 1)],
    chain_required: [],
    locations: [{ system: "Pyro", name: "Bloom", places: Array.from({ length: 21 }, (_, i) => ({ name: `P${i}`, record_name: `p${i}`, kind: null })) }],
    encounters: [],
    cargo: [],
    placeholders: [],
    instance_count: 1,
    facts: {} as MissionEntryView["facts"],
    ...over,
  } as MissionEntryView;
}

describe("the rank window", () => {
  it("reads the window off the ladder by name", () => {
    const l = ladderOf(req("Headhunters", "Jr. Contractor", "Elite Contractor"))!;
    expect([l.lo, l.hi]).toEqual([1, 6]);
    expect(windowText(l)).toBe("Jr. Contractor or higher");
  });
  it("names a one-rank window and an open one", () => {
    expect(windowText(ladderOf(req("Headhunters", "Neutral", "Neutral"))!)).toBe("Neutral only");
    expect(windowText(ladderOf(req("Headhunters", "Neutral", null))!)).toBe("Any rank");
    expect(windowText(ladderOf(req("Headhunters", "Contractor", "Veteran Contractor"))!)).toBe("Contractor to Veteran Contractor");
  });
  it("has no ladder without a scope", () => {
    expect(ladderOf({ ...req("X", "Neutral", null), ladder: [] })).toBeNull();
  });
  it("takes the strictest of two requirements on the own faction", () => {
    const m = simpleHit({ rep_required: [req("Headhunters", "Neutral", null, 0), req("Headhunters", "Contractor", null, 2)] });
    expect(ownRequirement(m)?.min_rank).toBe("Contractor");
  });
});

describe("what a mission needs first", () => {
  it("names another faction's rank", () => {
    const m = simpleHit({ rep_required: [req("Headhunters", "Jr. Contractor", null, 1), req("Citizens For Prosperity", "Neutral", "Exalted")] });
    expect(needsNote(m)).toBe("needs Citizens For Prosperity Neutral");
  });
  it("names the mission to finish first", () => {
    const chain = [{ mission_id: "a", title: "Setting a Trap", once_only: false }];
    expect(needsNote(simpleHit({ chain_required: chain }))).toBe("after Setting a Trap");
    expect(needsNote(simpleHit({ chain_required: [...chain, { mission_id: "b", title: "X", once_only: false }] }))).toBe(
      "after Setting a Trap and 1 more",
    );
  });
  it("names a faction rank that closes the mission", () => {
    // Hit a CFP Ship: closed to anyone CFP likes, from Neutral to Exalted.
    const cfp = { ...req("Citizens For Prosperity", "Neutral", "Exalted"), exclude: true, ladder: [] };
    expect(needsNote(simpleHit({ rep_required: [cfp, req("Headhunters", "Sr. Contractor", null, 3)] }))).toBe(
      "closed at Citizens For Prosperity Neutral to Exalted",
    );
  });
  it("says nothing when the own rank is all", () => {
    expect(needsNote(simpleHit())).toBeNull();
  });
});

describe("the row's cells", () => {
  it("places, pay and time", () => {
    expect(whereCell(simpleHit())).toEqual({ value: "Bloom", sub: "Pyro, 21 places" });
    expect(paysCell(simpleHit())).toEqual({ value: "58,000", sub: "+75 rep" });
    expect(timeCell(simpleHit())).toEqual({ value: "14 min", sub: "game estimate" });
  });
  it("blueprints: new to you once the owned set is known", () => {
    const owned = new Set(["bp2", "bp3"]);
    expect(blueprintCell(simpleHit(), (g) => owned.has(g), true)).toEqual({ value: "5 in pool", sub: "3 new to you" });
    expect(blueprintCell(simpleHit(), () => false, false)).toEqual({ value: "5 in pool", sub: "1 per run" });
    expect(blueprintCell(simpleHit({ blueprint_rewards: [] }), () => false, true).value).toBe("none");
  });
});

describe("odds for one blueprint", () => {
  it("one in five per run, 11 runs for 90%", () => {
    const p = blueprintChance(simpleHit(), "bp0");
    expect(p).toBeCloseTo(0.2);
    expect(oddsText(p)).toBe("1 in 5");
    expect(runsFor(p)).toEqual({ average: 5, ninety: 11 });
  });
  it("the NightFall pool: 1 in 8, 18 runs for 90%", () => {
    expect(runsFor(1 / 8)).toEqual({ average: 8, ninety: 18 });
  });
  it("a blueprint not in the pools has no odds", () => {
    expect(blueprintChance(simpleHit(), "nope")).toBe(0);
    expect(runsFor(0)).toBeNull();
  });
});
