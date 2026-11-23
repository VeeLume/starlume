# Rich Presence art

Upload each PNG to the Discord application (Rich Presence › Art Assets)
under its file name without `.png`: these are the keys `compose` sends
(`svc_presence::compose::asset`).

| Key | Shows | Glyph |
|---|---|---|
| `starlume` | the large image when there is no ship picture | the app icon (`src-tauri/icons/source/icon.svg`) |
| `loading` | before a shard | lucide `loader-circle` |
| `on_foot` | on foot | lucide `footprints` |
| `flying` | in a ship | lucide `rocket` |
| `quantum` | in quantum travel | lucide `orbit` (Ships' quantum drives) |
| `contract` | a contract shows | lucide `scroll-text` (the Contracts module) |
| `mining` | mining | lucide `pickaxe` (the Mining module) |

The icons are dark glyphs on the accent disc (`--accent` #a996ff,
`--on-accent` #15112a): Discord shows the small image as a circle about
20 px wide on the large image's corner. `sheet.png` shows them together.

Rebuild from the repo root:

```sh
node crates/svc-presence/art/make.mjs node_modules/.pnpm/lucide-svelte@*/node_modules/lucide-svelte/dist/icons crates/svc-presence/art/source
pnpm tauri icon crates/svc-presence/art/source/<key>.svg -o <dir> -p 1024   # then <dir>/1024x1024.png → <key>.png
```

`source/starlume.svg` is a copy of the app icon; refresh it when that changes.
