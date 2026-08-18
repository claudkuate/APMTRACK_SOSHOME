-- Activation des communes par abonnement paye.
--
-- `communes.active` reste l'interrupteur administratif. L'acces effectif est
-- calcule par `commune_subscription_is_active`, source unique utilisee par
-- l'authentification et les routes publiques/mobile.

ALTER TABLE communes
    ADD COLUMN IF NOT EXISTS subscription_legacy_access_until TIMESTAMPTZ;

CREATE TABLE IF NOT EXISTS commune_subscription_payments (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    commune_id          UUID        NOT NULL REFERENCES communes(id),
    payment_reference   TEXT        NOT NULL,
    amount_fcfa         BIGINT      NOT NULL CHECK (amount_fcfa > 0),
    paid_at             TIMESTAMPTZ NOT NULL,
    period_started_at   TIMESTAMPTZ NOT NULL,
    period_expires_at   TIMESTAMPTZ NOT NULL,
    confirmed_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    confirmed_by_user_id UUID       NOT NULL REFERENCES users(id),
    CONSTRAINT commune_subscription_payment_period_check
        CHECK (period_expires_at > period_started_at)
);

CREATE UNIQUE INDEX IF NOT EXISTS commune_subscription_payment_reference_unique_ci
    ON commune_subscription_payments (commune_id, lower(payment_reference));

CREATE INDEX IF NOT EXISTS idx_commune_subscription_payments_commune_period
    ON commune_subscription_payments (commune_id, period_started_at, period_expires_at);

CREATE OR REPLACE FUNCTION validate_commune_subscription_payment_insert()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.paid_at > clock_timestamp() THEN
        RAISE EXCEPTION 'subscription payment date cannot be in the future'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS commune_subscription_payments_validate_insert
    ON commune_subscription_payments;
CREATE TRIGGER commune_subscription_payments_validate_insert
BEFORE INSERT ON commune_subscription_payments
FOR EACH ROW EXECUTE FUNCTION validate_commune_subscription_payment_insert();

CREATE OR REPLACE FUNCTION reject_commune_subscription_payment_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'commune subscription payments are append-only'
        USING ERRCODE = '55000';
END;
$$;

DROP TRIGGER IF EXISTS commune_subscription_payments_append_only
    ON commune_subscription_payments;
CREATE TRIGGER commune_subscription_payments_append_only
BEFORE UPDATE OR DELETE ON commune_subscription_payments
FOR EACH ROW EXECUTE FUNCTION reject_commune_subscription_payment_mutation();

-- Compatibilite : les abonnements actifs existants restent valides jusqu'a leur
-- echeance connue. Une date de debut absente est reconstruite depuis la creation.
UPDATE communes
SET subscription_started_at = COALESCE(subscription_started_at, created_at, now()),
    subscription_legacy_access_until = CASE
        WHEN subscription_status = 'ACTIVE'
            THEN COALESCE(subscription_expires_at, now() + INTERVAL '60 days')
        ELSE subscription_legacy_access_until
    END,
    subscription_expires_at = COALESCE(subscription_expires_at, now() + INTERVAL '60 days'),
    updated_at = now()
WHERE deleted_at IS NULL
  AND active = TRUE
  AND subscription_status IN ('ACTIVE', 'TRIAL');

ALTER TABLE communes
    ALTER COLUMN active SET DEFAULT FALSE,
    ALTER COLUMN subscription_status SET DEFAULT 'SUSPENDED';

-- Droit temporel/comptable confirme et non expire, independamment de
-- l'interrupteur administratif `active`. Il inclut une periode future deja
-- accordee : elle bloque un second essai et ancre le renouvellement a son terme.
CREATE OR REPLACE FUNCTION commune_subscription_entitlement_is_current(
    target_commune_id UUID,
    checked_at TIMESTAMPTZ DEFAULT now()
) RETURNS BOOLEAN
LANGUAGE sql
STABLE
AS $$
    SELECT COALESCE((
        SELECT c.deleted_at IS NULL
           AND c.subscription_started_at IS NOT NULL
           AND c.subscription_expires_at IS NOT NULL
           AND c.subscription_expires_at >= checked_at
           AND (
                c.subscription_status = 'TRIAL'
                OR c.subscription_legacy_access_until >= checked_at
                OR EXISTS (
                    SELECT 1
                    FROM commune_subscription_payments sp
                    WHERE sp.commune_id = c.id
                      AND sp.period_expires_at >= checked_at
                )
           )
        FROM communes c
        WHERE c.id = target_commune_id
    ), FALSE)
$$;

CREATE OR REPLACE FUNCTION commune_subscription_is_active(
    target_commune_id UUID,
    checked_at TIMESTAMPTZ DEFAULT now()
) RETURNS BOOLEAN
LANGUAGE sql
STABLE
AS $$
    SELECT COALESCE((
        SELECT c.deleted_at IS NULL
           AND c.active = TRUE
           AND c.subscription_status IN ('ACTIVE', 'TRIAL')
           AND c.subscription_started_at IS NOT NULL
           AND c.subscription_started_at <= checked_at
           AND c.subscription_expires_at IS NOT NULL
           AND c.subscription_expires_at >= checked_at
           AND commune_subscription_entitlement_is_current(c.id, checked_at)
        FROM communes c
        WHERE c.id = target_commune_id
    ), FALSE)
$$;

COMMENT ON TABLE commune_subscription_payments IS
    'Registre append-only des paiements d abonnement communal confirmes par un SUPER_ADMIN.';
COMMENT ON COLUMN communes.subscription_legacy_access_until IS
    'Fin du maintien provisoire des abonnements anterieurs a la tracabilite des paiements.';
