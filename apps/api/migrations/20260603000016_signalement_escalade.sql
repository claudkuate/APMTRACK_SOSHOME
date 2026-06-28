-- Escalade des signalements vers les autorités de tutelle.
-- Conforme à la vision G-APM : remontée Mairie / NASLA / MINDDEVEL / MINAT
-- avec traçabilité complète (l'historique reste assuré par audit_logs).

ALTER TABLE signalements
    ADD COLUMN IF NOT EXISTS escalation_target TEXT,
    ADD COLUMN IF NOT EXISTS escalated_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS escalated_by UUID REFERENCES users(id),
    ADD COLUMN IF NOT EXISTS escalation_note TEXT;

ALTER TABLE signalements
    DROP CONSTRAINT IF EXISTS signalements_escalation_target_check,
    ADD CONSTRAINT signalements_escalation_target_check
        CHECK (escalation_target IS NULL
               OR escalation_target IN ('MAIRIE', 'NASLA', 'MINDDEVEL', 'MINAT'));

CREATE INDEX IF NOT EXISTS idx_signalements_escalation_target
    ON signalements (escalation_target)
    WHERE escalation_target IS NOT NULL;
