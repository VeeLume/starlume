// Global notification store — the single frontend funnel (the Hearth
// pattern, plus a `source` field naming the module/service that raised it).
//
// The session log's source of truth is **Rust-side** (notify.rs `NotifLog`):
// while the window is hidden the webview is suspended and runs no JS, so
// events raised then never reach this store live. Instead the store hydrates
// from `recent_notifications` on mount and re-syncs on window focus; live
// `notify` events cover the visible case. Hydrated entries keep their unread
// state but don't pop toasts (returning to a wall of stale toasts is noise —
// the bell badge + native toasts already covered them).

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type NotificationRecord } from "$lib/bindings";

export type NotifLevel = "info" | "success" | "warning" | "error";

export type NotifAction = { label: string; href: string };

/** Input for purely-frontend notifications. */
export type NotifPayload = {
  level: NotifLevel;
  title: string;
  body?: string | null;
  action?: NotifAction | null;
  source?: string | null;
};

/** A notification once it's in the store. */
export type Notification = NotifPayload & {
  /** Store key: `b<id>` for backend records, `l<n>` for local ones. */
  id: string;
  ts: number;
  read: boolean;
  /** Whether the toast stack should surface it (false for hydrated backlog). */
  popToast: boolean;
};

/** Levels whose toasts persist until dismissed (vs auto-fading). */
const STICKY: Record<NotifLevel, boolean> = {
  info: false,
  success: false,
  warning: true,
  error: true,
};

/** Keep the in-memory log bounded (matches the backend ring buffer). */
const MAX_ITEMS = 100;

let items = $state<Notification[]>([]);
let localCounter = 0;

/** Reactive read access. Components use `notifications.items` / `.unread`. */
export const notifications = {
  get items() {
    return items;
  },
  get unread() {
    return items.reduce((acc, n) => acc + (n.read ? 0 : 1), 0);
  },
  get hasUnread() {
    return items.some((n) => !n.read);
  },
};

function insert(n: Notification) {
  items = [n, ...items].sort((a, b) => b.ts - a.ts).slice(0, MAX_ITEMS);
}

function fromRecord(r: NotificationRecord, popToast: boolean): Notification {
  return {
    id: `b${r.id}`,
    ts: r.ts,
    read: false,
    popToast,
    level: r.level,
    title: r.title,
    body: r.body ?? null,
    action: r.action ?? null,
    source: r.source ?? null,
  };
}

/** Add a purely-frontend notification (backend ones arrive via the event). */
export function notify(input: NotifPayload): Notification {
  const n: Notification = {
    id: `l${localCounter++}`,
    ts: Date.now(),
    read: false,
    popToast: true,
    level: input.level,
    title: input.title,
    body: input.body ?? null,
    action: input.action ?? null,
    source: input.source ?? null,
  };
  insert(n);
  return n;
}

export function dismiss(id: string) {
  items = items.filter((n) => n.id !== id);
}

export function markAllRead() {
  if (items.some((n) => !n.read)) {
    items = items.map((n) => (n.read ? n : { ...n, read: true }));
  }
}

export function clearAll() {
  items = [];
}

/** Whether a level's toast should stay until dismissed. */
export function isSticky(level: NotifLevel): boolean {
  return STICKY[level];
}

/**
 * Pull the backend session log and merge anything we haven't seen (raised
 * while the webview was suspended, or before this view mounted). Call on
 * mount and on window focus. Dismissed entries stay dismissed: sync only
 * adds records newer than the newest backend record we've ever seen.
 */
let highestSeenBackendId = -1;

export async function syncNotifications(): Promise<void> {
  const records = await commands.recentNotifications();
  for (const r of records) {
    if (r.id <= highestSeenBackendId) continue;
    highestSeenBackendId = Math.max(highestSeenBackendId, r.id);
    insert(fromRecord(r, false));
  }
}

/** Subscribe to live backend `notify` events. Call once, in the root layout. */
export function listenForNotifications(): Promise<UnlistenFn> {
  return listen<NotificationRecord>("notify", (event) => {
    const r = event.payload;
    if (r.id <= highestSeenBackendId) return; // already hydrated via sync
    highestSeenBackendId = Math.max(highestSeenBackendId, r.id);
    insert(fromRecord(r, true));
  });
}
