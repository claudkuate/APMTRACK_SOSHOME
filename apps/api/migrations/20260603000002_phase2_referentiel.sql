-- Phase 2: Zones géographiques + Référentiel des interventions

CREATE TABLE IF NOT EXISTS zones (
    id UUID PRIMARY KEY,
    commune_id UUID NOT NULL REFERENCES communes(id),
    nom TEXT NOT NULL,
    type_zone TEXT NOT NULL,
    parent_id UUID REFERENCES zones(id),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT zones_type_check CHECK (
        type_zone IN (
            'QUARTIER', 'BLOC', 'SECTEUR', 'LIEU_DIT',
            'MARCHE', 'AXE_ROUTIER', 'ZONE_COMMERCIALE', 'ZONE_SENSIBLE'
        )
    )
);

CREATE INDEX IF NOT EXISTS zones_commune_id_idx ON zones (commune_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS zones_parent_id_idx ON zones (parent_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS zones_active_idx ON zones (active) WHERE deleted_at IS NULL;

-- Unicité du nom par commune (insensible à la casse)
CREATE UNIQUE INDEX IF NOT EXISTS zones_nom_commune_unique_ci
    ON zones (commune_id, lower(nom))
    WHERE deleted_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- Référentiel des interventions : Catégorie → Type → Intervention
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS intervention_categories (
    id UUID PRIMARY KEY,
    commune_id UUID NOT NULL REFERENCES communes(id),
    nom TEXT NOT NULL,
    description TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS interv_categories_commune_idx
    ON intervention_categories (commune_id) WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS interv_categories_nom_commune_unique_ci
    ON intervention_categories (commune_id, lower(nom))
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS intervention_types (
    id UUID PRIMARY KEY,
    commune_id UUID NOT NULL REFERENCES communes(id),
    category_id UUID NOT NULL REFERENCES intervention_categories(id),
    nom TEXT NOT NULL,
    description TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS interv_types_commune_idx
    ON intervention_types (commune_id) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS interv_types_category_idx
    ON intervention_types (category_id) WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS interv_types_nom_category_unique_ci
    ON intervention_types (category_id, lower(nom))
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS interventions (
    id UUID PRIMARY KEY,
    commune_id UUID NOT NULL REFERENCES communes(id),
    category_id UUID NOT NULL REFERENCES intervention_categories(id),
    type_id UUID NOT NULL REFERENCES intervention_types(id),
    nom TEXT NOT NULL,
    description TEXT,
    sujet_paiement BOOLEAN NOT NULL DEFAULT FALSE,
    montant NUMERIC(12, 2),
    delai_paiement_jours INTEGER,
    taux_penalite NUMERIC(5, 2),
    reference_deliberation TEXT,
    piece_justificative TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT interventions_montant_check CHECK (
        (sujet_paiement = FALSE) OR (montant IS NOT NULL AND montant > 0)
    ),
    CONSTRAINT interventions_deliberation_check CHECK (
        (sujet_paiement = FALSE) OR (reference_deliberation IS NOT NULL AND reference_deliberation <> '')
    )
);

CREATE INDEX IF NOT EXISTS interventions_commune_idx
    ON interventions (commune_id) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS interventions_category_idx
    ON interventions (category_id) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS interventions_type_idx
    ON interventions (type_id) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS interventions_active_idx
    ON interventions (active) WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS interventions_nom_type_unique_ci
    ON interventions (type_id, lower(nom))
    WHERE deleted_at IS NULL;
