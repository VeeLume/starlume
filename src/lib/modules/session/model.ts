// Session log: how a session reads on the page. Pure (no stores, no IPC), so
// vitest covers it. The wording is UI text (English, like the rest); numbers
// and times follow the user's formatting locale.
//
// A session's timeline is what the log and the stream said, entry by entry.
// The page shows fewer, larger things: `readSession` folds the entries into
// rows (a mission with its objectives is one row, a vehicle's enter and
// leave are one stay, jurisdiction changes are words on a place) and into
// the status strip's "what is true now".
//
// Names: a session stores the game's internal names. The page gets display
// names from Rust (`session_names`); `Names` is that lookup. A name that
// has not arrived yet reads as `PENDING`, never as the internal one.

import type {
  BodyPart,
  DisplayName,
  Entry,
  Session,
  SessionNameQuery,
  SessionNames,
  SessionSummary,
  WalletChange,
} from "$lib/bindings";

// ── Formatting ──────────────────────────────────────────────────────────────

/** "2 h 14 min", "45 min", "under a minute". */
export function duration(ms: number): string {
  const minutes = Math.floor(Math.max(0, ms) / 60_000);
  if (minutes < 1) return "under a minute";
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** How long a session ran; a live one runs until `now`. */
export function sessionLength(
  session: Pick<Session, "started_at" | "ended_at">,
  now: number,
): number {
  return (session.ended_at ?? now) - session.started_at;
}

const whole = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });

/** "125,000". */
export function amount(value: number): string {
  return whole.format(value);
}

/** "+25,000" / "−825" / "±0". */
export function signed(value: number): string {
  const sign = value > 0 ? "+" : value < 0 ? "−" : "±";
  return `${sign}${whole.format(Math.abs(value))}`;
}

/** "125,000 aUEC". */
export function auec(value: number): string {
  return `${amount(value)} aUEC`;
}

/** "+25,000 aUEC" / "−825 aUEC" / "±0 aUEC". */
export function signedAuec(value: number): string {
  return `${signed(value)} aUEC`;
}

