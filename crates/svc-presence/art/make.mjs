// Builds the Rich Presence icon SVGs from the lucide glyphs the app uses,
// drawn the way the app icon draws its planet: a graphite surface with a
// hairline edge, a soft accent glow, the glyph as a holo line (accent
// halo under a light stroke). Usage:
//   node make.mjs <lucide-svelte dist/icons dir> <out dir>
import fs from "node:fs";

const [lucideDir, outDir] = process.argv.slice(2);
const icons = {
  loading: "loader-circle",
  on_foot: "footprints",
  flying: "rocket",
  quantum: "orbit",
  contract: "scroll-text",
  mining: "pickaxe",
};
const attrs = (o) => Object.entries(o).map(([k, v]) => `${k}="${v}"`).join(" ");

for (const [key, name] of Object.entries(icons)) {
  const src = fs.readFileSync(`${lucideDir}/${name}.svelte`, "utf8");
  const node = JSON.parse(src.match(/const iconNode = (\[.*\]);/)[1]);
  const glyph = node.map(([tag, a]) => `<${tag} ${attrs(a)}/>`).join("");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 256 256">
  <!-- Starlume Rich Presence icon "${key}": lucide ${name} (ISC), the glyph
       the app uses, as a holo line on the app icon's graphite. Discord shows
       the small image as a circle, so the surface is a disc. -->
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#232325"/><stop offset="1" stop-color="#121213"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#a996ff" stop-opacity="0.26"/><stop offset="1" stop-color="#a996ff" stop-opacity="0"/>
    </radialGradient>
    <filter id="soft" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="5"/></filter>
  </defs>
  <circle cx="128" cy="128" r="126" fill="url(#bg)"/>
  <circle cx="128" cy="128" r="123" fill="none" stroke="#38383c" stroke-width="5"/>
  <circle cx="128" cy="128" r="100" fill="url(#glow)"/>
  <g filter="url(#soft)"><g transform="translate(50 50) scale(6.5)" fill="none" stroke="#a996ff" stroke-opacity="0.8" stroke-width="3.6" stroke-linecap="round" stroke-linejoin="round">${glyph}</g></g>
  <g transform="translate(50 50) scale(6.5)" fill="none" stroke="#ddd5ff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">${glyph}</g>
</svg>
`;
  fs.writeFileSync(`${outDir}/${key}.svg`, svg);
  console.log(key, name);
}
