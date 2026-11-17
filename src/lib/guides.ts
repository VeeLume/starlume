// What's new, per release: the changes worth a page in the guide. Module
// pages need no entry here, they come from the registry (a module the user
// hasn't been offered gets one, from its descriptor's pitch and setup).
//
// Releasing: a release with a change that touches something the user uses
// or set adds an entry, newest first. Only the big ones: a new page, a
// setting that moved or changed meaning, a workflow that changed. Fixes go
// to CHANGELOG.md only. A release without an entry shows no changes page,
// and no guide at all when it brings no module either.

import type { AppSettings } from "$lib/bindings";
import { compareVersions } from "$lib/home/version";

export interface GuideChange {
  kind: "new" | "changed" | "setting";
  title: string;
  body: string;
  /** Where to look: an in-app route, with the button's label. */
  href?: string;
  action?: string;
  /** A setting's current value, shown under the change ("on, 2 features
   *  allowed"); `null` shows nothing. */
  yours?: (s: AppSettings) => string | null;
}

export interface ReleaseGuide {
  version: string;
  changes: GuideChange[];
}

export const RELEASE_GUIDES: ReleaseGuide[] = [
  {
    version: "0.6.0",
    changes: [
      {
        kind: "changed",
        title: "My Items opens on your gear",
        body: "Every place leads with what you crafted, your full armour sets and your kitted weapons. One field above the list holds the place, the kind and your search: type a word, or Backspace to step back out.",
        href: "/items",
        action: "Open My Items",
      },
      {
        kind: "new",
        title: "Try a part as crafted",
        body: "In the part picker of Ships and Mining, set a fitted part to crafted at a quality: the page's figures follow its recipe.",
        href: "/ships",
        action: "Open Ships",
      },
      {
        kind: "changed",
        title: "Saved and Fitted now",
        body: "A ship you own twice says which copy is which. Saved is what a claim restores, Fitted now what is on it, on Ships and in My Items alike.",
        href: "/ships",
        action: "Open Ships",
      },
    ],
  },
  {
    version: "0.5.0",
    changes: [
      {
        kind: "new",
        title: "Ship holo",
        body: "On Build, your ship is a hologram of what is fitted. Drag it to turn it, point at a part to find it, click it for the ship's stats. The switch on the card shows the picture instead.",
        href: "/ships",
        action: "Open Ships",
      },
      {
        kind: "new",
        title: "Settings → Performance",
        body: "How much memory Starlume is using right now.",
        href: "/settings/performance",
        action: "Open",
      },
    ],
  },
  {
    version: "0.3.0",
    changes: [
      {
        kind: "changed",
        title: "Settings → About",
        body: "The version, the update check and running setup again moved here from General. Earlier releases' guides are listed there too.",
        href: "/settings/about",
        action: "Open",
      },
    ],
  },
  {
    version: "0.2.0",
    changes: [
      {
        kind: "new",
        title: "Home",
        body: "A game status card for each installed channel: its version, its game data, whether your build is current, your text patch. News and your modules beside it; Customize hides and orders the cards.",
        href: "/",
        action: "Open Home",
      },
      {
        kind: "new",
        title: "News",
        body: "The LIVE patch notes in short, the dev cycle as a timeline, the newest comm-links.",
        href: "/news",
        action: "Open News",
      },
      {
        kind: "setting",
        title: "Allow unofficial API calls",
        body: "The game-services switch is now one opt-in for every RSI and CIG service Starlume is not an official client of, Spectrum included. Your choice and per-feature toggles carried over.",
        href: "/settings/online",
        action: "Review",
        yours: (s) => {
          if (!s.grpc_enabled) return "off";
          const n = s.grpc_features.length;
          return `on, ${n} feature${n === 1 ? "" : "s"} allowed`;
        },
      },
      {
        kind: "changed",
        title: "Text Patching",
        body: "One page that fits the window: your installs on the left, the selected patcher's options and a preview on the right.",
        href: "/langpatch",
        action: "Open",
      },
    ],
  },
];

/** The release guides newer than `seen`, up to the running version. */
export function guidesSince(seen: string | null, current: string): ReleaseGuide[] {
  return RELEASE_GUIDES.filter(
    (g) => (seen === null || compareVersions(g.version, seen) > 0) && compareVersions(g.version, current) <= 0,
  );
}
