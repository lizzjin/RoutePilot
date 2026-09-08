ALTER TABLE routepilot_provider_attempts
    ADD CONSTRAINT routepilot_provider_attempts_tenant_attempt_unique
        UNIQUE (organization_id, project_id, environment_id, attempt_id);

CREATE TABLE routepilot_budget_accounts (
    organization_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    environment_id TEXT NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    limit_microunits BIGINT,
    reserved_microunits BIGINT NOT NULL DEFAULT 0,
    settled_microunits BIGINT NOT NULL DEFAULT 0,
    version BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, project_id, environment_id, currency),
    CONSTRAINT routepilot_budget_accounts_amounts_check CHECK (
        (limit_microunits IS NULL OR limit_microunits >= 0)
        AND reserved_microunits >= 0
        AND settled_microunits >= 0
        AND version >= 0
    ),
    FOREIGN KEY (organization_id, project_id, environment_id)
        REFERENCES routepilot_environments (organization_id, project_id, environment_id)
        ON DELETE RESTRICT
);

INSERT INTO routepilot_budget_accounts (
    organization_id,
    project_id,
    environment_id,
    currency,
    limit_microunits
)
VALUES ('org_local', 'prj_default', 'env_default', 'USD', NULL)
ON CONFLICT (organization_id, project_id, environment_id, currency) DO NOTHING;

CREATE TABLE routepilot_budget_reservations (
    reservation_id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    environment_id TEXT NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    request_ledger_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    reserved_microunits BIGINT NOT NULL,
    settled_microunits BIGINT NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'reserved',
    evidence_source TEXT,
    billing_mode TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    terminal_at TIMESTAMPTZ,
    CONSTRAINT routepilot_budget_reservations_attempt_unique UNIQUE (
        organization_id,
        project_id,
        environment_id,
        attempt_id
    ),
    CONSTRAINT routepilot_budget_reservations_tenant_id_unique UNIQUE (
        organization_id,
        project_id,
        environment_id,
        reservation_id
    ),
    CONSTRAINT routepilot_budget_reservations_state_check
        CHECK (state IN ('reserved', 'settled', 'released')),
    CONSTRAINT routepilot_budget_reservations_amounts_check
        CHECK (reserved_microunits >= 0 AND settled_microunits >= 0),
    FOREIGN KEY (organization_id, project_id, environment_id, attempt_id)
        REFERENCES routepilot_provider_attempts (
            organization_id,
            project_id,
            environment_id,
            attempt_id
        )
        ON DELETE RESTRICT,
    FOREIGN KEY (organization_id, project_id, environment_id, request_ledger_id)
        REFERENCES routepilot_gateway_requests (
            organization_id,
            project_id,
            environment_id,
            ledger_id
        )
        ON DELETE RESTRICT,
    FOREIGN KEY (organization_id, project_id, environment_id, currency)
        REFERENCES routepilot_budget_accounts (
            organization_id,
            project_id,
            environment_id,
            currency
        )
        ON DELETE RESTRICT
);

CREATE INDEX routepilot_budget_reservations_open_idx
    ON routepilot_budget_reservations (
        organization_id,
        project_id,
        environment_id,
        created_at
    )
    WHERE state = 'reserved';

CREATE TABLE routepilot_budget_events (
    event_id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    environment_id TEXT NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    reservation_id TEXT,
    request_ledger_id TEXT,
    attempt_id TEXT,
    event_type TEXT NOT NULL,
    reserved_delta_microunits BIGINT NOT NULL DEFAULT 0,
    settled_delta_microunits BIGINT NOT NULL DEFAULT 0,
    evidence_source TEXT NOT NULL,
    billing_mode TEXT,
    reason TEXT,
    actor_id TEXT,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    cache_write_tokens BIGINT NOT NULL DEFAULT 0,
    cache_read_tokens BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT routepilot_budget_events_type_check CHECK (
        event_type IN ('reservation_created', 'settled', 'released', 'adjustment')
    ),
    CONSTRAINT routepilot_budget_events_usage_check CHECK (
        input_tokens >= 0
        AND output_tokens >= 0
        AND cache_write_tokens >= 0
        AND cache_read_tokens >= 0
    ),
    FOREIGN KEY (organization_id, project_id, environment_id, currency)
        REFERENCES routepilot_budget_accounts (
            organization_id,
            project_id,
            environment_id,
            currency
        )
        ON DELETE RESTRICT,
    FOREIGN KEY (organization_id, project_id, environment_id, reservation_id)
        REFERENCES routepilot_budget_reservations (
            organization_id,
            project_id,
            environment_id,
            reservation_id
        )
        ON DELETE RESTRICT
);

CREATE INDEX routepilot_budget_events_tenant_created_idx
    ON routepilot_budget_events (
        organization_id,
        project_id,
        environment_id,
        created_at DESC
    );

CREATE INDEX routepilot_budget_events_attempt_idx
    ON routepilot_budget_events (attempt_id, created_at)
    WHERE attempt_id IS NOT NULL;

CREATE FUNCTION routepilot_reject_budget_event_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'routepilot_budget_events is append-only';
END;
$$;

CREATE TRIGGER routepilot_budget_events_append_only
    BEFORE UPDATE OR DELETE ON routepilot_budget_events
    FOR EACH ROW
    EXECUTE FUNCTION routepilot_reject_budget_event_mutation();
