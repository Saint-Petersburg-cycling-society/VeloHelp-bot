-- SPDX-License-Identifier: AGPL-3.0-only
-- Empty preferences preserve legacy subscriptions to the whole help kind.
CREATE TABLE helper_problem_tags (
    user_id INTEGER NOT NULL REFERENCES helper_profiles(user_id) ON DELETE CASCADE,
    tag TEXT NOT NULL CHECK (tag IN ('puncture','drivetrain','brakes','wheel','tools','transport','accident')),
    PRIMARY KEY (user_id, tag)
) STRICT;

-- Private requester-provided address, shared only with the current match.
ALTER TABLE help_requests ADD COLUMN exact_address TEXT
    CHECK (exact_address IS NULL OR length(exact_address) BETWEEN 1 AND 200);
CREATE TRIGGER clear_private_address
AFTER UPDATE OF status, exact_lat, exact_lon ON help_requests
WHEN NEW.status IN ('resolved','cancelled','expired')
    OR NEW.exact_lat IS NULL OR NEW.exact_lon IS NULL
BEGIN
    UPDATE help_requests SET exact_address=NULL WHERE id=NEW.id;
END;