/** The time of day ("14:23"). */
export function clock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** The time of day to the second ("14:23:05"), for a row's details. */
export function clockSeconds(ms: number): string {
  return new Date(ms).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** Day and time ("13 Nov 2026, 17:10"). */
export function dayTime(ms: number): string {
  return new Date(ms).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
}

/** The day ("13 Nov 2026"). */
export function day(ms: number): string {
  return new Date(ms).toLocaleDateString([], { dateStyle: "medium" });
}

/** "1 shard" / "3 shards". */
export function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

// ── Names ───────────────────────────────────────────────────────────────────

/** What a name reads as until Rust has answered. */
export const PENDING = "…";

const pending: DisplayName = { name: PENDING, parents: [], resolved: false };

/** Display names by internal name. */
export interface Names {
  place(internal: string): DisplayName;
  shop(internal: string): DisplayName;
  vehicle(className: string): DisplayName;
  item(guid: string): DisplayName;
  resource(guid: string): DisplayName;
  contract(internal: string): DisplayName;
  /** A quantum destination. */
  destination(internal: string): DisplayName;
  /** An item the log names by class only (an attachment, its weapon). */
  itemClass(className: string): DisplayName;
}

/** The lookup over what Rust has answered so far (`null`: nothing yet). */
export function namesFrom(book: SessionNames | null | undefined): Names {
  const pick = (map: Partial<Record<string, DisplayName>> | undefined, key: string) =>
    map?.[key] ?? pending;
  return {
    place: (k) => pick(book?.places, k),
    shop: (k) => pick(book?.shops, k),
    vehicle: (k) => pick(book?.vehicles, k),
    item: (k) => pick(book?.items, k),
    resource: (k) => pick(book?.resources, k),
    contract: (k) => pick(book?.contracts, k),
    destination: (k) => pick(book?.destinations, k),
    itemClass: (k) => pick(book?.item_classes, k),
  };
}

/** Every internal name a session holds, for `session_names`. */
export function nameQuery(session: Session): SessionNameQuery {
  const places = new Set<string>();
  const shops = new Set<string>();
  const vehicles = new Set<string>();
  const items = new Map<string, string>();
  const resources = new Set<string>();
  const contracts = new Set<string>();
  const destinations = new Set<string>();
  const itemClasses = new Set<string>();
  for (const { kind } of session.timeline) {
    switch (kind.type) {
      case "location":
      case "refinery_order_completed":
        places.add(kind.location);
        break;
      case "shop_purchase":
        shops.add(kind.shop);
        items.set(kind.item_guid, kind.item_name);
        break;
      case "trade":
        shops.add(kind.shop);
        resources.add(kind.resource_guid);
        break;
      case "vehicle_entered":
      case "vehicle_left":
      case "ship_destroyed":
      case "ship_lost":
        vehicles.add(kind.class_name);
        break;
      case "treated":
        if (kind.vehicle) vehicles.add(kind.vehicle);
        break;
      case "mission_ended":
        if (kind.contract) contracts.add(kind.contract);
        break;
      case "quantum_travel":
        destinations.add(kind.to);
        if (kind.vehicle) vehicles.add(kind.vehicle);
        break;
      case "attachments":
        if (kind.weapon) itemClasses.add(kind.weapon);
        for (const item of kind.items) itemClasses.add(item.item);
        break;
    }
  }
  return {
    channel: session.channel,
    places: [...places],
    shops: [...shops],
    vehicles: [...vehicles],
    items: [...items].map(([guid, class_name]) => ({ guid, class_name })),
    resources: [...resources],
    contracts: [...contracts],
    destinations: [...destinations],
    item_classes: [...itemClasses],
  };
}

/** The part of `query` that `book` has no answer for; `null` when it has
 *  them all. */
export function unanswered(
  query: SessionNameQuery,
  book: SessionNames | null | undefined,
): SessionNameQuery | null {
  const lacks = (map: Partial<Record<string, DisplayName>> | undefined, key: string) =>
    !map || !(key in map);
  const rest: SessionNameQuery = {
    channel: query.channel,
    places: query.places.filter((k) => lacks(book?.places, k)),
    shops: query.shops.filter((k) => lacks(book?.shops, k)),
    vehicles: query.vehicles.filter((k) => lacks(book?.vehicles, k)),
    items: query.items.filter((i) => lacks(book?.items, i.guid)),
    resources: query.resources.filter((k) => lacks(book?.resources, k)),
    contracts: query.contracts.filter((k) => lacks(book?.contracts, k)),
    destinations: query.destinations.filter((k) => lacks(book?.destinations, k)),
    item_classes: query.item_classes.filter((k) => lacks(book?.item_classes, k)),
  };
  const empty =
    rest.places.length +
      rest.shops.length +
      rest.vehicles.length +
      rest.items.length +
      rest.resources.length +
      rest.contracts.length +
      rest.destinations.length +
      rest.item_classes.length ===
    0;
  return empty ? null : rest;
}

/** `into` with the answers of `more` added (newer answers win). */
export function mergeNames(into: SessionNames | null | undefined, more: SessionNames): SessionNames {
  return {
    data_loaded: more.data_loaded,
    places: { ...into?.places, ...more.places },
    shops: { ...into?.shops, ...more.shops },
    vehicles: { ...into?.vehicles, ...more.vehicles },
    items: { ...into?.items, ...more.items },
    resources: { ...into?.resources, ...more.resources },
    contracts: { ...into?.contracts, ...more.contracts },
    destinations: { ...into?.destinations, ...more.destinations },
    item_classes: { ...into?.item_classes, ...more.item_classes },
  };
}

// ── Rows ────────────────────────────────────────────────────────────────────

/** What a row is about: its hue on the page. */
export type RowKind = "session" | "place" | "ship" | "mission" | "buy" | "money" | "life" | "other";

/** The log's "Show" filter. A row can be in several (a mission that paid). */
export type Facet = "missions" | "money" | "travel";

export const FACETS: { value: Facet; label: string }[] = [
  { value: "missions", label: "Missions" },
  { value: "money", label: "Money" },
  { value: "travel", label: "Travel" },
];

/** One quiet fact beside a row's title: its own element on the page. */
export interface MetaPart {
  text: string;
  /** Outlined, like a tag (a jurisdiction). */
  chip?: boolean;
}

/** One objective of a mission. */
export interface Step {
  at: number;
  /** Game text (for RichText). */
  text: string;
  state: "done" | "current" | "open" | "withdrawn";
}

/** A label and a value in a row's details. */
export interface DetailFact {
  label: string;
  value: string;
  mono?: boolean;
}

/** One row of the log. */
export interface LogRow {
  key: string;
  at: number;
  kind: RowKind;
  /** What happened. A name or game text, never an internal id. */
  title: string;
  meta: MetaPart[];
  /** How long it took (a stay, a mission); `null` = nothing to time. */
  took: number | null;
  /** Still going: `took` counts up to now. */
  running: boolean;
  /** What it did to the balance, or what the log says it cost. */
  amount: number | null;
  facets: Facet[];
  steps: Step[];
  facts: DetailFact[];
  /** Which source said what, and when. */
  sources: DetailFact[];
  /** Extra text the search looks at. */
  search: string[];
}

/** The status strip: what is true now (or was, when the session ended). */
export interface StatusNow {
  where: {
    name: string;
    /** The outermost place it lies in (the system). */
    system: string | null;
    jurisdictions: string[];
    /** Inside an armistice zone, as the HUD last said. */
    armistice: boolean;
    arrivedAt: number;
    /** Left it since, and nothing says where to yet. */
    leftAt: number | null;
  } | null;
  ship: {
    /** The vehicle sat in right now (a running session only). */
    aboard: { name: string; since: number } | null;
    last: { name: string; leftAt: number; took: number } | null;
    stays: number;
  };
  mission: {
    running: {
      title: string;
      objective: string | null;
      objectiveNo: number;
      objectiveSince: number | null;
      acceptedAt: number | null;
    } | null;
    last: { title: string; outcome: string; endedAt: number; amount: number | null } | null;
    count: number;
  };
}

export interface SessionReading {
  /** Oldest first. */
  rows: LogRow[];
  status: StatusNow;
  /** Timeline entries read (what the rows were folded from). */
  entries: number;
  /** Balance changes, and how many of them a row explains. */
  changes: number;
  explained: number;
}

const OUTCOME = {
  completed: "completed",
  failed: "failed",
  abandoned: "abandoned",
  withdrawn: "withdrawn",
  ended: "ended",
} as const;

const SOURCE = { log: "Game.log", telemetry: "telemetry", unknown: "an unknown source" } as const;

/** How the log says a ship was destroyed, as words. */
const DESTROYED_BY: Record<string, string> = {
  Combat: "in combat",
  Collision: "in a collision",
  SelfDestruct: "self-destructed",
  ShipItemCritical: "a critical failure",
};

function destroyedBy(reason: string): string {
  return DESTROYED_BY[reason] ?? reason.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
}

const PART: Record<BodyPart, string> = {
  Head: "Head",
  Torso: "Torso",
  LeftArm: "Left arm",
  RightArm: "Right arm",
  LeftLeg: "Left leg",
  RightLeg: "Right leg",
};

/** An injury's tier is the med bed it needs. */
const SEVERITY: Record<number, string> = { 1: "Severe", 2: "Moderate", 3: "Minor" };

function severity(tier: number): string {
  return SEVERITY[tier] ?? `Tier ${tier}`;
}

/** Ships by name, most first: "2 × Razor LX, Mantis". */
export function tally(ships: string[]): string {
  const counts = new Map<string, number>();
  for (const ship of ships) counts.set(ship, (counts.get(ship) ?? 0) + 1);
  return [...counts]
    .sort((a, b) => b[1] - a[1])
    .map(([ship, n]) => (n > 1 ? `${n} × ${ship}` : ship))
    .join(", ");
}

function row(key: string, at: number, kind: RowKind, title: string): LogRow {
  return {
    key,
    at,
    kind,
    title,
    meta: [],
    took: null,
    running: false,
    amount: null,
    facets: [],
    steps: [],
    facts: [],
    sources: [],
    search: [],
  };
}

function sourcesLine(sources: Session["sources"]): string {
  if (sources.log && sources.telemetry) return "Game.log and telemetry";
  if (sources.log) return "Game.log only";
  if (sources.telemetry) return "telemetry only";
  return "no source";
}

/** A weapon's port as a label: `underbarrel_attach` → Underbarrel. */
function attachmentPort(port: string): string {
  const words = port.replace(/_attach$/, "").replaceAll("_", " ").trim();
  return words ? words[0].toUpperCase() + words.slice(1) : "Attachment";
}

function saidBy(entry: Entry): string {
  const hud = entry.locale_dependent ? ", English HUD text" : "";
  return `${clockSeconds(entry.at)}, from ${SOURCE[entry.source]}${hud}`;
}

interface PlaceState {
  row: LogRow;
  name: DisplayName;
  jurisdictions: string[];
  arrivedAt: number;
  /** When you left it: the log writes no line for that, only for the next
   *  place with an inventory, so leaving is read from what comes after (a
   *  jurisdiction change while seated, the armistice zone left, a quantum
   *  course set). Until then, it is where you are. */
  leftAt: number | null;
}

interface StayState {
  row: LogRow;
  name: string;
  className: string;
  enteredAt: number;
  from: string | null;
}

interface MissionState {
  row: LogRow;
  title: string | null;
  acceptedAt: number | null;
  ended: { at: number; outcome: keyof typeof OUTCOME } | null;
  change: WalletChange | null;
  said: Entry;
  /** Ships destroyed while it was the one mission running. */
  kills: string[];
}

/** Ships destroyed with no mission running, close together. */
interface FightState {
  row: LogRow;
  ships: string[];
  last: number;
}

/** A stay on private property; one left and entered again within
 *  [`PROPERTY_GAP`] goes on. */
interface PropertyState {
  row: LogRow;
  leftAt: number | null;
}

const PROPERTY_GAP = 3 * 60_000;

/** Kills further apart than this are two fights. */
const FIGHT_GAP = 10 * 60_000;

/** Fold a session into rows and the status strip. `now` times what still
 *  runs; it matters only for a live session. */
export function readSession(session: Session, names: Names, now: number): SessionReading {
  const end = session.ended_at ?? (session.live ? now : session.last_activity_at);
  const open = session.live;
  const byEntry = new Map<number, WalletChange>();
  // A terminal visit may explain several charges; every other entry one.
  const allByEntry = new Map<number, WalletChange[]>();
  const onTimeline = new Set(session.timeline.map((e) => e.seq));
  const rows: LogRow[] = [];
  let explained = 0;

  for (const change of session.wallet.changes) {
    if (change.cause.type === "entry" && onTimeline.has(change.cause.seq)) {
      byEntry.set(change.cause.seq, change);
      allByEntry.set(change.cause.seq, [...(allByEntry.get(change.cause.seq) ?? []), change]);
      explained++;
      continue;
    }
    const r = row(`w${change.at}:${change.to}`, change.at, "money", "Balance changed");
    r.meta = [{ text: "no log entry explains it" }];
    r.amount = change.delta;
    r.facets = ["money"];
    r.facts = [
      { label: "Before", value: auec(change.from), mono: true },
      { label: "After", value: auec(change.to), mono: true },
    ];
    r.sources = [{ label: "Seen", value: `${clockSeconds(change.at)}, from the telemetry balance` }];
    rows.push(r);
  }

  // The session's own first row.
  const start = row("start", session.started_at, "session", "Session started");
  if (session.shards.length > 0) start.meta.push({ text: "joined a shard" });
  if (session.channel) start.meta.push({ text: session.channel });
  start.meta.push({ text: sourcesLine(session.sources) });
  start.facts = [{ label: "Started", value: dayTime(session.started_at) }];
  if (session.branch) start.facts.push({ label: "Branch", value: session.branch, mono: true });
  if (session.build) start.facts.push({ label: "Build", value: session.build, mono: true });
  if (session.shards[0]) start.facts.push({ label: "Shard", value: session.shards[0].shard, mono: true });
  rows.push(start);

  let place: PlaceState | null = null;
  const places: PlaceState[] = [];
  let stay: StayState | null = null;
  let armistice = false;
  const properties: PropertyState[] = [];
  // Untreated injuries, the worst (lowest) tier per part.
  const injuries = new Map<BodyPart, number>();
  let stays = 0;
  let lastStay: StatusNow["ship"]["last"] = null;
  let jurisdiction: string | null = null;
  let shardJoins = 0;
  const missions: MissionState[] = [];
  const missionById = new Map<string, MissionState>();
  const fights: FightState[] = [];

  const mission = (id: string | null, entry: Entry): MissionState => {
    // An objective without an id belongs to the mission still open.
    const known = id ? missionById.get(id) : [...missions].reverse().find((m) => !m.ended);
    if (known) return known;
    const m: MissionState = {
      row: row(`e${entry.seq}`, entry.at, "mission", "Mission"),
      title: null,
      acceptedAt: null,
      ended: null,
      change: null,
      said: entry,
      kills: [],
    };
    m.row.facets = ["missions"];
    missions.push(m);
    if (id) missionById.set(id, m);
    rows.push(m.row);
    return m;
  };

  const whereFact = (): DetailFact | null =>
    place ? { label: "Where", value: [place.name.name, ...place.name.parents].join(", ") } : null;

  for (const entry of session.timeline) {
    const kind = entry.kind;
    switch (kind.type) {
      case "shard_joined": {
        if (++shardJoins === 1) break; // The first join is the start row's.
        const r = row(`e${entry.seq}`, entry.at, "session", "Changed shard");
        r.facts = [{ label: "Shard", value: kind.shard, mono: true }];
        r.sources = [{ label: "Joined", value: saidBy(entry) }];
        rows.push(r);
        break;
      }
      case "jurisdiction": {
        jurisdiction = kind.jurisdiction;
        // On foot at a place, a change is that place's (Levski has two).
        // In a ship it is the way to the next one; without telemetry there
        // is no telling, so only the one at arrival is kept.
        if (place && !stay && session.sources.telemetry) {
          if (!place.jurisdictions.includes(kind.jurisdiction)) {
            place.jurisdictions.push(kind.jurisdiction);
          }
        }
        // Seated, a change is flying off.
        if (place && stay && place.leftAt == null) place.leftAt = entry.at;
        break;
      }
      case "location": {
        const name = names.place(kind.location);
        const r = row(`e${entry.seq}`, entry.at, "place", name.name);
        r.facets = ["travel"];
        r.sources = [{ label: "Arrived", value: saidBy(entry) }];
        place = {
          row: r,
          name,
          jurisdictions: jurisdiction ? [jurisdiction] : [],
          arrivedAt: entry.at,
          leftAt: null,
        };
        places.push(place);
        rows.push(r);
        break;
      }
      case "mission_accepted": {
        const m = mission(kind.mission_id, entry);
        m.title = kind.title;
        m.acceptedAt = entry.at;
        m.row.at = entry.at;
        break;
      }
      case "objective": {
        const m = mission(kind.mission_id, entry);
        const steps = m.row.steps;
        const same = [...steps].reverse().find((s) => s.text === kind.text && s.state === "open");
        if (kind.change === "New") {
          steps.push({ at: entry.at, text: kind.text, state: "open" });
        } else {
          const state = kind.change === "Completed" ? "done" : "withdrawn";
          if (same) same.state = state;
          else steps.push({ at: entry.at, text: kind.text, state });
        }
        break;
      }
      case "mission_ended": {
        const m = mission(kind.mission_id, entry);
        m.ended = { at: entry.at, outcome: kind.outcome };
        m.title = kind.title ?? m.title ?? (kind.contract ? names.contract(kind.contract).name : null);
        m.change = byEntry.get(entry.seq) ?? null;
        m.said = entry;
        break;
      }
      case "blueprint_received": {
        const r = row(`e${entry.seq}`, entry.at, "mission", "Blueprint received");
        r.meta = [{ text: kind.name }];
        r.facets = ["missions"];
        r.sources = [{ label: "Received", value: saidBy(entry) }];
        rows.push(r);
        break;
      }
      case "refinery_order_completed": {
        const at = names.place(kind.location);
        const r = row(`e${entry.seq}`, entry.at, "other", "Refinery order ready");
        r.meta = [{ text: at.name }, ...at.parents.map((text) => ({ text }))];
        r.sources = [{ label: "Reported", value: saidBy(entry) }];
        rows.push(r);
        break;
      }
      case "shop_purchase": {
        const item = names.item(kind.item_guid).name;
        const shop = names.shop(kind.shop).name;
        const change = byEntry.get(entry.seq) ?? null;
        const r = row(
          `e${entry.seq}`,
          entry.at,
          "buy",
          kind.quantity > 1 ? `${kind.quantity} × ${item}` : item,
        );
        r.meta = [{ text: shop }];
        if (place) r.meta.push({ text: place.name.name });
        if (kind.rental) r.meta.push({ text: "rented" });
        if (kind.outcome === "rejected") r.meta.push({ text: "refused" });
        if (kind.outcome === "requested") r.meta.push({ text: "not confirmed" });
        r.amount = kind.outcome === "rejected" ? null : (change?.delta ?? -kind.price);
        r.facets = ["money"];
        r.facts = [{ label: "Quantity", value: String(kind.quantity), mono: true }];
        if (kind.quantity > 1) {
          r.facts.push({ label: "Each", value: auec(Math.round(kind.price / kind.quantity)), mono: true });
        }
        r.facts.push(
          { label: kind.outcome === "rejected" ? "Asked" : "Paid", value: signedAuec(-kind.price), mono: true },
          { label: "Shop", value: shop },
        );
        const where = whereFact();
        if (where) r.facts.push(where);
        r.sources = [{ label: kind.rental ? "Rented" : "Bought", value: saidBy(entry) }];
        if (change) {
          r.sources.push({
            label: "Charged",
            value: `${clockSeconds(change.at)}, from the telemetry balance`,
          });
        }
        rows.push(r);
        break;
      }
      case "trade": {
        const resource = names.resource(kind.resource_guid).name;
        const shop = names.shop(kind.shop).name;
        const change = byEntry.get(entry.seq) ?? null;
        const bought = kind.side === "Buy";
        const unit = kind.quantity_unit ? ` ${kind.quantity_unit}` : "";
        const r = row(
          `e${entry.seq}`,
          entry.at,
          "buy",
          `${bought ? "Bought" : "Sold"} ${kind.quantity}${unit} ${resource}`,
        );
        r.meta = [{ text: shop }];
        if (place) r.meta.push({ text: place.name.name });
        if (kind.outcome === "rejected") r.meta.push({ text: "refused" });
        // The log has no answer line for commodity trades.
        if (kind.outcome === "requested" && !change) r.meta.push({ text: "not confirmed" });
        const stated = bought ? -kind.price : kind.price;
        r.amount = kind.outcome === "rejected" ? null : (change?.delta ?? stated);
        r.facets = ["money"];
        r.facts = [
          { label: "Quantity", value: `${kind.quantity}${unit}`, mono: true },
          { label: bought ? "Asked to pay" : "Asked for", value: signedAuec(stated), mono: true },
          { label: "Shop", value: shop },
        ];
        const where = whereFact();
        if (where) r.facts.push(where);
        r.sources = [{ label: bought ? "Bought" : "Sold", value: saidBy(entry) }];
        if (change) {
          r.sources.push({
            label: bought ? "Charged" : "Paid out",
            value: `${clockSeconds(change.at)}, from the telemetry balance`,
          });
        }
        rows.push(r);
        break;
      }
      case "money": {
        const awarded = kind.change === "Awarded";
        const change = byEntry.get(entry.seq) ?? null;
        const r = row(`e${entry.seq}`, entry.at, "money", awarded ? "Awarded" : "Fined");
        r.amount = change?.delta ?? (awarded ? kind.amount : -kind.amount);
        r.facets = ["money"];
        r.sources = [{ label: awarded ? "Awarded" : "Fined", value: saidBy(entry) }];
        if (change) {
          r.sources.push({
            label: "Balance",
            value: `${clockSeconds(change.at)}, from the telemetry balance`,
          });
        }
        rows.push(r);
        break;
      }
      case "vehicle_terminal": {
        // A visit that paid for nothing is just a look at the fleet.
        const charges = allByEntry.get(entry.seq);
        if (!charges) break;
        const r = row(`e${entry.seq}`, entry.at, "buy", "Ship services");
        if (charges.length > 1) r.meta.push({ text: count(charges.length, "charge") });
        if (place) r.meta.push({ text: place.name.name });
        r.amount = charges.reduce((sum, c) => sum + c.delta, 0);
        r.facets = ["money"];
        r.facts = charges.map((c) => ({ label: clockSeconds(c.at), value: signedAuec(c.delta), mono: true }));
        const where = whereFact();
        if (where) r.facts.push(where);
        r.sources = [
          { label: "Terminal", value: saidBy(entry) },
          { label: "Charged", value: `${clockSeconds(charges[0].at)}, from the telemetry balance` },
        ];
        rows.push(r);
        break;
      }
      case "vehicle_entered": {
        const name = names.vehicle(kind.class_name).name;
        const r = row(`e${entry.seq}`, entry.at, "ship", name);
        r.facets = ["travel"];
        r.sources = [{ label: "Entered", value: saidBy(entry) }];
        stay = {
          row: r,
          name,
          className: kind.class_name,
          enteredAt: entry.at,
          from: place?.name.name ?? null,
        };
        stays++;
        rows.push(r);
        break;
      }
      case "vehicle_left": {
        const name = names.vehicle(kind.class_name).name;
        if (!stay) {
          // Entered before this session's timeline begins.
          const r = row(`e${entry.seq}`, entry.at, "ship", name);
          r.meta = [{ text: "left" }];
          r.facets = ["travel"];
          r.sources = [{ label: "Left", value: saidBy(entry) }];
          rows.push(r);
          stays++;
          lastStay = { name, leftAt: entry.at, took: 0 };
          break;
        }
        closeStay(stay, entry.at, place?.name.name ?? null, `until ${clock(entry.at)}`);
        stay.row.sources.push({ label: "Left", value: saidBy(entry) });
        lastStay = { name: stay.name, leftAt: entry.at, took: entry.at - stay.enteredAt };
        stay = null;
        break;
      }
      case "ship_destroyed": {
        const ship = names.vehicle(kind.class_name).name;
        // A kill is a mission's when that mission is the only one running.
        const running = missions.filter((m) => !m.ended && m.acceptedAt != null);
        if (running.length === 1) {
          running[0].kills.push(ship);
          break;
        }
        let fight = fights.at(-1);
        if (!fight || entry.at - fight.last > FIGHT_GAP) {
          fight = { row: row(`e${entry.seq}`, entry.at, "ship", ""), ships: [], last: entry.at };
          fight.row.sources = [{ label: "First", value: saidBy(entry) }];
          fights.push(fight);
          rows.push(fight.row);
        }
        fight.ships.push(ship);
        fight.last = entry.at;
        break;
      }
      case "ship_lost": {
        const ship = names.vehicle(kind.class_name).name;
        const r = row(`e${entry.seq}`, entry.at, "life", `Lost ${ship}`);
        r.meta = [{ text: destroyedBy(kind.reason) }];
        if (kind.by) r.meta.push({ text: `by ${kind.by}` });
        if (place) r.meta.push({ text: place.name.name });
        r.facts = [
          { label: "Ship", value: ship },
          { label: "How", value: destroyedBy(kind.reason) },
        ];
        if (kind.by) r.facts.push({ label: "By", value: kind.by });
        r.sources = [{ label: "Destroyed", value: saidBy(entry) }];
        rows.push(r);
        break;
      }
      case "zone": {
        if (kind.zone === "Armistice") {
          armistice = kind.entered;
          if (!kind.entered && place && place.leftAt == null) place.leftAt = entry.at;
          break;
        }
        const last = properties.at(-1);
        if (!kind.entered) {
          if (last && last.leftAt == null) last.leftAt = entry.at;
          break;
        }
        if (last && (last.leftAt == null || entry.at - last.leftAt <= PROPERTY_GAP)) {
          last.leftAt = null;
          break;
        }
        const r = row(`e${entry.seq}`, entry.at, "place", "Private property");
        if (stay) r.meta.push({ text: `in ${stay.name}` });
        if (place) r.meta.push({ text: `near ${place.name.name}` });
        r.facets = ["travel"];
        r.sources = [{ label: "Entered", value: saidBy(entry) }];
        properties.push({ row: r, leftAt: null });
        rows.push(r);
        break;
      }
      case "injured": {
        const part = PART[kind.part];
        const r = row(`e${entry.seq}`, entry.at, "life", `${severity(kind.tier)} injury`);
        r.meta = [{ text: part }, { text: `tier ${kind.tier}` }];
        if (place) r.meta.push({ text: place.name.name });
        r.facts = [
          { label: "Part", value: part },
          { label: "Needs", value: `a tier ${kind.tier} med bed or better` },
        ];
        const where = whereFact();
        if (where) r.facts.push(where);
        r.sources = [{ label: "Reported", value: saidBy(entry) }];
        rows.push(r);
        injuries.set(kind.part, Math.min(kind.tier, injuries.get(kind.part) ?? kind.tier));
        break;
      }
      case "treated": {
        const ship = kind.vehicle ? names.vehicle(kind.vehicle).name : null;
        const r = row(`e${entry.seq}`, entry.at, "life", "Treated at a med bed");
        const parts = kind.parts.map((p) => PART[p]);
        if (parts.length > 0) {
          r.meta.push({ text: [parts[0], ...parts.slice(1).map((p) => p.toLowerCase())].join(", ") });
        }
        if (ship) r.meta.push({ text: `in ${ship}` });
        else if (place) r.meta.push({ text: place.name.name });
        r.facts = kind.parts.map((p) => {
          const tier = injuries.get(p);
          injuries.delete(p);
          return { label: PART[p], value: tier != null ? `${severity(tier)}, tier ${tier}` : "operated on" };
        });
        if (ship) r.facts.push({ label: "Ship", value: ship });
        const where = whereFact();
        if (where) r.facts.push(where);
        r.sources = [{ label: "Surgery", value: saidBy(entry) }];
        rows.push(r);
        break;
      }
      case "died":
      case "downed":
      case "respawned":
      case "spawned": {
        // A new body has none of the old one's injuries.
        if (kind.type === "died") injuries.clear();
        const title = { died: "Died", downed: "Downed", respawned: "Respawned", spawned: "Spawned" };
        const r = row(`e${entry.seq}`, entry.at, "life", title[kind.type]);
        if (place) r.meta = [{ text: place.name.name }];
        r.sources = [{ label: "Seen", value: saidBy(entry) }];
        rows.push(r);
        break;
      }
      case "quantum_travel": {
        if (place && place.leftAt == null) place.leftAt = entry.at;
        const to = names.destination(kind.to).name;
        const arrived = kind.arrived_at;
        // Without an arrival the log only says a course was set.
        const r = row(
          `e${entry.seq}`,
          entry.at,
          "ship",
          arrived != null ? `Quantum travel to ${to}` : `Course set for ${to}`,
        );
        if (kind.from) r.meta.push({ text: `from ${kind.from}` });
        const ship = kind.vehicle ? names.vehicle(kind.vehicle).name : null;
        if (ship) r.meta.push({ text: ship });
        if (arrived == null) r.meta.push({ text: "no arrival seen" });
        r.facets = ["travel"];
        if (kind.from) r.facts.push({ label: "From", value: kind.from });
        r.facts.push({ label: "To", value: to });
        if (ship) r.facts.push({ label: "Ship", value: ship });
        r.facts.push({ label: "Course set", value: clockSeconds(entry.at), mono: true });
        r.sources = [{ label: "Course set", value: saidBy(entry) }];
        if (arrived != null) {
          r.took = arrived - entry.at;
          r.facts.push(
            { label: "Arrived", value: clockSeconds(arrived), mono: true },
            // The log has no line for the jump itself.
            { label: "Course set to arrival", value: duration(arrived - entry.at) },
          );
          r.sources.push({ label: "Arrived", value: `${clockSeconds(arrived)}, from Game.log` });
        }
        rows.push(r);
        break;
      }
      case "attachments": {
        const fitted = kind.items.map((item) => ({
          name: names.itemClass(item.item).name,
          port: attachmentPort(item.port),
        }));
        const weapon = kind.weapon ? names.itemClass(kind.weapon).name : null;
        const r = row(
          `e${entry.seq}`,
          entry.at,
          "other",
          fitted.length === 1 ? `Fitted ${fitted[0].name}` : `Fitted ${fitted.length} attachments`,
        );
        if (weapon) r.meta.push({ text: `on ${weapon}` });
        if (fitted.length > 1) r.meta.push(...fitted.map((f) => ({ text: f.name })));
        if (weapon) r.facts.push({ label: "Weapon", value: weapon });
        r.facts.push(...fitted.map((f) => ({ label: f.port, value: f.name })));
        const where = whereFact();
        if (where) r.facts.push(where);
        // The inventory's request: the log has no answer line for it.
        r.sources = [{ label: "Asked for", value: saidBy(entry) }];
        const last = kind.items.at(-1);
        if (last && last.at > entry.at) {
          r.sources.push({ label: "Last one", value: `${clockSeconds(last.at)}, from Game.log` });
        }
        rows.push(r);
        break;
      }
      case "unknown": {
        // Written by a newer Starlume than this one.
        rows.push(row(`e${entry.seq}`, entry.at, "other", "Not shown by this version"));
        break;
      }
    }
  }

  // A place's jurisdiction words can grow until the next place, so its
  // quiet parts are written now: what it lies in, then its jurisdictions.
  for (const p of places) {
    p.row.meta = [
      ...p.name.parents.map((text) => ({ text })),
      ...p.jurisdictions.map((text) => ({ text, chip: true })),
    ];
    if (p.name.parents.length > 0) {
      p.row.facts.push({ label: "Lies in", value: p.name.parents.join(", ") });
    }
    if (p.jurisdictions.length > 0) {
      p.row.facts.push({ label: "Jurisdiction", value: p.jurisdictions.join(", ") });
    }
  }

  let aboard: StatusNow["ship"]["aboard"] = null;
  if (stay) {
    if (open) {
      stay.row.running = true;
      stay.row.took = now - stay.enteredAt;
      stay.row.meta = [{ text: "aboard" }];
      if (stay.from) stay.row.meta.push({ text: `from ${stay.from}` });
      aboard = { name: stay.name, since: stay.enteredAt };
    } else {
      closeStay(stay, end, place?.name.name ?? null, "until the session ended");
      lastStay = { name: stay.name, leftAt: end, took: end - stay.enteredAt };
    }
  }

  for (const m of missions) finishMission(m, open, now);

  for (const p of properties) {
    const r = p.row;
    r.facts = [{ label: "Entered", value: clockSeconds(r.at), mono: true }];
    if (p.leftAt != null) {
      r.took = p.leftAt - r.at;
      r.facts.push(
        { label: "Left", value: clockSeconds(p.leftAt), mono: true },
        { label: "Stayed", value: duration(p.leftAt - r.at), mono: true },
      );
      r.sources.push({ label: "Left", value: `${clockSeconds(p.leftAt)}, from Game.log` });
    } else {
      r.meta.push({ text: open ? "inside" : "no leaving seen" });
    }
  }

  for (const f of fights) {
    const r = f.row;
    r.title = f.ships.length === 1 ? `Destroyed ${f.ships[0]}` : `Destroyed ${f.ships.length} ships`;
    if (f.ships.length > 1) r.meta = [{ text: tally(f.ships) }];
    r.took = f.ships.length > 1 ? f.last - r.at : null;
    r.facts = [{ label: "Destroyed", value: tally(f.ships) }];
    if (f.ships.length > 1) r.sources.push({ label: "Last", value: `${clockSeconds(f.last)}, from Game.log` });
  }

  if (!open) {
    const ended = row("end", end, "session", "Session ended");
    ended.meta = [{ text: duration(end - session.started_at) }];
    if (session.shards.length > 0) ended.meta.push({ text: count(session.shards.length, "shard") });
    ended.facts = [{ label: "Ended", value: dayTime(end) }];
    if (session.end_reason) ended.facts.push({ label: "Reason", value: session.end_reason });
    else ended.facts.push({ label: "Reason", value: "the game was no longer seen" });
    rows.push(ended);
  }

  // Stable: rows of one instant keep the order they were read in.
  rows.sort((a, b) => a.at - b.at);
  for (const r of rows) {
    r.search = [...r.meta.map((m) => m.text), ...r.steps.map((s) => s.text), ...r.facts.map((f) => f.value)];
  }

  const running = open ? [...missions].reverse().find((m) => !m.ended) : undefined;
  const lastEnded = missions
    .filter((m) => m.ended)
    .sort((a, b) => b.ended!.at - a.ended!.at)[0];
  const current = running
    ? ([...running.row.steps].reverse().find((s) => s.state === "current") ?? null)
    : null;

  return {
    rows,
    entries: session.timeline.length,
    changes: session.wallet.changes.length,
    explained,
    status: {
      where: place
        ? {
            name: place.name.name,
            system: place.name.parents.at(-1) ?? null,
            jurisdictions: place.jurisdictions,
            armistice,
            arrivedAt: place.arrivedAt,
            leftAt: place.leftAt,
          }
        : null,
      ship: { aboard, last: lastStay, stays },
      mission: {
        running: running
          ? {
              title: running.row.title,
              objective: current?.text ?? null,
              objectiveNo: running.row.steps.length,
              objectiveSince: current?.at ?? null,
              acceptedAt: running.acceptedAt,
            }
          : null,
        last: lastEnded
          ? {
              title: lastEnded.row.title,
              outcome: OUTCOME[lastEnded.ended!.outcome],
              endedAt: lastEnded.ended!.at,
              amount: lastEnded.change?.delta ?? null,
            }
          : null,
        count: missions.length,
      },
    },
  };

  // ── helpers that close what the loop opened ──

  function closeStay(s: StayState, at: number, to: string | null, until: string) {
    s.row.took = at - s.enteredAt;
    s.row.meta = [{ text: until }];
    const route = s.from && to && s.from !== to ? `${s.from} → ${to}` : (to ?? s.from);
    if (route) s.row.meta.push({ text: route });
    s.row.facts = [
      { label: "Entered", value: clockSeconds(s.enteredAt), mono: true },
      { label: "Left", value: clockSeconds(at), mono: true },
      { label: "In it", value: duration(at - s.enteredAt) },
    ];
    if (s.from) s.row.facts.push({ label: "Boarded at", value: s.from });
    if (to && to !== s.from) s.row.facts.push({ label: "Left at", value: to });
  }
}

function finishMission(m: MissionState, open: boolean, now: number) {
  const r = m.row;
  const steps = r.steps;
  r.title = m.title ?? "Mission";
  steps.forEach((step, i) => {
    if (step.state !== "open") return;
    const last = i === steps.length - 1;
    // A new objective follows the one before it: that one is done.
    if (!last) step.state = "done";
    else if (m.ended) step.state = m.ended.outcome === "completed" ? "done" : "open";
    else if (open) step.state = "current";
  });
  const started = m.acceptedAt ?? r.at;
  if (m.ended) {
    r.meta = [{ text: OUTCOME[m.ended.outcome] }];
    if (steps.length > 0) r.meta.push({ text: count(steps.length, "objective") });
    if (m.kills.length > 0) r.meta.push({ text: count(m.kills.length, "kill") });
    r.took = m.ended.at - started;
    r.amount = m.change?.delta ?? null;
    if (r.amount != null) r.facets = ["missions", "money"];
  } else if (open) {
    r.meta = [{ text: "running" }];
    if (steps.length > 0) r.meta.push({ text: `objective ${steps.length}` });
    if (m.kills.length > 0) r.meta.push({ text: count(m.kills.length, "kill") });
    r.took = now - started;
    r.running = true;
  } else {
    r.meta = [{ text: "open when the session ended" }];
    if (steps.length > 0) r.meta.push({ text: count(steps.length, "objective") });
    if (m.kills.length > 0) r.meta.push({ text: count(m.kills.length, "kill") });
  }
  r.facts = [];
  if (m.acceptedAt != null) r.facts.push({ label: "Accepted", value: clock(m.acceptedAt), mono: true });
  if (m.ended) {
    r.facts.push(
      { label: "Ended", value: clock(m.ended.at), mono: true },
      { label: "Took", value: duration(m.ended.at - started), mono: true },
      { label: "Outcome", value: OUTCOME[m.ended.outcome] },
      {
        label: "Reward",
        value: m.change ? signedAuec(m.change.delta) : "no payment seen",
        mono: m.change != null,
      },
    );
  } else if (open) {
    r.facts.push(
      { label: "Running", value: duration(now - started), mono: true },
      { label: "Reward", value: "shows when it pays" },
    );
  }
  if (m.kills.length > 0) r.facts.push({ label: "Destroyed", value: tally(m.kills) });
  r.sources = [{ label: "Read from", value: SOURCE[m.said.source] }];
  if (m.said.locale_dependent || steps.length > 0) {
    r.sources.push({ label: "Needs", value: "the game's English HUD text" });
  }
  if (m.change) {
    r.sources.push({
      label: "Paid",
      value: `${clockSeconds(m.change.at)}, from the telemetry balance`,
    });
  }
}

// ── The wallet line ─────────────────────────────────────────────────────────

/** A tick under the wallet line. */
export interface Tick {
  label: string;
  /** 0..1 along the session. */
  pos: number;
}

/** The balance over the session as a step line, in a `width` × `height`
 *  box (SVG user units). */
export interface WalletLine {
  /** The step line. */
  step: string;
  /** The area between the line and the opening balance. */
  area: string;
  /** The opening balance's height. */
  base: number;
  ticks: Tick[];
}

/** A round time near the middle of the span, when one exists. */
function middleTick(start: number, end: number): Tick | null {
  const span = end - start;
  if (span < 3 * 60_000) return null;
  const mid = start + span / 2;
  // Local wall-clock rounding: the zone's offset at the midpoint.
  const offset = new Date(mid).getTimezoneOffset() * 60_000;
  for (const minutes of [120, 60, 30, 15, 10, 5, 1]) {
    const unit = minutes * 60_000;
    const at = Math.round((mid - offset) / unit) * unit + offset;
    const pos = (at - start) / span;
    if (pos >= 0.3 && pos <= 0.7) return { label: clock(at), pos };
  }
  return null;
}

/** The wallet's step line and its (at most three) time ticks. `null` when
 *  no balance was read. */
export function walletLine(
  session: Session,
  now: number,
  width: number,
  height: number,
): WalletLine | null {
  const wallet = session.wallet;
  const first = wallet.changes[0]?.from ?? wallet.opening?.balance ?? wallet.latest?.balance;
  if (first == null) return null;
  const start = session.started_at;
  const end = Math.max(session.ended_at ?? (session.live ? now : session.last_activity_at), start + 1);
  const balances = [first, ...wallet.changes.map((c) => c.to)];
  const max = Math.max(...balances);
  const min = Math.min(...balances);
  const pad = 2;
  const x = (at: number) => +(Math.min(1, Math.max(0, (at - start) / (end - start))) * width).toFixed(1);
  // A flat line sits in the middle; otherwise the range fills the box.
  const y = (balance: number) =>
    +(max === min ? height / 2 : pad + ((max - balance) / (max - min)) * (height - 2 * pad)).toFixed(1);
  const base = y(first);
  let step = `M0 ${base}`;
  for (const change of wallet.changes) step += ` H${x(change.at)} V${y(change.to)}`;
  step += ` H${width}`;
  const ticks: Tick[] = [{ label: clock(start), pos: 0 }];
  const middle = middleTick(start, end);
  if (middle) ticks.push(middle);
  ticks.push({ label: session.live ? `now ${clock(end)}` : clock(end), pos: 1 });
  return { step, area: `${step} V${base} H0 Z`, base, ticks };
}

// ── History ─────────────────────────────────────────────────────────────────

/** Which of the two sources a session is missing, as the page says it. */
export function missingSources(sources: { log: boolean; telemetry: boolean }): string | null {
  if (!sources.log && !sources.telemetry) return null;
  if (!sources.log) return "telemetry only";
  if (!sources.telemetry) return "Game.log only";
  return null;
}

/** A history row's quiet facts, each its own element. */
export function summaryParts(s: SessionSummary): string[] {
  const parts = [duration(sessionLength(s, s.started_at))];
  if (s.missions_completed > 0) {
    parts.push(`${count(s.missions_completed, "mission")} completed`);
  }
  if (s.ships_destroyed > 0) parts.push(count(s.ships_destroyed, "kill"));
  if (s.ships_lost > 0) parts.push(`${count(s.ships_lost, "ship")} lost`);
  if (s.shards.length > 0) parts.push(count(s.shards.length, "shard"));
  return parts;
}
