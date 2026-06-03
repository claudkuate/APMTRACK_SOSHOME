-- Durcissement pilote: audit multi-commune, sequences documentaires et montants FCFA entiers.

ALTER TABLE audit_logs
    ADD COLUMN IF NOT EXISTS commune_id UUID REFERENCES communes(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS audit_logs_commune_created_at_idx
    ON audit_logs (commune_id, created_at DESC);

CREATE TABLE IF NOT EXISTS document_sequences (
    commune_id UUID NOT NULL REFERENCES communes(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL,
    year INTEGER NOT NULL,
    next_value BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (commune_id, kind, year),
    CONSTRAINT document_sequences_kind_check CHECK (
        kind IN ('PV', 'RECEIPT', 'SIGNALEMENT')
    ),
    CONSTRAINT document_sequences_next_value_check CHECK (next_value > 0)
);

ALTER TABLE interventions
    ADD COLUMN IF NOT EXISTS montant_fcfa BIGINT,
    ADD COLUMN IF NOT EXISTS taux_penalite_basis_points INTEGER;

UPDATE interventions
SET montant_fcfa = ROUND(montant)::BIGINT
WHERE montant_fcfa IS NULL AND montant IS NOT NULL;

UPDATE interventions
SET taux_penalite_basis_points = ROUND(taux_penalite * 100)::INTEGER
WHERE taux_penalite_basis_points IS NULL AND taux_penalite IS NOT NULL;

ALTER TABLE interventions
    DROP CONSTRAINT IF EXISTS interventions_montant_fcfa_check,
    ADD CONSTRAINT interventions_montant_fcfa_check CHECK (
        montant_fcfa IS NULL OR montant_fcfa >= 0
    ),
    DROP CONSTRAINT IF EXISTS interventions_taux_penalite_basis_points_check,
    ADD CONSTRAINT interventions_taux_penalite_basis_points_check CHECK (
        taux_penalite_basis_points IS NULL OR taux_penalite_basis_points >= 0
    );

ALTER TABLE pvs
    ADD COLUMN IF NOT EXISTS amount_initial_fcfa BIGINT;

UPDATE pvs
SET amount_initial_fcfa = ROUND(amount_initial)::BIGINT
WHERE amount_initial_fcfa IS NULL AND amount_initial IS NOT NULL;

ALTER TABLE pvs
    DROP CONSTRAINT IF EXISTS pvs_amount_initial_fcfa_check,
    ADD CONSTRAINT pvs_amount_initial_fcfa_check CHECK (
        amount_initial_fcfa IS NULL OR amount_initial_fcfa >= 0
    );

ALTER TABLE payments
    ADD COLUMN IF NOT EXISTS amount_due_fcfa BIGINT,
    ADD COLUMN IF NOT EXISTS amount_penalty_fcfa BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS amount_total_fcfa BIGINT,
    ADD COLUMN IF NOT EXISTS amount_paid_fcfa BIGINT;

UPDATE payments
SET
    amount_due_fcfa = COALESCE(amount_due_fcfa, ROUND(amount_due)::BIGINT),
    amount_penalty_fcfa = COALESCE(amount_penalty_fcfa, ROUND(amount_penalty)::BIGINT),
    amount_total_fcfa = COALESCE(amount_total_fcfa, ROUND(amount_total)::BIGINT),
    amount_paid_fcfa = COALESCE(amount_paid_fcfa, ROUND(amount_paid)::BIGINT);

ALTER TABLE payments
    DROP CONSTRAINT IF EXISTS payments_amounts_fcfa_check,
    ADD CONSTRAINT payments_amounts_fcfa_check CHECK (
        amount_due_fcfa IS NULL OR amount_due_fcfa >= 0
    ),
    DROP CONSTRAINT IF EXISTS payments_amount_penalty_fcfa_check,
    ADD CONSTRAINT payments_amount_penalty_fcfa_check CHECK (
        amount_penalty_fcfa >= 0
    ),
    DROP CONSTRAINT IF EXISTS payments_amount_total_fcfa_check,
    ADD CONSTRAINT payments_amount_total_fcfa_check CHECK (
        amount_total_fcfa IS NULL OR amount_total_fcfa >= 0
    ),
    DROP CONSTRAINT IF EXISTS payments_amount_paid_fcfa_check,
    ADD CONSTRAINT payments_amount_paid_fcfa_check CHECK (
        amount_paid_fcfa IS NULL OR amount_paid_fcfa >= 0
    );

CREATE TABLE IF NOT EXISTS generated_documents (
    id UUID PRIMARY KEY,
    commune_id UUID REFERENCES communes(id) ON DELETE SET NULL,
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    document_type TEXT NOT NULL,
    filename TEXT NOT NULL,
    generated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS generated_documents_entity_idx
    ON generated_documents (entity_type, entity_id, generated_at DESC);
