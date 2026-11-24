# Rich Presence art

Upload each PNG to the Discord application (Rich Presence › Art Assets)
under its file name without `.png`: these are the keys `compose` sends
(`svc_presence::compose::asset`).

- `starlume`: the large image when there is no ship picture; the app
  icon (`src-tauri/icons/source/icon.svg`).
- `contract_*`: the small image while a contract shows, one per contract
  type, the tile Contracts' catalog draws for it (`CATEGORY` in
  `src/lib/modules/contracts/Catalog.svelte`). `contract` is the catalog's
  fallback for a type it has no tile for.
- `career_*`: the small image in a ship with no contract showing, one per
  ship career, the tile Ships draws for it (`LOOKS` in
  `src/lib/modules/ships/careers.ts`). `career` is its fallback.

Nothing else gets a small image: on foot with no contract, the profile
shows none rather than a generic icon.

The tiles use the app's `.tile` mixes on the app.css tokens: the hue at 12%
on `--bg-raised`, its edge at 26% on `--border`, the lucide glyph at 85% on
`--text`, at the tile's own proportions (32 px, 7 px corners, a 1 px edge,
a 16 px glyph). Discord shows the small image as a circle, so the tile
sits inside it on a transparent surround. `sheet.png` shows them all.

Rebuild from the repo root, keeping `make.mjs`'s list in step with the two
app files and with `compose::asset`:

```sh
node crates/svc-presence/art/make.mjs node_modules/.pnpm/lucide-svelte@*/node_modules/lucide-svelte/dist/icons crates/svc-presence/art/source
pnpm tauri icon crates/svc-presence/art/source/<key>.svg -o <dir> -p 1024   # then <dir>/1024x1024.png → <key>.png
```

`source/starlume.svg` is a copy of the app icon; refresh it when that changes.
