CREATE INDEX help_requests_requester_active_idx
    ON help_requests(requester_id, status)
    WHERE status IN ('draft', 'open', 'matched');

CREATE INDEX help_requests_expiry_idx
    ON help_requests(expires_at)
    WHERE status = 'open';

CREATE INDEX dispatches_request_status_idx
    ON dispatches(request_id, status);
