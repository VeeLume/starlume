// Text Patching (langpatch) module state — the /langpatch page renders this
// cache synchronously (docs/frontend.md rule 1); `langpatch:changed` from
// the backend (reconcile finished, patch applied/removed) triggers a
// refresh while the page listens.

import {
  commands,
  type LangpatchOverview,
  type LangpatchConfigUpdate,
} from "$lib/bindings";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

let _overview = $state<LangpatchOverview | null>(null);
let _busy = $state(false);
let _error = $state<string | null>(null);

export const langpatchStore = {
  get overview() {
    return _overview;
  },
  /** A config save / manual apply / remove is in flight. */
  get busy() {
    return _busy;
  },
  get error() {
    return _error;
  },
};

export async function loadLangpatch(): Promise<void> {
  const result = await commands.langpatchOverview();
  if (result.status === "ok") {
    _overview = result.data;
    _error = null;
  } else {
    _error = result.error.message;
  }
}

/** Backend push: reconcile ran, a patch was applied/removed. */
export function listenForLangpatchChanges(): Promise<UnlistenFn> {
  return listen("langpatch:changed", () => void loadLangpatch());
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

export async function saveLangpatchConfig(
  update: LangpatchConfigUpdate,
): Promise<void> {
  _busy = true;
  try {
    const result = await commands.langpatchUpdateConfig(update);
    if (result.status === "error") _error = result.error.message;
    await loadLangpatch();
  } finally {
    _busy = false;
  }
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
