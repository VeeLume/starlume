import { describe, expect, it } from "vitest";
import { gameTag, splitAdded } from "./preview";

describe("splitAdded", () => {
  it("splits off what the patch appended", () => {
    const original = "About the job\\nBring the parts.\\n";
    const patched = "About the job\\nBring the parts.\\n\\n<EM4>Mission Info</EM4>\\nCooldown: 30min";
    expect(splitAdded(original, patched)).toEqual({
      base: "About the job\\nBring the parts.",
      added: "<EM4>Mission Info</EM4>\\nCooldown: 30min",
    });
  });

  it("keeps the whole text when the patch changed the original", () => {
    expect(splitAdded("Kill the target", "Kill the [CS Risk] target")).toEqual({
      base: "Kill the [CS Risk] target",
      added: "",
    });
  });

  it("has nothing added when nothing changed", () => {
    expect(splitAdded("Text", "Text")).toEqual({ base: "Text", added: "" });
  });
});

describe("gameTag", () => {
  it("finds the tag an option adds", () => {
    expect(gameTag("Add [BP] to titles of missions that reward blueprints")).toBe("[BP]");
    expect(gameTag("Add [Uniq] to titles of one-time-only missions")).toBe("[Uniq]");
    expect(gameTag("Append cooldown, rep reward, scrip, and payout")).toBeNull();
  });
});
