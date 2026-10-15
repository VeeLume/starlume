import { describe, expect, it } from "vitest";
import { buildStatus, compareVersions, installChangelist, installVersion, upToDate } from "./version";

describe("version compare", () => {
  it("reads the install version without channel and build", () => {
    expect(installVersion("4.10.2-live.12913470")).toBe("4.10.2");
  });

  it("treats missing parts as zero and compares numerically", () => {
    expect(compareVersions("4.10", "4.10.0")).toBe(0);
    expect(compareVersions("4.10.2", "4.9.9")).toBe(1);
    expect(compareVersions("4.2", "4.10")).toBe(-1);
  });

  it("calls a hotfix build of the announced version current", () => {
    expect(upToDate("4.10.2-live.12913470", "4.10.2")).toBe("current");
    expect(upToDate("4.10.1-live.12800000", "4.10.2")).toBe("behind");
    expect(upToDate("4.11.0-live.13000000", "4.10.2")).toBe("ahead");
  });

  it("tells a hotfix build from the noted one", () => {
    expect(installChangelist("4.10.2-live.12913470")).toBe("12913470");
    expect(installChangelist("4.10.2")).toBeNull();
    expect(buildStatus("4.10.2-live.12913470", "4.10.2", "12881860")).toBe("hotfix");
    expect(buildStatus("4.10.2-live.12881860", "4.10.2", "12881860")).toBe("current");
    expect(buildStatus("4.10.2-live.12913470", "4.10.2", null)).toBe("current");
    expect(buildStatus("4.10.1-live.12999999", "4.10.2", "12881860")).toBe("behind");
  });
});
