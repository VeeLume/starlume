// `just dev-here`: the Tauri dev app on this checkout's own dev port, so two checkouts run side by side.
//
// Tauri's `devUrl` is fixed in tauri.conf.json and cannot read the environment, so the port it should open is
// handed over as a `--config` file the CLI merges into the config (the CLI ignores a TAURI_CONFIG variable: it
// waited on 1445 with vite on the checkout's port). The STARLUME_* keys of an untracked
// `.env.local` in the repo root (STARLUME_DEV_PORT, and STARLUME_PROFILE / STARLUME_DATA_DIR for the app itself,
// see CONTRIBUTING "Testing with two accounts") reach vite and the app through the environment; a variable
// already set in the shell wins. With none of them set this is `pnpm tauri dev`.
//
// `--dry-run` prints what it would do and starts nothing.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
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
const devUrl = port !== 1445 ? `http://localhost:${port}` : null;

const args = process.argv.slice(2);
const dry = args.includes("--dry-run");
const passed = args.filter((a) => a !== "--dry-run");

console.log(`starlume dev: vite on :${port}${env.STARLUME_PROFILE ? `, profile ${env.STARLUME_PROFILE}` : ""}`);
if (dry) {
  console.log(`  devUrl=${devUrl ?? "(tauri.conf.json)"}`);
  process.exit(0);
}

if (devUrl) {
  const file = join(mkdtempSync(join(tmpdir(), "starlume-dev-")), "devurl.json");
  writeFileSync(file, JSON.stringify({ build: { devUrl } }));
  passed.unshift("--config", `"${file}"`);
}

const child = spawn("pnpm", ["exec", "tauri", "dev", ...passed], { cwd: root, env, stdio: "inherit", shell: true });
child.on("exit", (code) => process.exit(code ?? 1));
