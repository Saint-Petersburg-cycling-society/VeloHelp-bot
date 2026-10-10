-- Tie each notification wave to an authoritative request version.
ALTER TABLE dispatches ADD COLUMN request_version INTEGER NOT NULL DEFAULT 0
    CHECK (request_version >= 0);
-- Existing callbacks are invalidated at process restart; preserve the live wave.
UPDATE dispatches SET request_version = (
    SELECT version FROM help_requests WHERE help_requests.id = dispatches.request_id
);
