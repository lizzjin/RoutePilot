CREATE TABLE IF NOT EXISTS modelport_state (
    namespace TEXT PRIMARY KEY,
    document JSONB NOT NULL,
    revision BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE modelport_state
    ADD COLUMN IF NOT EXISTS revision BIGINT NOT NULL DEFAULT 0;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conrelid = 'modelport_state'::regclass
          AND conname = 'modelport_state_revision_nonnegative'
    ) THEN
        ALTER TABLE modelport_state
            ADD CONSTRAINT modelport_state_revision_nonnegative
            CHECK (revision >= 0);
    END IF;
END
$$;
