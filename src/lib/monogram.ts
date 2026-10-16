// A person's monogram: two letters and a stable category hue from the
// name, so the same person reads in the same colour everywhere (the dev
// tracker's authors, a mining crew).

const HUES = ["--hue-sky", "--hue-apricot", "--hue-rose", "--hue-teal", "--hue-sand"];

/** A stable category hue for a name (monograms). */
export function nameHue(name: string): string {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return `var(${HUES[h % HUES.length]})`;
}

/** Two letters for a monogram: the name without its CIG tag
 *  ("Nightrider-CIG" → "Ni", "CIG-SeanT" → "Se"). */
export function initials(name: string): string {
  const bare = name.replace(/^CIG[-_]|[-_]CIG$/i, "") || name;
  const letters = [...bare.replace(/[^\p{L}\p{N}]/gu, "")];
  if (!letters.length) return "?";
  return letters[0].toUpperCase() + (letters[1] ?? "").toLowerCase();
}
