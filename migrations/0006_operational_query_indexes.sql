CREATE INDEX routepilot_gateway_requests_created_idx
    ON routepilot_gateway_requests (created_at DESC, ledger_id DESC)
    WHERE state <> 'started';

CREATE INDEX routepilot_gateway_requests_status_created_idx
    ON routepilot_gateway_requests (state, created_at DESC, ledger_id DESC)
    WHERE state <> 'started';
