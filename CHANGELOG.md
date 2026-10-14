# Changelog

## [Unreleased]

## [0.0.1] - 2026-10-14

First test release: it exercises the install and auto-update path ahead of
v0.1.0. Expect rough edges.

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
