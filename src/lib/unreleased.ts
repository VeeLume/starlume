// Surfaces held back from public releases until their design work is done
// (v0.1.0 scope decision): friends + the Discord/server connector, the
// reference catalogs browse UI, the contracts module. Dev builds keep showing
// them so work on them continues; release builds hide them — hidden, not
// deleted. The Rust half is `modules::registry()` (`cfg!(debug_assertions)`);
// `tauri dev` and `tauri build` flip both together.
//
// Remove a surface from this gate when it ships, not by flipping the flag.

/** True in dev builds: unreleased surfaces are shown. */
export const SHOW_UNRELEASED: boolean = import.meta.env.DEV;
