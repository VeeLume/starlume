import { describe, expect, it } from "vitest";
import { copyHeadings, myItemsShipHref, rentalEnd, type CopyFacts } from "./ownedShips";

const copy = (source: CopyFacts["source"], place: string | null = null, rentalExpiry: number | null = null): CopyFacts => ({
  source,
  place,
  rentalExpiry,
});
const end = (t: number) => `T${t}`;

describe("copyHeadings", () => {
  it("names a pledged and a rented copy by what each is", () => {
    expect(copyHeadings([copy("pledge", "Test City hangar"), copy("rental", null, 5)], end)).toEqual([
      "Pledge, Test City hangar",
      "Rental, ends T5",
    ]);
  });

  it("numbers only the copies nothing else tells apart", () => {
    expect(
      copyHeadings([copy("pledge", "Test City hangar"), copy("in_game"), copy("in_game"), copy("rental", "in Test Carrier", 5)], end),
    ).toEqual(["Pledge, Test City hangar", "In-game, copy 1", "In-game, copy 2", "Rental, in Test Carrier, ends T5"]);
  });

  it("has a name for a copy it knows nothing of", () => {
    expect(copyHeadings([copy("unknown")], end)).toEqual(["Your ship"]);
    expect(copyHeadings([copy("unknown"), copy("unknown")], end)).toEqual(["Copy 1", "Copy 2"]);
  });
});

describe("rentalEnd", () => {
  it("says the day as well as the time", () => {
    // Noon UTC: the same calendar day in every time zone but the far edges.
    const s = rentalEnd(Date.UTC(2026, 10, 16, 12, 0) / 1000, "en-GB");
    expect(s).toMatch(/^1[67] Nov \d\d:\d\d$/);
  });
});

describe("myItemsShipHref", () => {
  it("opens the Ships view on the ship, selected", () => {
    const url = new URL(myItemsShipHref("urn:test:ship-1"), "http://x");
    expect(url.pathname).toBe("/items");
    expect(Object.fromEntries(url.searchParams)).toEqual({ view: "ships", at: "ship:urn:test:ship-1", sel: "ship:urn:test:ship-1" });
    expect(new URL(myItemsShipHref("geid:7", true), "http://x").searchParams.get("bricked")).toBe("1");
  });
});
