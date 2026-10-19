// Background activity — what the app is doing right now (the activity.rs
// registry). One store, three kinds of reader: the kit's notification
// center ("Now" section + the bell's busy mark, fed via setActivities), the
// inline surfaces that filter by kind + channel (catalog card, langpatch
// install row), and nothing else — pages never track
// background work themselves.
//
// The source of truth is Rust-side (frontend rule 6: a suspended webview
// runs no JS). Every change arrives as a whole snapshot on
// `activity:changed`; the root layout re-syncs on mount and window focus.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { setActivities } from "@veelume/ui";
import {
  commands,
  type ActivityKind,
  type ActivityRecord,
} from "$lib/bindings";

let _items = $state<ActivityRecord[]>([]);

export const activityStore = {
  get items() {
    return _items;
  },
};

// The notification `source` tag each kind renders with in the center.
const SOURCE: Record<ActivityKind, string> = {
  "data-load": "data",
  langpatch: "langpatch",
  blueprints: "blueprints",
  inventory: "items",
  devtracker: "devtracker",
  "patch-notes": "patchnotes",
  "comm-links": "commlinks",
};

function apply(records: ActivityRecord[]) {
  _items = records;
  setActivities(
    records.map((r) => ({
      key: String(r.id),
      title: r.title,
      phase: r.phase,
      started: r.started,
      source: SOURCE[r.kind],
    })),
  );
}

/**
 * The activity an inline surface shows for one channel: the one about that
 * channel, else a channel-less one of the same kind (a pack download or the
 * wait for SC to exit holds back every install). `channel` is compared
 * case-insensitively — pages carry display casing ("Live"), records the key.
 */
export function activityFor(
  kind: ActivityKind,
  channel: string,
): ActivityRecord | undefined {
  const key = channel.toLowerCase();
  return (
    _items.find((a) => a.kind === kind && a.channel === key) ??
    _items.find((a) => a.kind === kind && a.channel === null)
  );
}

/** The one-line label for an inline surface: the stage, the wait reason,
 *  or the title for stage-less work. */
export function activityLabel(a: ActivityRecord): string {
  if (a.phase.type === "waiting") return a.phase.reason;
  return a.phase.stage ?? a.title;
}

/** Re-read the backend snapshot. Call on mount and on window focus. */
export async function syncActivities(): Promise<void> {
  apply(await commands.currentActivities());
}

/** Subscribe to live snapshots. Call once, in the root layout. */
export function listenForActivities(): Promise<UnlistenFn> {
  return listen<ActivityRecord[]>("activity:changed", (event) =>
    apply(event.payload),
  );
}
