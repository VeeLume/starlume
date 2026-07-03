-- Direct friends — the 1:1 base primitive; groups (0002) are the 1:M
-- extension on top. Same content-free rule: a friendship is a mutual edge,
-- nothing more; features reference friend user-ids as visibility scopes.
--
-- Friendships are stored as TWO rows (one per direction) — every read
-- becomes "WHERE user_id = ?" with no pair-normalization logic.

CREATE TABLE friendships (
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    friend_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, friend_id)
);

CREATE TABLE friend_invites (
    code        TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at  TEXT NOT NULL
);
