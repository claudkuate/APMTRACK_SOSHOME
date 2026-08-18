-- Defense en profondeur pour les confirmations inserees hors de l'API.
-- Le handler conserve ses validations afin de retourner des erreurs metier
-- explicites, tandis que ce trigger protege les scripts et imports SQL directs.

ALTER TABLE commune_subscription_payments
    ADD CONSTRAINT commune_subscription_payment_reference_not_blank
        CHECK (btrim(payment_reference) <> ''),
    ADD CONSTRAINT commune_subscription_payment_reference_length
        CHECK (char_length(payment_reference) <= 160);

DROP INDEX IF EXISTS commune_subscription_payment_reference_unique_ci;
CREATE UNIQUE INDEX commune_subscription_payment_reference_unique_ci
    ON commune_subscription_payments (commune_id, lower(btrim(payment_reference)));

CREATE OR REPLACE FUNCTION validate_commune_subscription_payment_insert()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.paid_at > clock_timestamp() THEN
        RAISE EXCEPTION 'subscription payment paid_at cannot be in the future'
            USING ERRCODE = '23514',
                  CONSTRAINT = 'commune_subscription_payment_paid_at_not_future';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM users u
        INNER JOIN user_roles ur ON ur.user_id = u.id
        INNER JOIN roles r ON r.id = ur.role_id
        WHERE u.id = NEW.confirmed_by_user_id
          AND u.active = TRUE
          AND u.deleted_at IS NULL
          AND r.code = 'SUPER_ADMIN'
    ) THEN
        RAISE EXCEPTION 'subscription payment confirmer must be an active SUPER_ADMIN'
            USING ERRCODE = '23514',
                  CONSTRAINT = 'commune_subscription_payment_confirmer_super_admin';
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS commune_subscription_payments_validate_insert
    ON commune_subscription_payments;
DROP TRIGGER IF EXISTS commune_subscription_payment_insert_guard
    ON commune_subscription_payments;
CREATE TRIGGER commune_subscription_payment_insert_guard
BEFORE INSERT ON commune_subscription_payments
FOR EACH ROW EXECUTE FUNCTION validate_commune_subscription_payment_insert();

-- Les bases ayant deja applique la migration 30 recoivent aussi la semantique
-- « droit confirme non expire », y compris lorsque son debut est futur.
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
