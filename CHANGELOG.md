# Changelog

## [Unreleased]

- **Settings → Online & privacy** — the game-services switch is now **Allow
  unofficial API calls**: one opt-in for every RSI/CIG service Starlume is not
  an official client of (CIG's game services, and RSI's Spectrum for upcoming
  features). Your current choice and per-feature toggles carry over.

## [0.1.0] - 2026-10-15

The first release of Starlume, a Star Citizen companion for Windows. It reads
your game install and makes the game's own text more useful, and (if you opt
in) shows you where everything you own is.

- **Text Patching** — enriches Star Citizen's English text: blueprint rewards,
  cooldowns and crimestat risk in missions; size, tracking type and stats in
  weapon descriptions; class and grade on ship components; markers on illegal
  goods; shorter labels where the UI overlaps. Every patcher is optional, a
  preview pane shows the result, and Starlume re-patches by itself after a game
  update (waiting until the game is closed).
- **My Items** *(opt-in)* — every item and ship on your account: where it is by
  system, planet and station, what is fitted to which ship beside its saved and
  stock loadouts, armour sets, weapon attachments, crafted stats and resources
  by quality. It reads your inventory through CIG's game services, which are
  off by default (Settings → Online & privacy): not an official API, read-only,
  your own account only.
- **A light tray app** — close the window and Starlume drops to about 11 MB in
  the tray; it can start with Windows, and updates itself after asking.

Coming from a 0.0.x test build? This release only changes the version and the
documentation; your settings and data carry over.

## [0.0.4] - 2026-10-15

- **New look.** Starlume has its own icon: a violet ringed planet with a
  rose moon, replacing the placeholder carried over from Hearth, plus a
  matching tray icon. The app moves from warm amber to a neutral graphite
  background with violet as its accent.

## [0.0.3] - 2026-10-14

- **Lighter in the tray.** Starlume closes its window's web view a minute
  after you hide it to the tray, dropping from ~180 MB to ~11 MB of memory;
  reopening rebuilds it in about half a second, on the page you left. A
  minimized start (run at login) no longer creates a window until you open
  it.
- The update check no longer prompts again every time you reopen the window.

## [0.0.2] - 2026-10-14

First published test release: it exercises the install and auto-update path
ahead of v0.1.0. Expect rough edges. (0.0.1 was tagged but never published:
its release workflow lacked the permission to create the release.)

- **Text Patching** — patches Star Citizen's English text with readable
  mission details (the sc-langpatch patchers), and re-patches after a game
  update. A side pane previews the patched mission text.
- **My Items** — finds every item and ship you own: where it is (by system,
  planet and station), what it is fitted to, your armour sets, weapon kits,
  crafted stats, and your resources by quality. Opt-in: it reads your
  inventory through CIG's game services, which needs online mode plus the
  "Items & ships" (and, for saved loadouts, "Saved loadouts") features in
  Settings.
- **App** — tray app (close to tray, start minimized, run at login), first-run
  setup, notifications, and signed auto-updates. Game data is read from your
  Star Citizen install once per game build, in the background.
