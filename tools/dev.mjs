// `just dev-here`: the Tauri dev app on this checkout's own dev port, so two checkouts run side by side.
//
// Tauri's `devUrl` is fixed in tauri.conf.json and cannot read the environment, so the port it should open is
// handed over through TAURI_CONFIG, which the CLI merges into the config. The STARLUME_* keys of an untracked
// `.env.local` in the repo root (STARLUME_DEV_PORT, and STARLUME_PROFILE / STARLUME_DATA_DIR for the app itself,
// see CONTRIBUTING "Testing with two accounts") reach vite and the app through the environment; a variable
// already set in the shell wins. With none of them set this is `pnpm tauri dev`.
//
// `--dry-run` prints what it would do and starts nothing.
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const env = { ...process.env };

try {
  for (const line of readFileSync(join(root, ".env.local"), "utf8").split(/\r?\n/)) {
    const m = /^\s*(STARLUME_[A-Z0-9_]+)\s*=\s*(.*?)\s*$/.exec(line);
    if (m && !(m[1] in process.env)) env[m[1]] = m[2].replace(/^(["'])(.*)\1$/, "$2");
  }
} catch (e) {
  if (e.code !== "ENOENT") throw e;
}

const port = Number(env.STARLUME_DEV_PORT) || 1445;
if (port !== 1445 && !env.TAURI_CONFIG) {
  env.TAURI_CONFIG = JSON.stringify({ build: { devUrl: `http://localhost:${port}` } });
}

const args = process.argv.slice(2);
const dry = args.includes("--dry-run");
const passed = args.filter((a) => a !== "--dry-run");

console.log(`starlume dev: vite on :${port}${env.STARLUME_PROFILE ? `, profile ${env.STARLUME_PROFILE}` : ""}`);
if (dry) {
  console.log(`  TAURI_CONFIG=${env.TAURI_CONFIG ?? "(unset)"}`);
  process.exit(0);
}

const child = spawn("pnpm", ["exec", "tauri", "dev", ...passed], { cwd: root, env, stdio: "inherit", shell: true });
child.on("exit", (code) => process.exit(code ?? 1));
