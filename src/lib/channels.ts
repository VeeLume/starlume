// Game channels' category hue (app.css "Category hues"): the same channel
// reads in the same colour everywhere it shows — Home, News, catalogs.

const HUES: Record<string, string> = {
  live: "var(--hue-live)",
  hotfix: "var(--hue-hotfix)",
  ptu: "var(--hue-ptu)",
  eptu: "var(--hue-eptu)",
  "tech-preview": "var(--hue-tech-preview)",
  techpreview: "var(--hue-tech-preview)",
};

/** The channel's hue as a CSS value for `--hue`, or `undefined` (neutral)
 *  for a channel without one. Takes any casing ("LIVE", "Live", "live"). */
export function channelHue(channel: string | null | undefined): string | undefined {
  return channel ? HUES[channel.trim().toLowerCase().replace(/[\s_]+/g, "-")] : undefined;
}
