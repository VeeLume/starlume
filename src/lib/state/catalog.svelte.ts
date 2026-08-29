// Catalog caches (docs/frontend.md rule 1): the reference datasets are
// fetched once per channel and kept here — a tab switch renders instantly
// from cache instead of refetching. Browse state moved to the kit's
// URL-backed `createBrowseState` per page (missions/resources/
// manufacturers); only the server-paged items page keeps a store-side
// browse (its results are backend queries, not a client pipeline).
//
// Invalidation: the layout calls `invalidateCatalogs()` on the backend's
// `data:changed` event (a load may have cooked a new build).

import {
  commands,
  type ItemRowView,
  type ItemTypeFacetView,
  type ResourceRowView,
  type ManufacturerRowView,
  type MissionEntryView,
} from "$lib/bindings";

/** Per-channel cached reference datasets. */
type Datasets = {
  items?: ItemRowView[];
  types?: ItemTypeFacetView[];
  resources?: ResourceRowView[];
  manufacturers?: ManufacturerRowView[];
  missions?: MissionEntryView[];
};

let _datasets = $state<Record<string, Datasets>>({});
/** Inflight de-dup so a prefetch and a page mount don't double-fetch. */
const _inflight = new Map<string, Promise<unknown>>();

function cached<K extends keyof Datasets>(
  channel: string,
  key: K,
  fetcher: () => Promise<Datasets[K] | string>,
): Promise<NonNullable<Datasets[K]> | string> {
  const hit = _datasets[channel]?.[key];
  if (hit) return Promise.resolve(hit as NonNullable<Datasets[K]>);
  const inflightKey = `${channel}:${key}`;
  const running = _inflight.get(inflightKey);
  if (running) return running as Promise<NonNullable<Datasets[K]> | string>;
  const p = fetcher()
    .then((r) => {
      if (typeof r !== "string") {
        _datasets[channel] = { ..._datasets[channel], [key]: r };
      }
      return r as NonNullable<Datasets[K]> | string;
    })
    .finally(() => _inflight.delete(inflightKey));
  _inflight.set(inflightKey, p);
  return p;
}

const unwrap = <T>(r: { status: "ok"; data: T } | { status: "error"; error: { message: string } }) =>
  r.status === "ok" ? r.data : r.error.message;

export function getItemsAll(channel: string) {
  return cached(channel, "items", async () => unwrap(await commands.dataItemsAll(channel)));
}
export function getItemTypes(channel: string) {
  return cached(channel, "types", async () => unwrap(await commands.dataItemTypes(channel)));
}
export function getResources(channel: string) {
  return cached(channel, "resources", async () => unwrap(await commands.dataResources(channel)));
}
export function getManufacturers(channel: string) {
  return cached(channel, "manufacturers", async () =>
    unwrap(await commands.dataManufacturers(channel)),
  );
}
export function getMissions(channel: string) {
  return cached(channel, "missions", async () => unwrap(await commands.dataMissions(channel)));
}

/** Kick off every catalog fetch for a channel in the background (startup
 *  hydration, rule 3). Errors are ignored — pages surface them on demand. */
export function prefetchCatalogs(channel: string): void {
  void getItemsAll(channel);
  void getResources(channel);
  void getManufacturers(channel);
  void getMissions(channel);
}

/** Drop every cached dataset (new build cooked / cache wiped). */
export function invalidateCatalogs(): void {
  _datasets = {};
}
