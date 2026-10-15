// The unofficial-API opt-in: any RSI/CIG API Starlume is not an official
// client of — CIG's game-services backend (gRPC) and RSI's Spectrum. One
// master switch (persisted as `grpc_enabled`, the name predates Spectrum) plus
// a per-feature allow-list (`grpc_features`); the backend gate is
// `AppState::require_unofficial`. Shared by Settings → Online and onboarding.

/** The one-time consent shown before the master switch first turns on. */
export const UNOFFICIAL_CONSENT = {
  title: "Unofficial APIs consent",
  message:
    "These features read your data from RSI and CIG services Starlume is not an " +
    "official client of — the game-services backend and Spectrum — using your " +
    "launcher session. That sits in a ToS-grey area. Calls are read-only, paced " +
    "to stay light, and made only for the features you enable. " +
    "Your account, your risk.\n\nAllow unofficial API calls?",
} as const;
