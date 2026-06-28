-- Module Fourrières (mise en fourrière de véhicules).
-- Vision G-APM : « gestion des fourrières » comme niche de recettes communales.
-- Multi-tenant (commune_id), soft-delete, lié optionnellement au PV d'origine.

CREATE TABLE IF NOT EXISTS fourrieres (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commune_id UUID NOT NULL REFERENCES communes(id),
    pv_id UUID REFERENCES pvs(id),
    fourriere_number TEXT NOT NULL,
    vehicle_plate TEXT NOT NULL,
    vehicle_type TEXT,
    vehicle_details TEXT,
    motif TEXT NOT NULL,
    lieu_enlevement TEXT,
    status TEXT NOT NULL DEFAULT 'EN_FOURRIERE'
        CHECK (status IN ('EN_FOURRIERE', 'RESTITUE', 'VENDU', 'DETRUIT')),
    daily_fee_fcfa BIGINT NOT NULL DEFAULT 0 CHECK (daily_fee_fcfa >= 0),
    entered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ,
    released_to TEXT,
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (commune_id, fourriere_number)
);

CREATE INDEX IF NOT EXISTS idx_fourrieres_commune
    ON fourrieres (commune_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_fourrieres_status
    ON fourrieres (commune_id, status)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_fourrieres_plate
    ON fourrieres (commune_id, vehicle_plate)
    WHERE deleted_at IS NULL;
