import { describe, expect, it } from "vitest";
import { channelHue } from "./channels";

describe("channelHue", () => {
  it("maps every casing of a channel to its hue", () => {
    expect(channelHue("LIVE")).toBe("var(--hue-live)");
    expect(channelHue("Live")).toBe("var(--hue-live)");
    expect(channelHue("ptu")).toBe("var(--hue-ptu)");
    expect(channelHue("TECH-PREVIEW")).toBe("var(--hue-tech-preview)");
    expect(channelHue("Tech Preview")).toBe("var(--hue-tech-preview)");
  });

  it("leaves unknown or missing channels neutral", () => {
    expect(channelHue("EVOCATI")).toBeUndefined();
    expect(channelHue(null)).toBeUndefined();
    expect(channelHue("")).toBeUndefined();
  });
});
