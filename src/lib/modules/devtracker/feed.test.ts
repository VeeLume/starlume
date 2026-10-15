import { describe, expect, it } from "vitest";
import type { DevPostView } from "$lib/bindings";
import { groupFeed, initials, mentionParts, nameHue, RUN_GAP_MS } from "./feed";

const MIN = 60 * 1000;

function post(key: string, kind: string, author: string, place: string | null, time_ms: number): DevPostView {
  return {
    key,
    kind,
    author_id: author.toLowerCase(),
    author,
    place,
    subject: null,
    body: key,
    time_ms,
    url: "",
    shown: true,
    reason: "",
    ops: false,
    unread: false,
    quote: null,
  };
}

describe("groupFeed", () => {
  it("joins one author's close chat in one lobby, oldest first", () => {
    const items = groupFeed([
      post("c3", "chat", "Waka", "general", 100 * MIN),
      post("c2", "chat", "Waka", "general", 90 * MIN),
      post("c1", "chat", "Waka", "general", 60 * MIN),
    ]);
    expect(items).toHaveLength(1);
    expect(items[0].kind === "run" && items[0].messages.map((m) => m.key)).toEqual(["c1", "c2", "c3"]);
  });

  it("splits on another author, another lobby, a forum post or a long gap", () => {
    const t = 1000 * MIN;
    const items = groupFeed([
      post("a", "chat", "Waka", "general", t),
      post("b", "chat", "Night", "general", t - MIN),
      post("c", "chat", "Night", "testing", t - 2 * MIN),
      post("d", "reply", "Night", "General", t - 3 * MIN),
      post("e", "chat", "Night", "testing", t - 4 * MIN),
      post("f", "chat", "Night", "testing", t - 4 * MIN - RUN_GAP_MS - 1),
    ]);
    expect(items.map((i) => i.key)).toEqual(["a", "b", "c", "d", "e", "f"]);
    expect(items[3].kind).toBe("post");
  });

  it("measures the gap from the run's oldest message", () => {
    const items = groupFeed([
      post("c3", "chat", "Waka", "general", 80 * MIN),
      post("c2", "chat", "Waka", "general", 40 * MIN),
      post("c1", "chat", "Waka", "general", 0),
    ]);
    expect(items).toHaveLength(1);
  });
});

describe("monograms", () => {
  it("drop the CIG tag and take two letters", () => {
    expect(initials("Nightrider-CIG")).toBe("Ni");
    expect(initials("CIG-SeanT")).toBe("Se");
    expect(initials("Swordfish_CIG")).toBe("Sw");
    expect(initials("500servererror-CIG")).toBe("50");
    expect(initials("--")).toBe("?");
  });

  it("give a name the same hue every time", () => {
    expect(nameHue("Wakapedia-CIG")).toBe(nameHue("Wakapedia-CIG"));
    expect(nameHue("Wakapedia-CIG")).toMatch(/^var\(--hue-/);
  });
});

describe("mentionParts", () => {
  it("marks @mentions and keeps the rest", () => {
    expect(mentionParts("@Calean I'll have news (@Waka too)")).toEqual([
      { text: "@Calean", mention: true },
      { text: " I'll have news (", mention: false },
      { text: "@Waka", mention: true },
      { text: " too)", mention: false },
    ]);
  });

  it("leaves e-mail-like text alone", () => {
    expect(mentionParts("mail me@example.com")).toEqual([{ text: "mail me@example.com", mention: false }]);
  });
});
