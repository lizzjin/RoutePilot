CREATE TABLE IF NOT EXISTS routepilot_state (
    namespace TEXT PRIMARY KEY,
    document JSONB NOT NULL,
    revision BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE routepilot_state
    ADD COLUMN IF NOT EXISTS revision BIGINT NOT NULL DEFAULT 0;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conrelid = 'routepilot_state'::regclass
          AND conname = 'routepilot_state_revision_nonnegative'
    ) THEN
        ALTER TABLE routepilot_state
            ADD CONSTRAINT routepilot_state_revision_nonnegative
            CHECK (revision >= 0);
    END IF;
END
$$;
