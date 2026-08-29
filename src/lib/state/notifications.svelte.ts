// The notification funnel — now @veelume/ui's store (which is this app's
// own design, distilled); this module is the Tauri transport adapter plus
// re-exports so consumers keep one import path.
//
// The session log's source of truth stays **Rust-side** (notify.rs
// `NotifLog`): while the window is hidden the webview is suspended and runs
// no JS, so events raised then never reach the store live. The store
// hydrates from `recent_notifications` on mount and re-syncs on window
// focus; live `notify` events cover the visible case. The kit's keyed
// `ingest` dedupes the overlap between the live stream and the catch-up
// sweep — hydrated backlog entries keep their unread state but never toast.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ingest } from "@veelume/ui";
import { commands, type NotificationRecord } from "$lib/bindings";

export {
  clearAll,
  dismiss,
  isSticky,
  markAllRead,
  notifications,
  notify,
} from "@veelume/ui";
export type { NotifAction, NotifInput, NotifLevel, Notification } from "@veelume/ui";

function ingestRecord(r: NotificationRecord, toast: boolean) {
  ingest(
    {
      key: `b${r.id}`,
      ts: r.ts,
      level: r.level,
      title: r.title,
      body: r.body ?? null,
      action: r.action ?? null,
      source: r.source ?? null,
    },
    { toast },
  );
}

/**
 * Pull the backend session log and merge anything we haven't seen (raised
 * while the webview was suspended, or before this view mounted). Call on
 * mount and on window focus. Dismissed entries stay dismissed — their keys
 * remain in the kit's seen-set, so a re-sweep is a no-op for them.
 */
export async function syncNotifications(): Promise<void> {
  const records = await commands.recentNotifications();
  for (const r of records) {
    ingestRecord(r, false);
  }
}

/** Subscribe to live backend `notify` events. Call once, in the root layout. */
export function listenForNotifications(): Promise<UnlistenFn> {
  return listen<NotificationRecord>("notify", (event) => {
    ingestRecord(event.payload, true);
  });
}
