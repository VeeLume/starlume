import { describe, expect, it } from "vitest";
import type { MissionEntryView, RepRequirementView } from "$lib/bindings";
import {
  STANDING_GROUPS,
  blueprintCell,
  blueprintChance,
  commonLadder,
  factionSummaries,
  hasNewBlueprint,
  ladderOf,
  needsNote,
  oddsText,
  opensAt,
  ownRequirement,
  paysCell,
  rankIndex,
  rowFor,
  runsFor,
  standingOf,
  stepsLeft,
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
].map(([name, min_reputation], i) => ({ id: `hh${i}`, name: name as string, min_reputation: min_reputation as number }));

const req = (faction: string, min: string | null, max: string | null, minIdx: number | null = null): RepRequirementView => ({
  faction,
  min_rank: min,
  max_rank: max,
  min_rank_index: minIdx,
  max_rank_index: null,
  exclude: false,
  scope: "d31bd373-35ab-4ea4-b0a7-0ccc867ef082",
  ladder: HH,
  faction_geid: faction === "Headhunters" ? "100" : "200",
});

// Simple Hit in Bloom, as cooked (trimmed to what the row reads).
function simpleHit(over: Partial<MissionEntryView> = {}): MissionEntryView {
  return {
    mission_id: "m1",
    title: "Simple Hit",
    debug_name: "HH_Pyro_RegionB_LRT_Criminals_KillShip",
    description: null,
    category: { name: "Bounty Hunter", icon: "" },
    faction: { guid: "hh", name: "Headhunters", geid: "100" },
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
    contracts: ["HH_Pyro_RegionB_LRT_Criminals_KillShip"],
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
    const chain = [{ mission_id: "a", title: "Setting a Trap", once_only: false, contracts: ["Trap_1"] }];
    expect(needsNote(simpleHit({ chain_required: chain }))).toBe("after Setting a Trap");
    expect(needsNote(simpleHit({ chain_required: [...chain, { mission_id: "b", title: "X", once_only: false, contracts: ["X_1"] }] }))).toBe(
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

describe("the rank filter", () => {
  const l = ladderOf(req("Headhunters", "Jr. Contractor", "Elite Contractor"))!;
  it("is open inside the window, by rank name", () => {
    expect(opensAt(l, "Jr. Contractor")).toBe(true);
    expect(opensAt(l, "Neutral")).toBe(false);
    expect(opensAt(l, "Junior")).toBe(false);
    expect(opensAt(null, "Neutral")).toBe(true);
  });
  it("offers the ladder most missions use", () => {
    const other = { ranks: ["Rookie", "Junior"], reps: [0, 10], ids: ["r0", "r1"], lo: 0, hi: 1 };
    expect(commonLadder([l, l, other, null])).toEqual(HH.map((r) => r.name));
  });
  it("finds pools with blueprints you lack", () => {
    expect(hasNewBlueprint(simpleHit(), (g) => g !== "bp4")).toBe(true);
    expect(hasNewBlueprint(simpleHit(), () => true)).toBe(false);
  });
});

describe("your standing", () => {
  const row = (score: number, standing: string | null, geid = "100") => ({
    org_geid: geid,
    scope: "FactionReputation",
    scope_id: "d31bd373-35ab-4ea4-b0a7-0ccc867ef082",
    score,
    standing_id: standing,
    standing_min: null,
  });
  const none = new Set<string>();

  it("finds your rank by standing id, else by score", () => {
    const l = ladderOf(req("Headhunters", "Jr. Contractor", null, 1))!;
    expect(rankIndex(l, row(1000, "hh1"))).toBe(1);
    expect(rankIndex(l, row(2500, null))).toBe(2);
    expect(rowFor(req("Headhunters", "Neutral", null), [row(1, null, "999")])).toBeNull();
  });
  it("sorts a mission by what stands between you and it", () => {
    expect(standingOf(simpleHit(), [row(1000, "hh1")], none)).toBe("open");
    expect(standingOf(simpleHit(), [row(100, "hh0")], none)).toBe("higher");
    expect(standingOf(simpleHit({ rep_required: [req("Headhunters", "Neutral", "Neutral", 0)] }), [row(1000, "hh1")], none)).toBe(
      "outgrown",
    );
    // No row is not Neutral: unknown, never hidden.
    expect(standingOf(simpleHit(), [], none)).toBe("unknown");
  });
  it("an exclusion you sit in closes it; one you do not, does not", () => {
    const cfp = { ...req("Citizens For Prosperity", "Neutral", "Elite Contractor"), exclude: true };
    const m = simpleHit({ rep_required: [cfp, req("Headhunters", "Jr. Contractor", null, 1)] });
    expect(standingOf(m, [row(1000, "hh1"), row(0, "hh0", "200")], none)).toBe("closed");
    expect(standingOf(m, [row(1000, "hh1"), row(-500, null, "200")], none)).toBe("open");
  });
  it("chain steps and once-only contracts follow your completed contracts", () => {
    const chain = [{ mission_id: "a", title: "Setting a Trap", once_only: false, contracts: ["Trap_1", "Trap_2"] }];
    const m = simpleHit({ chain_required: chain });
    expect(standingOf(m, [row(1000, "hh1")], none)).toBe("step");
    expect(stepsLeft(m, new Set(["Trap_2"]))).toEqual([]);
    expect(standingOf(m, [row(1000, "hh1")], new Set(["Trap_2"]))).toBe("open");
    expect(standingOf(simpleHit({ once_only: true }), [row(1000, "hh1")], new Set(["HH_Pyro_RegionB_LRT_Criminals_KillShip"]))).toBe("done");
  });
  it("every status has a group", () => {
    expect(STANDING_GROUPS.map((g) => g.id).sort()).toEqual(["closed", "done", "higher", "open", "outgrown", "step", "unknown"]);
  });
});

describe("the Standing tab", () => {
  const row = { org_geid: "100", scope: "FactionReputation", scope_id: "d31bd373-35ab-4ea4-b0a7-0ccc867ef082", score: 1000, standing_id: "hh1", standing_min: 800 };
  it("sums a faction's missions per rank and places you", () => {
    const ms = [
      simpleHit(),
      simpleHit({ mission_id: "m2", rep_required: [req("Headhunters", "Neutral", null, 0)], payout: { calculated: true, fixed: null, estimate: 36250, buy_in: 0, time_to_complete: 10 } }),
      simpleHit({ mission_id: "m3", rep_required: [req("Headhunters", "Contractor", null, 2)], blueprint_rewards: [] }),
    ];
    const [hh] = factionSummaries(ms, [row]);
    expect(hh.name).toBe("Headhunters");
    expect(hh.you).toBe(1);
    expect(hh.score).toBe(1000);
    expect(hh.next).toBe(2200);
    expect(hh.missions).toBe(3);
    expect(hh.withBlueprint).toBe(2);
    expect(hh.open).toBe(2);
    expect(hh.perRank.slice(0, 3).map((p) => p.missions)).toEqual([1, 1, 1]);
    expect(hh.perRank[0].median).toBe(36250);
  });
  it("without a row, your place stays unknown", () => {
    const [hh] = factionSummaries([simpleHit()], []);
    expect([hh.you, hh.score, hh.open, hh.next]).toEqual([-1, null, null, null]);
  });
});
