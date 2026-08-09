-- Atomically reserve key-, team-, and user-scoped hard-limit capacity before
-- an upstream attempt starts. One row represents one logical gateway request;
-- retries increase its token/cost reservation without incrementing requests.
-- Retention only de-identifies terminal rows: after the configured user-usage
-- window, user_id/team_id are removed or replaced while the already
-- pseudonymous quota_subject_id and financial evidence remain available.
CREATE TABLE modelport_usage_reservations (
    reservation_id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    environment_id TEXT NOT NULL,
    request_ledger_id TEXT NOT NULL,
    quota_subject_id TEXT,
    team_id TEXT,
    user_id TEXT NOT NULL,
    reserved_requests BIGINT NOT NULL DEFAULT 1,
    reserved_tokens BIGINT NOT NULL DEFAULT 0,
    reserved_cost_microunits BIGINT NOT NULL DEFAULT 0,
    actual_requests BIGINT NOT NULL DEFAULT 0,
    actual_tokens BIGINT NOT NULL DEFAULT 0,
    actual_cost_microunits BIGINT NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'reserved',
    evidence_source TEXT,
    billing_mode TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    terminal_at TIMESTAMPTZ,
    CONSTRAINT modelport_usage_reservations_request_unique UNIQUE (
        organization_id,
        project_id,
        environment_id,
        request_ledger_id
    ),
    CONSTRAINT modelport_usage_reservations_state_check
        CHECK (state IN ('reserved', 'settled', 'released')),
    CONSTRAINT modelport_usage_reservations_amounts_check CHECK (
        reserved_requests BETWEEN 0 AND 1
        AND reserved_tokens >= 0
        AND reserved_cost_microunits >= 0
        AND actual_requests BETWEEN 0 AND 1
        AND actual_tokens >= 0
        AND actual_cost_microunits >= 0
    ),
    FOREIGN KEY (organization_id, project_id, environment_id, request_ledger_id)
        REFERENCES modelport_gateway_requests (
            organization_id,
            project_id,
            environment_id,
            ledger_id
        )
        ON DELETE RESTRICT
);

CREATE INDEX modelport_usage_reservations_subject_open_idx
    ON modelport_usage_reservations (quota_subject_id, created_at)
    WHERE state = 'reserved' AND quota_subject_id IS NOT NULL;

CREATE INDEX modelport_usage_reservations_team_open_idx
    ON modelport_usage_reservations (team_id, created_at)
    WHERE state = 'reserved' AND team_id IS NOT NULL;

CREATE INDEX modelport_usage_reservations_user_open_idx
    ON modelport_usage_reservations (user_id, created_at)
    WHERE state = 'reserved';
