-- Phase 2 complète — PV, Paiements, Signalements
-- Dépendances : phase1 (communes, users, agents), phase2 (zones, interventions)

-- ─────────────────────────────────────────────────────────────────────────────
-- Colonne double_verbalisation_bloquant sur communes
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE communes
    ADD COLUMN IF NOT EXISTS double_verbalisation_bloquant BOOLEAN NOT NULL DEFAULT TRUE;

-- ─────────────────────────────────────────────────────────────────────────────
-- Procès-verbaux
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE pvs (
    id                   UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    commune_id           UUID        NOT NULL REFERENCES communes(id),
    agent_id             UUID        NOT NULL REFERENCES agents(id),
    pv_number            TEXT        NOT NULL UNIQUE,
    intervention_id      UUID        NOT NULL REFERENCES interventions(id),
    zone_id              UUID        REFERENCES zones(id),
    verbalized_name      TEXT,
    verbalized_identifier TEXT,
    vehicle_plate        TEXT,
    location_description TEXT,
    gps_latitude         NUMERIC(10, 7),
    gps_longitude        NUMERIC(10, 7),
    amount_initial       NUMERIC(12, 2),
    status               TEXT        NOT NULL DEFAULT 'EN_ATTENTE_PAIEMENT'
                             CHECK (status IN (
                                 'BROUILLON',
                                 'EMIS',
                                 'EN_ATTENTE_PAIEMENT',
                                 'PAYE',
                                 'EN_RETARD',
                                 'ANNULE',
                                 'CONTESTE',
                                 'NON_PAYANT'
                             )),
    qr_code_svg          TEXT,
    notes_internes       TEXT,
    created_by           UUID        NOT NULL REFERENCES users(id),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at           TIMESTAMPTZ
);

CREATE INDEX idx_pvs_commune_status  ON pvs (commune_id, status) WHERE deleted_at IS NULL;
CREATE INDEX idx_pvs_agent           ON pvs (agent_id)           WHERE deleted_at IS NULL;
CREATE INDEX idx_pvs_intervention    ON pvs (intervention_id)     WHERE deleted_at IS NULL;
-- Index pour la détection de double verbalisation
CREATE INDEX idx_pvs_double_verb_id  ON pvs (commune_id, intervention_id, verbalized_identifier)
    WHERE deleted_at IS NULL AND verbalized_identifier IS NOT NULL;
CREATE INDEX idx_pvs_double_verb_plate ON pvs (commune_id, intervention_id, vehicle_plate)
    WHERE deleted_at IS NULL AND vehicle_plate IS NOT NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- Historique des statuts PV
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE pv_status_history (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    pv_id       UUID        NOT NULL REFERENCES pvs(id),
    old_status  TEXT,
    new_status  TEXT        NOT NULL,
    changed_by  UUID        NOT NULL REFERENCES users(id),
    changed_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    reason      TEXT
);

CREATE INDEX idx_pv_status_history_pv ON pv_status_history (pv_id);

-- ─────────────────────────────────────────────────────────────────────────────
-- Paiements
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE payments (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    pv_id             UUID        NOT NULL UNIQUE REFERENCES pvs(id),
    commune_id        UUID        NOT NULL REFERENCES communes(id),
    amount_due        NUMERIC(12, 2) NOT NULL,
    amount_penalty    NUMERIC(12, 2) NOT NULL DEFAULT 0,
    amount_total      NUMERIC(12, 2) NOT NULL,
    amount_paid       NUMERIC(12, 2) NOT NULL,
    receiver_user_id  UUID        NOT NULL REFERENCES users(id),
    paid_at           TIMESTAMPTZ,
    status            TEXT        NOT NULL DEFAULT 'EN_ATTENTE'
                          CHECK (status IN ('EN_ATTENTE', 'PAYE', 'ANNULE')),
    receipt_number    TEXT        UNIQUE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_payments_commune    ON payments (commune_id, status);
CREATE INDEX idx_payments_receiver   ON payments (receiver_user_id);

-- ─────────────────────────────────────────────────────────────────────────────
-- Signalements
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE signalements (
    id                    UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    commune_id            UUID        NOT NULL REFERENCES communes(id),
    signalement_number    TEXT        NOT NULL UNIQUE,
    type_incident         TEXT        NOT NULL,
    location_description  TEXT        NOT NULL,
    description           TEXT        NOT NULL,
    contact_anonyme       BOOLEAN     NOT NULL DEFAULT FALSE,
    contact_info          TEXT,
    admin_notes           TEXT,
    status                TEXT        NOT NULL DEFAULT 'RECU'
                              CHECK (status IN ('RECU', 'EN_COURS', 'TRAITE', 'CLASSE', 'REJETE')),
    assigned_to           UUID        REFERENCES users(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_signalements_commune_status ON signalements (commune_id, status);
