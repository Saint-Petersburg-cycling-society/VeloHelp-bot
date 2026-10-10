PRAGMA foreign_keys = ON;

CREATE TABLE users (
    telegram_user_id INTEGER PRIMARY KEY,
    consent_version TEXT NOT NULL,
    consented_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at INTEGER
) STRICT;

CREATE TABLE areas (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES areas(id),
    kind TEXT NOT NULL,
    display_name TEXT NOT NULL,
    sort_key INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1))
) STRICT;

CREATE TABLE helper_profiles (
    user_id INTEGER PRIMARY KEY REFERENCES users(telegram_user_id) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    timezone TEXT NOT NULL DEFAULT 'Europe/Moscow',
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE TABLE helper_areas (
    user_id INTEGER NOT NULL REFERENCES helper_profiles(user_id) ON DELETE CASCADE,
    area_id TEXT NOT NULL REFERENCES areas(id),
    PRIMARY KEY (user_id, area_id)
) STRICT;

CREATE TABLE helper_help_kinds (
    user_id INTEGER NOT NULL REFERENCES helper_profiles(user_id) ON DELETE CASCADE,
    help_kind TEXT NOT NULL CHECK (help_kind IN ('technical', 'accident')),
    PRIMARY KEY (user_id, help_kind)
) STRICT;

CREATE TABLE availability_windows (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES helper_profiles(user_id) ON DELETE CASCADE,
    weekday INTEGER NOT NULL CHECK (weekday BETWEEN 1 AND 7),
    local_start_minutes INTEGER NOT NULL CHECK (local_start_minutes BETWEEN 0 AND 1439),
    local_end_minutes INTEGER NOT NULL CHECK (local_end_minutes BETWEEN 0 AND 1439),
    CHECK (local_start_minutes <> local_end_minutes)
) STRICT;

CREATE TABLE help_requests (
    id TEXT PRIMARY KEY,
    requester_id INTEGER NOT NULL REFERENCES users(telegram_user_id),
    help_kind TEXT NOT NULL CHECK (help_kind IN ('technical', 'accident')),
    description TEXT NOT NULL CHECK (length(description) BETWEEN 1 AND 4000),
    bike_description TEXT CHECK (bike_description IS NULL OR length(bike_description) <= 2000),

    exact_lat REAL CHECK (exact_lat IS NULL OR exact_lat BETWEEN -90.0 AND 90.0),
    exact_lon REAL CHECK (exact_lon IS NULL OR exact_lon BETWEEN -180.0 AND 180.0),
    area_id TEXT REFERENCES areas(id),

    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'open', 'matched', 'resolved', 'cancelled', 'expired')),
    matched_helper_id INTEGER REFERENCES users(telegram_user_id),

    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    published_at INTEGER,
    matched_at INTEGER,
    resolved_at INTEGER,
    expires_at INTEGER NOT NULL,

    CHECK (expires_at > created_at),
    CHECK (matched_helper_id IS NULL OR matched_helper_id <> requester_id),
    CHECK ((status = 'matched' AND matched_helper_id IS NOT NULL) OR status <> 'matched')
) STRICT;

CREATE INDEX help_requests_open_area_idx
    ON help_requests(area_id, help_kind, expires_at)
    WHERE status = 'open';

CREATE TABLE request_problem_tags (
    request_id TEXT NOT NULL REFERENCES help_requests(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (request_id, tag)
) STRICT;

CREATE TABLE dispatches (
    request_id TEXT NOT NULL REFERENCES help_requests(id) ON DELETE CASCADE,
    helper_id INTEGER NOT NULL REFERENCES helper_profiles(user_id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'declined', 'superseded', 'expired')),

    -- Projection identity is adapter/storage state, not domain truth.
    view_id INTEGER,
    chat_id INTEGER,
    message_id INTEGER,
    ephemeral_message_id INTEGER,

    sent_at INTEGER,
    acted_at INTEGER,
    PRIMARY KEY (request_id, helper_id)
) STRICT;

CREATE INDEX dispatches_helper_pending_idx
    ON dispatches(helper_id)
    WHERE status = 'pending';
