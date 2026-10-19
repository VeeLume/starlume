import { describe, expect, it } from "vitest";
import { RELEASE_GUIDES, guidesSince } from "./guides";
import { compareVersions } from "$lib/home/version";

describe("guidesSince", () => {
  it("lists the releases after the last one seen, up to the running version", () => {
    expect(guidesSince("0.1.0", "0.2.0").map((g) => g.version)).toEqual(["0.2.0"]);
    expect(guidesSince("0.2.0", "0.2.0")).toEqual([]);
    // A guide for a release the user isn't running yet stays hidden.
    expect(guidesSince("0.1.0", "0.1.5")).toEqual([]);
    expect(guidesSince(null, "9.0.0")).toHaveLength(RELEASE_GUIDES.length);
  });

  it("keeps the guides newest first", () => {
    const versions = RELEASE_GUIDES.map((g) => g.version);
    expect([...versions].sort((a, b) => compareVersions(b, a))).toEqual(versions);
  });
});
