CREATE TABLE modelport_routing_decisions (
    decision_id TEXT PRIMARY KEY,
    request_ledger_id TEXT NOT NULL UNIQUE,
    organization_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    environment_id TEXT NOT NULL,
    route_group_id TEXT,
    routing_profile TEXT NOT NULL,
    routing_mode TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    selected_provider_id TEXT NOT NULL,
    selected_model TEXT NOT NULL,
    recommended_provider_id TEXT NOT NULL,
    recommended_model TEXT NOT NULL,
    candidate_count INTEGER NOT NULL,
    selected_score DOUBLE PRECISION NOT NULL,
    recommended_score DOUBLE PRECISION NOT NULL,
    reason_codes TEXT[] NOT NULL DEFAULT '{}',
    session_affinity BOOLEAN NOT NULL DEFAULT false,
    shadow_disagreement BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT modelport_routing_decisions_request_fk
        FOREIGN KEY (
            organization_id,
            project_id,
            environment_id,
            request_ledger_id
        )
        REFERENCES modelport_gateway_requests (
            organization_id,
            project_id,
            environment_id,
            ledger_id
        )
        ON DELETE CASCADE,
    CONSTRAINT modelport_routing_decisions_profile_check
        CHECK (routing_profile IN ('explicit', 'quality', 'balanced', 'economy', 'latency')),
    CONSTRAINT modelport_routing_decisions_mode_check
        CHECK (
            routing_mode IN (
                'static',
                'off_static',
                'shadow',
                'canary_control',
                'active'
            )
        ),
    CONSTRAINT modelport_routing_decisions_bounds_check
        CHECK (
            candidate_count BETWEEN 1 AND 256
            AND selected_score BETWEEN 0 AND 2
            AND recommended_score BETWEEN 0 AND 2
            AND length(decision_id) BETWEEN 1 AND 80
            AND length(policy_version) BETWEEN 1 AND 64
            AND (route_group_id IS NULL OR length(route_group_id) BETWEEN 1 AND 80)
            AND length(selected_provider_id) BETWEEN 1 AND 80
            AND length(selected_model) BETWEEN 1 AND 240
            AND length(recommended_provider_id) BETWEEN 1 AND 80
            AND length(recommended_model) BETWEEN 1 AND 240
            AND cardinality(reason_codes) <= 16
        )
);

CREATE INDEX modelport_routing_decisions_tenant_created_idx
    ON modelport_routing_decisions (
        organization_id,
        project_id,
        environment_id,
        created_at DESC,
        decision_id DESC
    );

CREATE TABLE modelport_routing_feedback (
    feedback_id TEXT PRIMARY KEY,
    decision_id TEXT NOT NULL,
    source TEXT NOT NULL,
    outcome TEXT NOT NULL,
    score SMALLINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (decision_id)
        REFERENCES modelport_routing_decisions (decision_id)
        ON DELETE CASCADE,
    CONSTRAINT modelport_routing_feedback_source_check
        CHECK (source IN ('user', 'application', 'evaluation')),
    CONSTRAINT modelport_routing_feedback_outcome_check
        CHECK (outcome IN ('accepted', 'rejected', 'correct', 'incorrect', 'partial')),
    CONSTRAINT modelport_routing_feedback_score_check
        CHECK (score IS NULL OR score BETWEEN 0 AND 100),
    CONSTRAINT modelport_routing_feedback_bounds_check
        CHECK (
            length(feedback_id) BETWEEN 1 AND 80
            AND length(decision_id) BETWEEN 1 AND 80
        )
);

CREATE INDEX modelport_routing_feedback_decision_created_idx
    ON modelport_routing_feedback (decision_id, created_at DESC);
