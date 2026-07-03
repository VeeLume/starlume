-- Friend groups — the v2 social primitive. Deliberately CONTENT-FREE: a group
-- is a named container of users plus invite codes, nothing more. Features
-- (shared wishlists, craft offers, fleet views…) reference group ids as
-- visibility scopes later; the group model itself never learns about them.

CREATE TABLE groups (
    id          TEXT PRIMARY KEY,             -- UUIDv7
    name        TEXT NOT NULL,
    owner_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE group_members (
    group_id    TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (group_id, user_id)
);

CREATE INDEX idx_group_members_user ON group_members(user_id);

CREATE TABLE group_invites (
    code        TEXT PRIMARY KEY,
    group_id    TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    created_by  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at  TEXT NOT NULL
);
