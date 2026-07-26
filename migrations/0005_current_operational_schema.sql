ALTER TABLE modelport_gateway_requests
    ADD COLUMN username TEXT,
    ADD COLUMN api_key_id TEXT,
    ADD COLUMN api_key_name TEXT,
    ADD COLUMN api_key_group TEXT,
    ADD COLUMN team_id TEXT,
    ADD COLUMN team_name TEXT,
    ADD COLUMN client_ip INET,
    ADD COLUMN request_path TEXT,
    ADD COLUMN traffic_class TEXT,
    ADD COLUMN tool_use_requested BOOLEAN,
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

UPDATE modelport_gateway_requests
SET username = principal_id,
    request_path = CASE client_protocol
        WHEN 'anthropic-messages' THEN '/v1/messages'
        ELSE '/v1/chat/completions'
    END,
    traffic_class = 'business',
    tool_use_requested = false,
    latency_ms = GREATEST(
        0,
        FLOOR(
            EXTRACT(
                EPOCH FROM (COALESCE(completed_at, updated_at) - created_at)
            ) * 1000
        )::bigint
    );

WITH attempt_rollup AS (
    SELECT
        organization_id,
        project_id,
        environment_id,
        request_ledger_id,
        count(*)::integer AS attempt_count,
        (array_agg(attempt_id ORDER BY created_at DESC, attempt_id DESC))[1]
            AS last_attempt_id,
        (array_agg(provider_id ORDER BY created_at DESC, attempt_id DESC))[1]
            AS last_provider_id,
        (array_agg(resolved_model ORDER BY created_at DESC, attempt_id DESC))[1]
            AS last_resolved_model,
        (array_agg(provider_protocol ORDER BY created_at DESC, attempt_id DESC))[1]
            AS last_provider_protocol,
        (array_agg(provider_id ORDER BY created_at, attempt_id))[1]
            AS first_provider_id
    FROM modelport_provider_attempts
    GROUP BY
        organization_id,
        project_id,
        environment_id,
        request_ledger_id
)
UPDATE modelport_gateway_requests AS request
SET provider_id = rollup.last_provider_id,
    resolved_model = rollup.last_resolved_model,
    provider_protocol = rollup.last_provider_protocol,
    last_attempt_id = rollup.last_attempt_id,
    retry_count = GREATEST(rollup.attempt_count - 1, 0),
    fallback_from_provider = CASE
        WHEN rollup.attempt_count > 1
         AND rollup.first_provider_id <> rollup.last_provider_id
            THEN rollup.first_provider_id
        ELSE NULL
    END
FROM attempt_rollup AS rollup
WHERE request.organization_id = rollup.organization_id
  AND request.project_id = rollup.project_id
  AND request.environment_id = rollup.environment_id
  AND request.ledger_id = rollup.request_ledger_id;

ALTER TABLE modelport_gateway_requests
    ALTER COLUMN username SET NOT NULL,
    ALTER COLUMN request_path SET NOT NULL,
    ALTER COLUMN traffic_class SET NOT NULL,
    ALTER COLUMN tool_use_requested SET NOT NULL,
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
    ADD COLUMN fallback_from_provider TEXT;

UPDATE modelport_provider_attempts
SET latency_ms = GREATEST(
    0,
    FLOOR(
        EXTRACT(
            EPOCH FROM (COALESCE(completed_at, updated_at) - created_at)
        ) * 1000
    )::bigint
);

WITH ranked_attempts AS (
    SELECT
        organization_id,
        project_id,
        environment_id,
        attempt_id,
        provider_id,
        row_number() OVER (
            PARTITION BY
                organization_id,
                project_id,
                environment_id,
                request_ledger_id
            ORDER BY created_at, attempt_id
        ) - 1 AS retry_count,
        first_value(provider_id) OVER (
            PARTITION BY
                organization_id,
                project_id,
                environment_id,
                request_ledger_id
            ORDER BY created_at, attempt_id
        ) AS first_provider_id
    FROM modelport_provider_attempts
)
UPDATE modelport_provider_attempts AS attempt
SET retry_count = ranked.retry_count::integer,
    fallback_from_provider = CASE
        WHEN ranked.retry_count > 0
         AND ranked.provider_id <> ranked.first_provider_id
            THEN ranked.first_provider_id
        ELSE NULL
    END
FROM ranked_attempts AS ranked
WHERE attempt.organization_id = ranked.organization_id
  AND attempt.project_id = ranked.project_id
  AND attempt.environment_id = ranked.environment_id
  AND attempt.attempt_id = ranked.attempt_id;

ALTER TABLE modelport_provider_attempts
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
