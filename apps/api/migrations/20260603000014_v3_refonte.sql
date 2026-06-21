-- V3 APMTRACK refonte fonctionnelle.
-- Suppression volontaire des champs agent sortis du métier.

ALTER TABLE agents
    DROP COLUMN IF EXISTS grade,
    DROP COLUMN IF EXISTS formation_nasla;

ALTER TABLE communes
    ADD COLUMN IF NOT EXISTS subscription_status TEXT NOT NULL DEFAULT 'ACTIVE',
    ADD COLUMN IF NOT EXISTS subscription_started_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS subscription_expires_at TIMESTAMPTZ;

ALTER TABLE communes
    DROP CONSTRAINT IF EXISTS communes_subscription_status_check,
    ADD CONSTRAINT communes_subscription_status_check
        CHECK (subscription_status IN ('ACTIVE', 'TRIAL', 'EXPIRED', 'SUSPENDED'));

CREATE INDEX IF NOT EXISTS idx_communes_subscription_visibility
    ON communes (active, subscription_status, subscription_expires_at)
    WHERE deleted_at IS NULL;

ALTER TABLE signalements
    ADD COLUMN IF NOT EXISTS zone_id UUID REFERENCES zones(id),
    ADD COLUMN IF NOT EXISTS lieu_dit TEXT,
    ADD COLUMN IF NOT EXISTS contact_name TEXT,
    ADD COLUMN IF NOT EXISTS contact_phone TEXT;

CREATE INDEX IF NOT EXISTS idx_signalements_zone_id
    ON signalements (zone_id);

ALTER TABLE interventions
    ADD COLUMN IF NOT EXISTS requires_vehicle BOOLEAN NOT NULL DEFAULT FALSE;
