// The game's local telemetry stream (telemetry.rs): where the connection
// stands, the current snapshot (balance, vehicle, vitals, last events) and
// whether the variable that makes the game serve it is installed.
//
// The state lives Rust-side; this store mirrors it (frontend rules 1, 6).
// The backend announces changes with `telemetry:changed` (rule 5), already
// throttled to state changes; a focus re-read catches up after a suspension.
// Loopback only: no online gate, its own opt-in (`telemetry_enabled`).

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type TelemetryEnvState, type TelemetryView } from "$lib/bindings";

let _view = $state<TelemetryView | null>(null);
let _env = $state<TelemetryEnvState | null>(null);
let _error = $state("");

export const telemetryStore = {
  /** `null` = not read yet. */
  get view() {
    return _view;
  },
  get status() {
    return _view?.status ?? "off";
  },
  get snapshot() {
    return _view?.snapshot ?? null;
  },
  /** The variable install; `null` = not read yet. */
  get env() {
    return _env;
  },
  get error() {
    return _error;
  },
};

/** Status + snapshot: a memory read Rust-side. */
export async function loadTelemetry(): Promise<void> {
  _view = await commands.telemetryState();
}

function takeEnv(result: Awaited<ReturnType<typeof commands.telemetryEnvStatus>>): void {
  if (result.status === "ok") {
    _env = result.data;
    _error = "";
  } else {
    _error = result.error.message;
  }
}

/** Is the variable set for the Windows user (a registry read)? */
export async function loadTelemetryEnv(): Promise<void> {
  takeEnv(await commands.telemetryEnvStatus());
}

/** Set the variable for the Windows user. */
export async function installTelemetryEnv(): Promise<void> {
  takeEnv(await commands.telemetryEnvInstall());
}

/** Remove the variable again (only what the install set). */
export async function uninstallTelemetryEnv(): Promise<void> {
  takeEnv(await commands.telemetryEnvUninstall());
}

let _watchers = 0;
let _unlisten: Promise<UnlistenFn> | null = null;
const onFocus = () => {
  void loadTelemetry();
  void loadTelemetryEnv();
};

/** Keep the store current while a surface shows it. Returns the stop function. */
export function watchTelemetry(): () => void {
  if (_watchers++ === 0) {
    _unlisten = listen("telemetry:changed", () => void loadTelemetry());
    window.addEventListener("focus", onFocus);
    onFocus();
  }
  let stopped = false;
  return () => {
    if (stopped) return;
    stopped = true;
    if (--_watchers === 0) {
      void _unlisten?.then((u) => u());
      _unlisten = null;
      window.removeEventListener("focus", onFocus);
    }
  };
}
