// Text Patching (langpatch) module state — the /langpatch page renders this
// cache synchronously (frontend rule "stores own data"); `langpatch:changed` from
// the backend (reconcile finished, patch applied/removed) triggers a
// refresh while the page listens.

import {
  commands,
  type LangpatchOverview,
  type LangpatchConfigUpdate,
  type MissionPreviewView,
} from "$lib/bindings";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

let _overview = $state<LangpatchOverview | null>(null);
let _busy = $state(false);
let _error = $state<string | null>(null);
let _savedRev = $state(0);
// The preview pane's few missions, kept across visits (frontend rules 1–2):
// the last sample renders at once, the re-read runs behind it.
let _sample = $state<MissionPreviewView[] | null>(null);
let _sampleChannel: string | null = null;
let _sampleBusy = $state(false);
let _sampleError = $state<string | null>(null);
// Guards against an older response landing after a newer request.
let _sampleReq = 0;

export const langpatchStore = {
  get overview() {
    return _overview;
  },
  /** Bumps once a config edit is saved on disk — the point where anything
   *  derived Rust-side from the config (the preview) can be re-read. The
   *  overview itself changes earlier, optimistically. */
  get savedRev() {
    return _savedRev;
  },
  /** A config save / manual apply / remove is in flight. */
  get busy() {
    return _busy;
  },
  get error() {
    return _error;
  },
  /** The mission preview sample; null until the first read lands. */
  get sample() {
    return _sample;
  },
  /** A sample re-read is in flight (the derive takes seconds). */
  get sampleBusy() {
    return _sampleBusy;
  },
  get sampleError() {
    return _sampleError;
  },
};

/** Re-read the preview sample under the current saved config. The backend
 *  picks the missions per build, not per config, so an option's effect
 *  shows on text already on screen. */
export async function loadPreviewSample(channel: string): Promise<void> {
  if (channel !== _sampleChannel) {
    _sample = null;
    _sampleChannel = channel;
  }
  const mine = ++_sampleReq;
  _sampleBusy = true;
  try {
    const r = await commands.langpatchPreviewSample(channel);
    if (mine !== _sampleReq) return;
    if (r.status === "ok") {
      _sample = r.data;
      _sampleError = null;
    } else {
      _sampleError = r.error.message;
    }
  } catch (e) {
    if (mine === _sampleReq) _sampleError = String(e);
  } finally {
    if (mine === _sampleReq) _sampleBusy = false;
  }
}

export async function loadLangpatch(): Promise<void> {
  const result = await commands.langpatchOverview();
  if (result.status === "ok") {
    _overview = result.data;
    _error = null;
  } else {
    _error = result.error.message;
  }
}

/** Backend push: reconcile ran, a patch was applied/removed. Ignored while
 *  config edits are still being flushed — a reload then would snap the
 *  controls back to an older saved config; the flush reloads at its end. */
export function listenForLangpatchChanges(): Promise<UnlistenFn> {
  return listen("langpatch:changed", () => {
    if (!_flushing) void loadLangpatch();
  });
}

/** Build a full config update from the current overview (the IPC takes the
 *  whole config — one shape for every edit). */
export function updateFromOverview(o: LangpatchOverview): LangpatchConfigUpdate {
  return {
    auto_patch: o.auto_patch,
    channels: [...o.channels],
    language_pack: o.language_pack,
    patchers: Object.fromEntries(
      o.patchers.map((p) => [
        p.id,
        { enabled: p.enabled, options: { ...p.values } },
      ]),
    ),
  };
}

/** Mirror a config edit into the cached overview, so the page shows it on
 *  the click instead of after the save round-trip. Install patch states
 *  stay as they are until the backend reports back. */
function withUpdate(o: LangpatchOverview, u: LangpatchConfigUpdate): LangpatchOverview {
  return {
    ...o,
    auto_patch: u.auto_patch,
    channels: [...u.channels],
    language_pack: u.language_pack,
    installs: o.installs.map((i) => ({
      ...i,
      selected: u.channels.includes(i.channel_key),
    })),
    patchers: o.patchers.map((p) => {
      const next = u.patchers[p.id];
      if (!next) return p;
      return {
        ...p,
        enabled: next.enabled ?? p.enabled,
        values: { ...next.options },
      };
    }),
  };
}

// Config saves are queued: one IPC in flight, and edits made meanwhile
// collapse to the latest (each update carries the whole config, so only the
// newest matters). The backend debounces the actual re-patch on its side.
let _pending: LangpatchConfigUpdate | null = null;
let _flushing = false;

export async function saveLangpatchConfig(
  update: LangpatchConfigUpdate,
): Promise<void> {
  if (_overview) _overview = withUpdate(_overview, update);
  _pending = update;
  if (_flushing) return;
  _flushing = true;
  try {
    while (_pending) {
      const next = _pending;
      _pending = null;
      const result = await commands.langpatchUpdateConfig(next);
      if (result.status === "error") _error = result.error.message;
    }
  } finally {
    _flushing = false;
  }
  _savedRev++;
  await loadLangpatch();
}

/** Manual apply — also the foreign-file "take over" action. */
export async function applyLangpatch(channel: string): Promise<void> {
  _busy = true;
  try {
    const result = await commands.langpatchApply(channel);
    if (result.status === "error") _error = result.error.message;
    await loadLangpatch();
  } finally {
    _busy = false;
  }
}

export async function removeLangpatch(channel: string): Promise<void> {
  _busy = true;
  try {
    const result = await commands.langpatchRemove(channel);
    if (result.status === "error") _error = result.error.message;
    await loadLangpatch();
  } finally {
    _busy = false;
  }
}
