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

# Needs `just dev` running (shares its vite server). Own data dir + keyring slot,
# no single-instance lock, loopback auth. See README "Testing with two accounts".
# Run a second app instance under a dev profile
dev-alt profile="alt":
    STARLUME_PROFILE={{profile}} cargo run -p starlume
