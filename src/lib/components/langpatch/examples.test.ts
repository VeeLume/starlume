import { describe, expect, it } from "vitest";
import type { PatcherInfoView } from "$lib/bindings";
import { patcherExample, patcherSummary } from "./examples";

const bool = (id: string) => ({ id, label: id, description: "", kind: { type: "Bool" as const }, default: "true" });
const choice = (id: string, values: string[], def: string) => ({
  id,
  label: id,
  description: "",
  kind: { type: "Choice" as const, choices: values.map((v) => ({ value: v, label: v.toUpperCase() })) },
  default: def,
});

function patcher(id: string, options: PatcherInfoView["options"], values: Record<string, string> = {}, enabled = true): PatcherInfoView {
  return { id, name: id, description: "", enabled, uses_replace_ops: false, options, values };
}

describe("patcherSummary", () => {
  it("counts toggles and names choices", () => {
    const p = patcher("m", [bool("a"), bool("b"), bool("c"), choice("owned", ["off", "mark"], "mark")], { b: "false" });
    expect(patcherSummary(p)).toBe("2 of 3 on · MARK");
  });

  it("is the choice alone, On without options, Off when off", () => {
    expect(patcherSummary(patcher("c", [choice("format", ["x", "y"], "y")]))).toBe("Y");
    expect(patcherSummary(patcher("l", []))).toBe("On");
    expect(patcherSummary(patcher("l", [], {}, false))).toBe("Off");
  });
});

describe("patcherExample", () => {
  it("follows the component name format", () => {
    const fmt = choice("format", ["name_class_grade", "compact_prefix", "short_prefix"], "name_class_grade");
    const to = (v?: string) =>
      patcherExample(patcher("component_grades", [fmt], v ? { format: v } : {}))!.rows.map((r) => r.to);
    expect(to()).toEqual(["Bracer Military C", "FullStop Military C"]);
    expect(to("compact_prefix")).toEqual(["M1C Bracer", "M2C FullStop"]);
    expect(to("short_prefix")).toEqual(["MIL1C Bracer", "MIL2C FullStop"]);
  });

  it("follows the weapon toggles", () => {
    const opts = ["size_prefix", "missile_type_prefix", "weapon_stats", "missile_stats"].map(bool);
    const ex = patcherExample(patcher("weapon_enhancer", opts, { size_prefix: "false" }))!;
    expect(ex.rows.map((r) => r.to)).toEqual(["Omnisky III Cannon", "[IR] Marksman I Missile"]);
  });

  it("has none for Mission Enhancer (it previews live)", () => {
    expect(patcherExample(patcher("mission_enhancer", []))).toBeNull();
  });
});
