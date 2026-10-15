# Changelog

## [Unreleased]

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
