// The explorer's URL state (frontend rule 7): which view, where you are,
// what is selected, and every filter. Selection is navigation, so it lives
// here too (a push per pick: back returns to the item you were on). Which
// tree branches are open is page-local, never the URL.

import { createBrowseState } from "@veelume/ui";

export function createItemsBrowse() {
  return createBrowseState({
    q: { kind: "text" },
    view: { kind: "one", default: "items", narrows: false },
    /** The open place or container: a tree node id. */
    at: { kind: "one", default: "", narrows: false },
    /** The selected node id. */
    sel: { kind: "one", default: "", narrows: false },
    crafted: { kind: "one", default: "" },
    bricked: { kind: "one", default: "" },
    cat: { kind: "one", default: "" },
    sub: { kind: "one", default: "" },
    w: { kind: "one", default: "" },
    arm: { kind: "one", default: "" },
    kit: { kind: "one", default: "" },
    /** Resources: the quality filter. */
    min: { kind: "one", default: "0" },
    sort: { kind: "one", default: "q", narrows: false },
  });
}

export type ItemsBrowse = ReturnType<typeof createItemsBrowse>;
