// How the ship holo's camera behaves: a turntable while it is a card (one
// axis, nothing to get lost in), free once expanded, and turning a part
// towards the viewer. Frames are drawn only while a turn runs (the view has
// no loop of its own, and the tray app has no idle work).
import type { ShipView } from "@veelume/sc-render";

const TURN_MS = 450;
/** How far above the ship's horizontal the camera starts. */
const ELEVATION = (12 * Math.PI) / 180;
/** A part this close to the ship's axis has no side to turn to. */
const CENTRED = 0.04;

/** Shortest signed way from angle `a` to angle `b`. */
export function shortest(a: number, b: number): number {
  const d = (b - a) % (Math.PI * 2);
  if (d > Math.PI) return d - Math.PI * 2;
  if (d < -Math.PI) return d + Math.PI * 2;
  return d;
}

export class Turntable {
  /** Where the ship rests when no part asks for it: the viewer's last drag. */
  private rest = 0;
  private frame = 0;

  constructor(private readonly view: ShipView) {
    view.controls.addEventListener("start", () => this.stop());
    view.controls.addEventListener("end", () => (this.rest = this.azimuth()));
  }

  /** Down to a low look along the ship, at the same distance and side. */
  level() {
    const { camera, controls } = this.view;
    const distance = camera.position.distanceTo(controls.target);
    const azimuth = this.azimuth();
    const flat = Math.cos(ELEVATION) * distance;
    camera.position.set(
      controls.target.x + Math.cos(azimuth) * flat,
      controls.target.y + Math.sin(azimuth) * flat,
      controls.target.z + Math.sin(ELEVATION) * distance,
    );
    controls.update();
  }

  /** Around the ship's vertical axis only, from the low look. */
  lock() {
    const { controls } = this.view;
    controls.minPolarAngle = 0;
    controls.maxPolarAngle = Math.PI;
    this.level();
    const polar = controls.getPolarAngle();
    controls.minPolarAngle = polar;
    controls.maxPolarAngle = polar;
    controls.enablePan = false;
    controls.enableZoom = false;
    this.rest = this.azimuth();
  }

  /** Every axis, pan and zoom. */
  free() {
    this.stop();
    const { controls } = this.view;
    controls.minPolarAngle = 0;
    controls.maxPolarAngle = Math.PI;
    controls.enablePan = true;
    controls.enableZoom = true;
  }

  /** Bring these ports' side of the ship to the viewer. */
  turnTo(ports: string[]) {
    const angle = sideOf(this.view, ports);
    if (angle !== null) this.turn(angle);
  }

  /** Back to where it rested. */
  settle() {
    this.turn(this.rest);
  }

  stop() {
    cancelAnimationFrame(this.frame);
    this.frame = 0;
  }

  private azimuth(): number {
    const { camera, controls } = this.view;
    return Math.atan2(camera.position.y - controls.target.y, camera.position.x - controls.target.x);
  }

  private turn(to: number) {
    this.stop();
    const from = this.azimuth();
    const by = shortest(from, to);
    if (Math.abs(by) < 0.01) return;
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return this.setAzimuth(to);
    const start = performance.now();
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / TURN_MS);
      this.setAzimuth(from + by * (1 - (1 - t) ** 3));
      this.frame = t < 1 ? requestAnimationFrame(step) : 0;
    };
    this.frame = requestAnimationFrame(step);
  }

  private setAzimuth(angle: number) {
    const { camera, controls } = this.view;
    const flat = Math.hypot(camera.position.x - controls.target.x, camera.position.y - controls.target.y);
    camera.position.x = controls.target.x + Math.cos(angle) * flat;
    camera.position.y = controls.target.y + Math.sin(angle) * flat;
    controls.update(); // draws through its change event
  }
}

/** What the holo knows of a port. Private in sc-render v0.1.0 (its merged
 *  mesh is in the ship frame): read here until it has a public call for a
 *  port's bounds. */
type Box = { min: Point; max: Point };
type Point = { x: number; y: number; z: number };
type PortBounds = {
  path: string;
  bounds?: Box;
  mesh: { geometry: { boundingBox: Box | null; computeBoundingBox(): void } };
};

function boxOf(port: PortBounds): Box | null {
  if (port.bounds) return port.bounds;
  if (!port.mesh.geometry.boundingBox) port.mesh.geometry.computeBoundingBox();
  return port.mesh.geometry.boundingBox;
}

/** The direction of these ports from the ship's axis, or null when they
 *  are unknown or central. */
function sideOf(view: ShipView, ports: string[]): number | null {
  const known = (view as unknown as { holo: { ports: PortBounds[] } | null }).holo?.ports ?? [];
  const wanted = ports.map((p) => p.toLowerCase());
  const found = known
    .filter((k) => wanted.some((w) => k.path === w || k.path.startsWith(`${w}/`)))
    .flatMap((k) => boxOf(k) ?? []);
  if (!found.length) return null;
  const mid = (axis: "x" | "y") => found.reduce((sum, b) => sum + (b.min[axis] + b.max[axis]) / 2, 0) / found.length;
  const x = mid("x") - view.centre.x;
  const y = mid("y") - view.centre.y;
  const size = view.bounds.max.distanceTo(view.bounds.min) / 2;
  return Math.hypot(x, y) < size * CENTRED ? null : Math.atan2(y, x);
}
