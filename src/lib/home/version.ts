// Comparing an install's version with the newest LIVE patch notes. Versions
// only: the notes' changelist is the release build, and hotfixes ship newer
// builds without new notes — so a changelist can say "newer than announced",
// never "outdated".

/** "4.10.2-live.12913470" → "4.10.2". */
export function installVersion(version: string): string {
  return version.split("-")[0];
}

/** Numeric compare of dotted versions; missing parts count as 0 ("4.10" = "4.10.0"). */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map((p) => parseInt(p, 10) || 0);
  const pb = b.split(".").map((p) => parseInt(p, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return Math.sign(d);
  }
  return 0;
}

export type UpToDate = "current" | "behind" | "ahead";

/** Where an install stands against the newest announced LIVE version. */
export function upToDate(install: string, announced: string): UpToDate {
  const c = compareVersions(installVersion(install), announced);
  return c < 0 ? "behind" : c > 0 ? "ahead" : "current";
}

/** "4.10.2-live.12913470" → "12913470"; `null` without one. */
export function installChangelist(version: string): string | null {
  const m = /\.(\d{6,})$/.exec(version);
  return m ? m[1] : null;
}

export type BuildStatus = UpToDate | "hotfix";

/** {@link upToDate}, plus `hotfix`: the announced version with a newer
 *  build than the notes name — a hotfix shipped after the notes (they get
 *  no notes of their own). Needs the notes' changelist. */
export function buildStatus(install: string, announced: string, changelist: string | null): BuildStatus {
  const status = upToDate(install, announced);
  if (status !== "current" || !changelist) return status;
  const build = parseInt(installChangelist(install) ?? "", 10);
  return Number.isFinite(build) && build > parseInt(changelist, 10) ? "hotfix" : "current";
}
