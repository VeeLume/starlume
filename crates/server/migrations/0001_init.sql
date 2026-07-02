-- Users are Discord identities; device tokens are what desktop companions
-- authenticate with (one row per install → per-device revocation for free).

CREATE TABLE users (
    id          TEXT PRIMARY KEY,             -- UUIDv7
    discord_id  TEXT NOT NULL UNIQUE,
    username    TEXT NOT NULL,                -- Discord global_name (fallback: username)
    avatar      TEXT,                         -- Discord avatar hash, nullable
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE device_tokens (
    id          TEXT PRIMARY KEY,             -- UUIDv7
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,         -- sha256 hex; the raw token is never stored
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_device_tokens_user ON device_tokens(user_id);
