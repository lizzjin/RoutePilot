DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM modelport_gateway_requests LIMIT 1)
        OR EXISTS (SELECT 1 FROM modelport_provider_attempts LIMIT 1)
    THEN
        RAISE EXCEPTION
            'ModelPort current operational schema requires a new database; legacy request and attempt rows are not migrated';
    END IF;
END
$$;

ALTER TABLE modelport_gateway_requests
    ADD COLUMN username TEXT NOT NULL,
    ADD COLUMN api_key_id TEXT,
    ADD COLUMN api_key_name TEXT,
    ADD COLUMN api_key_group TEXT,
    ADD COLUMN team_id TEXT,
    ADD COLUMN team_name TEXT,
    ADD COLUMN client_ip INET,
    ADD COLUMN request_path TEXT NOT NULL,
    ADD COLUMN traffic_class TEXT NOT NULL,
    ADD COLUMN tool_use_requested BOOLEAN NOT NULL,
    ADD COLUMN provider_id TEXT,
    ADD COLUMN resolved_model TEXT,
    ADD COLUMN provider_protocol TEXT,
    ADD COLUMN last_attempt_id TEXT,
    ADD COLUMN model_pricing JSONB,
    ADD COLUMN latency_ms BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN first_byte_latency_ms BIGINT,
    ADD COLUMN tool_outcome TEXT NOT NULL DEFAULT 'not_requested',
    ADD COLUMN tool_repair_attempted BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN tool_repair_recovered BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN fallback_from_provider TEXT;

ALTER TABLE modelport_gateway_requests
    ADD CONSTRAINT modelport_gateway_requests_path_check
        CHECK (request_path IN ('/v1/messages', '/v1/chat/completions')),
    ADD CONSTRAINT modelport_gateway_requests_traffic_class_check
        CHECK (traffic_class IN ('business', 'synthetic', 'diagnostic')),
    ADD CONSTRAINT modelport_gateway_requests_latency_check
        CHECK (
            latency_ms >= 0
            AND (first_byte_latency_ms IS NULL OR first_byte_latency_ms >= 0)
            AND retry_count >= 0
        ),
    ADD CONSTRAINT modelport_gateway_requests_provider_snapshot_check
        CHECK (
            (provider_id IS NULL
                AND resolved_model IS NULL
                AND provider_protocol IS NULL
                AND last_attempt_id IS NULL)
            OR
            (provider_id IS NOT NULL
                AND resolved_model IS NOT NULL
                AND provider_protocol IS NOT NULL
                AND last_attempt_id IS NOT NULL)
        ),
    ADD CONSTRAINT modelport_gateway_requests_tool_outcome_check
        CHECK (
            tool_outcome IN (
                'not_requested',
                'continuation_tool_called',
                'tool_called',
                'final_answer',
                'answered_without_tool',
                'completed_unobserved',
                'client_cancelled',
                'timeout',
                'protocol_error',
                'upstream_or_delivery_error'
            )
        );

ALTER TABLE modelport_provider_attempts
    ADD COLUMN latency_ms BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN first_byte_latency_ms BIGINT,
    ADD COLUMN tool_outcome TEXT NOT NULL DEFAULT 'not_requested',
    ADD COLUMN tool_repair_attempted BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN tool_repair_recovered BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN fallback_from_provider TEXT,
    ADD CONSTRAINT modelport_provider_attempts_latency_check
        CHECK (
            latency_ms >= 0
            AND (first_byte_latency_ms IS NULL OR first_byte_latency_ms >= 0)
            AND retry_count >= 0
        ),
    ADD CONSTRAINT modelport_provider_attempts_tool_outcome_check
        CHECK (
            tool_outcome IN (
                'not_requested',
                'continuation_tool_called',
                'tool_called',
                'final_answer',
                'answered_without_tool',
                'completed_unobserved',
                'client_cancelled',
                'timeout',
                'protocol_error',
                'upstream_or_delivery_error'
            )
        );

CREATE INDEX modelport_gateway_requests_operational_created_idx
    ON modelport_gateway_requests (
        traffic_class,
        created_at DESC,
        ledger_id DESC
    );

CREATE INDEX modelport_gateway_requests_api_key_created_idx
    ON modelport_gateway_requests (api_key_id, created_at DESC)
    WHERE api_key_id IS NOT NULL;

CREATE INDEX modelport_gateway_requests_team_created_idx
    ON modelport_gateway_requests (team_id, created_at DESC)
    WHERE team_id IS NOT NULL;

CREATE INDEX modelport_gateway_requests_principal_created_idx
    ON modelport_gateway_requests (principal_id, created_at DESC);

CREATE INDEX modelport_gateway_requests_provider_created_idx
    ON modelport_gateway_requests (provider_id, created_at DESC)
    WHERE provider_id IS NOT NULL;

CREATE TABLE modelport_audit_events (
    event_id TEXT PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    activity_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    actor_name TEXT NOT NULL,
    target TEXT NOT NULL,
    message TEXT NOT NULL,
    severity TEXT NOT NULL,
    CONSTRAINT modelport_audit_events_severity_check
        CHECK (severity IN ('info', 'warning', 'error')),
    CONSTRAINT modelport_audit_events_text_check
        CHECK (
            length(activity_type) BETWEEN 1 AND 80
            AND length(actor_id) BETWEEN 1 AND 160
            AND length(actor_name) BETWEEN 1 AND 160
            AND length(target) BETWEEN 1 AND 500
            AND length(message) BETWEEN 1 AND 1000
        )
);

CREATE INDEX modelport_audit_events_occurred_idx
    ON modelport_audit_events (occurred_at DESC, event_id DESC);
