-- Authoritative incident ledger for the optional, read-only ModelPort
-- operations agent. The agent never connects to this database directly;
-- observations arrive through the versioned internal API.
CREATE TABLE modelport_ops_incidents (
    incident_id TEXT PRIMARY KEY,
    event_key TEXT NOT NULL UNIQUE,
    detector_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    affected_scope JSONB NOT NULL DEFAULT '{}'::jsonb,
    recovery_criteria TEXT NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    resolved_at TIMESTAMPTZ,
    occurrence_count BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT modelport_ops_incidents_severity_check
        CHECK (severity IN ('SEV-1', 'SEV-2', 'SEV-3', 'SEV-4')),
    CONSTRAINT modelport_ops_incidents_status_check
        CHECK (status IN (
            'open', 'acknowledged', 'mitigating', 'monitoring', 'resolved', 'suppressed'
        )),
    CONSTRAINT modelport_ops_incidents_text_check CHECK (
        length(event_key) BETWEEN 1 AND 240
        AND length(detector_type) BETWEEN 1 AND 80
        AND length(title) BETWEEN 1 AND 240
        AND length(summary) BETWEEN 1 AND 2000
        AND length(recovery_criteria) BETWEEN 1 AND 1000
        AND occurrence_count > 0
    )
);

CREATE INDEX modelport_ops_incidents_status_seen_idx
    ON modelport_ops_incidents (status, last_seen_at DESC, incident_id DESC);

CREATE TABLE modelport_ops_incident_evidence (
    evidence_id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES modelport_ops_incidents (incident_id)
        ON DELETE CASCADE,
    evidence_hash TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    evidence JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT modelport_ops_incident_evidence_unique
        UNIQUE (incident_id, evidence_hash),
    CONSTRAINT modelport_ops_incident_evidence_hash_check
        CHECK (length(evidence_hash) = 64)
);

CREATE INDEX modelport_ops_incident_evidence_incident_idx
    ON modelport_ops_incident_evidence (incident_id, observed_at DESC, evidence_id DESC);

CREATE TABLE modelport_ops_incident_timeline (
    timeline_id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES modelport_ops_incidents (incident_id)
        ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    actor_name TEXT NOT NULL,
    message TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT modelport_ops_incident_timeline_text_check CHECK (
        length(event_type) BETWEEN 1 AND 80
        AND length(actor_id) BETWEEN 1 AND 160
        AND length(actor_name) BETWEEN 1 AND 160
        AND length(message) BETWEEN 1 AND 2000
    )
);

CREATE INDEX modelport_ops_incident_timeline_incident_idx
    ON modelport_ops_incident_timeline (incident_id, occurred_at, timeline_id);

CREATE TABLE modelport_ops_agent_heartbeats (
    instance_id TEXT PRIMARY KEY,
    agent_version TEXT NOT NULL,
    mode TEXT NOT NULL,
    rule_set_version TEXT NOT NULL,
    queue_depth BIGINT NOT NULL DEFAULT 0,
    interval_seconds BIGINT NOT NULL,
    analysis_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    selected_model TEXT,
    model_status TEXT NOT NULL DEFAULT 'disabled',
    model_last_success_at TIMESTAMPTZ,
    observed_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT modelport_ops_agent_heartbeats_mode_check
        CHECK (mode IN ('disabled', 'replay', 'shadow', 'read_only')),
    CONSTRAINT modelport_ops_agent_heartbeats_text_check CHECK (
        length(instance_id) BETWEEN 1 AND 160
        AND length(agent_version) BETWEEN 1 AND 80
        AND length(rule_set_version) BETWEEN 1 AND 80
        AND (selected_model IS NULL OR length(selected_model) BETWEEN 1 AND 320)
        AND model_status IN ('disabled', 'configured', 'missing_credential', 'error')
        AND queue_depth >= 0
        AND interval_seconds BETWEEN 10 AND 3600
    )
);

CREATE TABLE modelport_ops_incident_feedback (
    feedback_id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES modelport_ops_incidents (incident_id)
        ON DELETE CASCADE,
    actor_id TEXT NOT NULL,
    actor_name TEXT NOT NULL,
    outcome TEXT NOT NULL,
    root_cause_correct BOOLEAN,
    recommendation_adopted BOOLEAN,
    note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT modelport_ops_incident_feedback_outcome_check
        CHECK (outcome IN ('true_positive', 'false_positive', 'needs_review')),
    CONSTRAINT modelport_ops_incident_feedback_text_check CHECK (
        length(actor_id) BETWEEN 1 AND 160
        AND length(actor_name) BETWEEN 1 AND 160
        AND (note IS NULL OR length(note) <= 1000)
    )
);

CREATE INDEX modelport_ops_incident_feedback_incident_idx
    ON modelport_ops_incident_feedback (incident_id, created_at DESC);
