-- Active-match arrival window. Clear on resolve/cancel/expire/withdraw/deletion.
ALTER TABLE help_requests ADD COLUMN arrival_from_at INTEGER
    CHECK (arrival_from_at IS NULL OR arrival_from_at >= 0);
ALTER TABLE help_requests ADD COLUMN arrival_until_at INTEGER
    CHECK ((arrival_from_at IS NULL AND arrival_until_at IS NULL)
        OR (arrival_from_at IS NOT NULL AND arrival_until_at IS NOT NULL
            AND arrival_from_at <= arrival_until_at));
