// Builds the Rich Presence icon SVGs from the lucide glyphs the app uses.
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
  const glyph = node.map(([tag, a]) => `<${tag} ${attrs(a)}/>`).join("\n      ");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 256 256">
  <!-- Starlume Rich Presence icon "${key}": lucide ${name} (ISC), the glyph the
       app uses, dark on the accent disc. Discord shows it as a small circle
       on the corner of the large image, so it is a full disc. -->
  <defs>
    <radialGradient id="disc" cx="0.38" cy="0.32" r="0.75">
      <stop offset="0" stop-color="#c3b6ff"/><stop offset="1" stop-color="#8a72ee"/>
    </radialGradient>
  </defs>
  <circle cx="128" cy="128" r="128" fill="url(#disc)"/>
  <circle cx="128" cy="128" r="122" fill="none" stroke="#15112a" stroke-opacity="0.18" stroke-width="4"/>
  <g transform="translate(50 50) scale(6.5)" fill="none" stroke="#15112a" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
      ${glyph}
  </g>
</svg>
`;
  fs.writeFileSync(`${outDir}/${key}.svg`, svg);
  console.log(key, name);
}
