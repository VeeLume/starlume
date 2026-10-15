<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" height="96" alt="Starlume icon">
</p>

# Starlume

**A Star Citizen companion for Windows.** Starlume reads your game install and
makes the game's own text more useful, and (if you opt in) shows you where every
item and ship you own actually is.

> **Starlume is young.** Things work, but expect rough edges and changes
> between releases. Feedback and bug reports are welcome in
> [Issues](https://github.com/VeeLume/starlume/issues).

## Features

### Text Patching

Patches Star Citizen's English text with information the game doesn't show:

- **Mission Enhancer:** blueprint rewards, cooldowns, crimestat risk and more in
  mission titles and descriptions.
- **Weapon Enhancer:** size prefixes, missile tracking type and combat stats in
  weapon descriptions.
- **Component Grades:** ship components labelled with class and grade, e.g.
  *Bracer Military C*.
- **Illegal Goods Markers:** drugs and contraband marked, with where they are
  outlawed.
- **Label Fixes:** shorter labels where the game's UI text overlaps.

Every patcher can be switched on or off, most have options, and a preview pane
shows how mission text will look. After a game update Starlume re-patches on
its own as soon as it notices the update; if the game is running, it waits
until you close it.

### My Items *(opt-in)*

Finds every item and ship on your account:

- where it is, by system, planet and station, including ship lockers and what
  you carry
- what is fitted to which ship, beside the saved and stock loadouts
- armour sets with the pieces you own, weapons with their attachments
- crafted items with their stats, and your resources by quality

My Items reads your inventory from CIG's game services, so it only works when
you switch those on (see [Online and privacy](#online-and-privacy)).

## Install

1. Download `Starlume_<version>_x64-setup.exe` from the
   [latest release](https://github.com/VeeLume/starlume/releases/latest).
2. Run it. The installer isn't code-signed yet, so Windows SmartScreen may warn
   about an unknown publisher: choose **More info → Run anyway**.

Requires Windows 10 or 11 (64-bit) and Microsoft's WebView2 runtime. Windows 11
ships with it; on Windows 10 the installer fetches it if it's missing.

**Updates:** Starlume checks for a new version when it starts and asks before
installing it. Updates are signed and verified before they're installed.

**Tray app:** closing the window keeps Starlume in the tray, where it uses very
little memory. It can start with Windows (Settings → General → Run at login).

## Online and privacy

Starlume does its work on your machine: it reads your Star Citizen install and
parses the game data locally. Online use is limited and under your control:

- **Online features** (Settings → Online & privacy → Enable online features, on
  by default) let Starlume look up your public RSI profile and download what
  you ask it to. Switch them off and nothing leaves your machine except the
  update check.
- **Update check:** always on, even offline, so everyone gets security fixes.
  It only asks GitHub whether a new release exists.

### CIG's game services *(off by default)*

Some features (My Items, and the "owned blueprint" marks in mission text) read
your own account data through the backend services the game client itself
uses. **These are not an official public API, and using them may be against
CIG's Terms of Service.** Starlume only reads, only your own account, and never
changes anything. Each feature stays off until you turn on **Allow game-services
calls** in Settings → Online & privacy and then enable that feature. Use them at
your own risk.

## Your data

- **Settings, caches and your inventory snapshots:** `%APPDATA%\starlume`
- **In your Star Citizen folder, while Text Patching is on:**
  `data\Localization\english\global.ini` and a `g_language = english` line in
  `user.cfg`. Switching Text Patching off removes both.

## Uninstall

Switch Text Patching off first to get the game's original text back, then
uninstall Starlume from Windows Settings → Apps. To remove its data too, delete
`%APPDATA%\starlume`.

## Not affiliated with CIG

Starlume is a fan-made tool. It is not affiliated with or endorsed by Cloud
Imperium Games. Star Citizen® and Squadron 42® are trademarks of Cloud Imperium
Rights LLC.

## Development

Building, architecture and the module rules are in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
