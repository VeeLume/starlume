# Your repo owns this file — the template never overwrites it (copier _skip_if_exists).
# Shared recipes live in common.just, kept in sync via `copier update`. `just --list` to see all.
import? 'common.just'

default:
    @just --list

# --- repo-local recipes below (add yours here) ---

# The bin is gated behind the non-default `bindgen` feature (kept out of release builds).
# Regenerate src/lib/bindings.ts from the Rust command surface
bindings:
    cargo run -p starlume --features bindgen --bin export-bindings

# Reads .env at the repo root (dotenvy walks up from the cwd); one-time setup: `just server-env`.
# Run the auth/API server locally
server:
    cargo run -p starlume-server

# Creation steps for the Discord application are inside the template file.
# One-time local server setup: copy .env.example → .env, then fill in credentials
server-env:
    @test -f .env && echo ".env already exists — edit it directly" || (cp .env.example .env && echo "created .env — fill in DISCORD_CLIENT_ID / DISCORD_CLIENT_SECRET")

# Dev data only — signs every locally-issued device token out. Migrations recreate the DB on next start.
# Delete the local server database
server-db-reset:
    rm -f starlume.db starlume.db-shm starlume.db-wal

# Needs `just dev` running (shares its vite server, and provides the build —
# this runs the existing exe directly: cargo can't relink while the first
# instance holds the file lock). See README "Testing with two accounts".
# Run a second app instance under a dev profile
dev-alt profile="alt":
    STARLUME_PROFILE={{profile}} ./target/debug/starlume.exe

# Backgrounds the server and kills it when `just dev` exits (Ctrl+C). Needs
# `just server-env` done once first. See README "Testing with two accounts"
# for the two-instance flow — this only starts server + first instance.
# Run the server and the first app instance together
# Does not work, hangs terminal after exit
# dev-full:
#     just server & pid=$!; trap 'kill $pid 2>/dev/null' EXIT; just dev

# Debug builds only (app_data_root is build-namespaced) — release data is
# never touched. Close any running dev instance first, files may be locked.
# Delete the default dev data dir (%APPDATA%\starlume-dev)
clean-dev-data:
    rm -rf "$APPDATA/starlume-dev"

# Debug builds only. Close the dev-alt instance first, files may be locked.
# Delete a dev profile's data dir (%APPDATA%\starlume-dev-<profile>)
clean-dev-alt-data profile="alt":
    rm -rf "$APPDATA/starlume-dev-{{profile}}"

# Delete both the default and dev-alt data dirs
clean-dev-data-all profile="alt": clean-dev-data (clean-dev-alt-data profile)
