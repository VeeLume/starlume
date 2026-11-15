// The qualities a material comes in. The game sorts a mined resource's
// quality into bands and gives every roll in a band one value, so a
// material exists at a handful of qualities, not at any from 0 to 1000
// (Rust: svc_data::quality). A control that sets a quality stops at those
// values. Pure: used by the recipe sheet's sliders and the plan's minimums.

/** Every roll from `from` to `to` comes out as `value`. */
export interface QualityBand {
  from: number;
  to: number;
  value: number;
}

/** The qualities a slot can be set to: its material's, from the slot's own
 *  minimum (`floor`) up. Empty when the data gives the material no bands. */
export function stopsOf(bands: QualityBand[], floor = 0): number[] {
  const values = bands.map((b) => b.value);
  const from = values.findIndex((v) => v >= floor);
  return from < 0 ? values.slice(-1) : values.slice(from);
}

/** The lowest stop at or above `quality`: what a minimum of `quality` asks
 *  for. The highest stop when it is above them all; `quality` itself when
 *  there are no stops. */
export function snapUp(stops: number[], quality: number): number {
  return stops.find((v) => v >= quality) ?? stops.at(-1) ?? quality;
}

/** The stop nearest to `quality` (the upper one on a tie). */
export function snapNearest(stops: number[], quality: number): number {
  if (stops.length === 0) return quality;
  return stops.reduce((best, v) => (Math.abs(v - quality) <= Math.abs(best - quality) ? v : best));
}

/** The band a quality falls in. */
export function bandOf(bands: QualityBand[], quality: number): QualityBand | undefined {
  return bands.find((b) => quality >= b.from && quality <= b.to);
}

/** "400 to 599": the band a quality falls in, in words; empty when none. */
export function bandText(bands: QualityBand[], quality: number): string {
  const band = bandOf(bands, quality);
  return band ? `${band.from} to ${band.to}` : "";
}

/** Where `quality` sits along evenly spaced stops, 0..1: a slider over the
 *  stops places a mark there. */
export function markAt(stops: number[], quality: number): number {
  if (stops.length < 2) return 0;
  if (quality <= stops[0]) return 0;
  const last = stops.length - 1;
  if (quality >= stops[last]) return 1;
  const i = stops.findIndex((v, n) => quality >= v && quality < stops[n + 1]);
  return (i + (quality - stops[i]) / (stops[i + 1] - stops[i])) / last;
}

/** A planned craft's minimum for each of `count` slots: its own where it
 *  sets one, else its one quality for all. */
export function slotMinimums(craft: { quality: number; slots: number[] }, count: number): number[] {
  return Array.from({ length: count }, (_, i) => craft.slots[i] ?? craft.quality);
}
