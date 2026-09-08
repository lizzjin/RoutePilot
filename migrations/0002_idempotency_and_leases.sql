ALTER TABLE routepilot_gateway_requests
    ADD COLUMN idempotency_key_hash TEXT,
    ADD COLUMN request_fingerprint TEXT NOT NULL DEFAULT repeat('0', 64),
    ADD COLUMN lease_owner TEXT NOT NULL DEFAULT 'migration',
    ADD COLUMN lease_expires_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE routepilot_gateway_requests
    ADD CONSTRAINT routepilot_gateway_requests_idempotency_hash_check
        CHECK (idempotency_key_hash IS NULL OR length(idempotency_key_hash) = 64),
    ADD CONSTRAINT routepilot_gateway_requests_fingerprint_check
        CHECK (length(request_fingerprint) = 64),
    ADD CONSTRAINT routepilot_gateway_requests_lease_owner_check
        CHECK (length(lease_owner) BETWEEN 1 AND 128);

CREATE UNIQUE INDEX routepilot_gateway_requests_idempotency_unique_idx
    ON routepilot_gateway_requests (
        organization_id,
        project_id,
        environment_id,
        idempotency_key_hash
    )
    WHERE idempotency_key_hash IS NOT NULL;

CREATE INDEX routepilot_gateway_requests_expired_lease_idx
    ON routepilot_gateway_requests (lease_expires_at)
    WHERE state = 'started';

ALTER TABLE routepilot_provider_attempts
    ADD COLUMN lease_owner TEXT NOT NULL DEFAULT 'migration',
    ADD COLUMN lease_expires_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE routepilot_provider_attempts
    ADD CONSTRAINT routepilot_provider_attempts_lease_owner_check
        CHECK (length(lease_owner) BETWEEN 1 AND 128);

CREATE INDEX routepilot_provider_attempts_expired_lease_idx
    ON routepilot_provider_attempts (lease_expires_at)
    WHERE state = 'started';
