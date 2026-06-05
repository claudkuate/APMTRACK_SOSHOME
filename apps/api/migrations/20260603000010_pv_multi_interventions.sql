-- PV multi-infractions et typage du sujet verbalisé.
-- `pvs.intervention_id` reste la référence legacy / infraction principale.

ALTER TABLE pvs
    ADD COLUMN IF NOT EXISTS subject_type TEXT NOT NULL DEFAULT 'PERSON_WITH_VEHICLE';

ALTER TABLE pvs
    DROP CONSTRAINT IF EXISTS pvs_subject_type_check,
    ADD CONSTRAINT pvs_subject_type_check CHECK (
        subject_type IN ('PERSON_ONLY', 'VEHICLE_ONLY', 'PERSON_WITH_VEHICLE')
    );

CREATE TABLE IF NOT EXISTS pv_interventions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pv_id UUID NOT NULL REFERENCES pvs(id) ON DELETE CASCADE,
    intervention_id UUID NOT NULL REFERENCES interventions(id),
    order_index INTEGER NOT NULL DEFAULT 0,
    nom TEXT NOT NULL,
    sujet_paiement BOOLEAN NOT NULL DEFAULT FALSE,
    montant_fcfa BIGINT,
    delai_paiement_jours INTEGER,
    taux_penalite NUMERIC(5, 2),
    taux_penalite_basis_points INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT pv_interventions_amount_check CHECK (
        montant_fcfa IS NULL OR montant_fcfa >= 0
    ),
    CONSTRAINT pv_interventions_order_unique UNIQUE (pv_id, order_index),
    CONSTRAINT pv_interventions_intervention_unique UNIQUE (pv_id, intervention_id)
);

CREATE INDEX IF NOT EXISTS idx_pv_interventions_pv
    ON pv_interventions (pv_id, order_index);

CREATE INDEX IF NOT EXISTS idx_pv_interventions_intervention
    ON pv_interventions (intervention_id);

INSERT INTO pv_interventions (
    pv_id,
    intervention_id,
    order_index,
    nom,
    sujet_paiement,
    montant_fcfa,
    delai_paiement_jours,
    taux_penalite,
    taux_penalite_basis_points
)
SELECT
    p.id,
    i.id,
    0,
    i.nom,
    i.sujet_paiement,
    COALESCE(p.amount_initial_fcfa, i.montant_fcfa),
    i.delai_paiement_jours,
    i.taux_penalite,
    i.taux_penalite_basis_points
FROM pvs p
JOIN interventions i ON i.id = p.intervention_id
WHERE p.deleted_at IS NULL
ON CONFLICT (pv_id, intervention_id) DO NOTHING;

UPDATE pvs
SET subject_type = CASE
    WHEN COALESCE(vehicle_plate, '') <> ''
         AND (COALESCE(verbalized_name, '') <> '' OR COALESCE(verbalized_identifier, '') <> '')
        THEN 'PERSON_WITH_VEHICLE'
    WHEN COALESCE(vehicle_plate, '') <> ''
        THEN 'VEHICLE_ONLY'
    ELSE 'PERSON_ONLY'
END
WHERE subject_type = 'PERSON_WITH_VEHICLE';
