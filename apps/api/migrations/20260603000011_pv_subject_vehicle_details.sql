-- Enrichissement des donnees contrevenant et vehicule sur les PV.
-- Les champs restent nullable pour conserver la compatibilite des PV existants
-- et des anciens clients API.

ALTER TABLE pvs
    ADD COLUMN IF NOT EXISTS verbalized_first_name TEXT,
    ADD COLUMN IF NOT EXISTS verbalized_last_name TEXT,
    ADD COLUMN IF NOT EXISTS verbalized_identity_type TEXT,
    ADD COLUMN IF NOT EXISTS verbalized_identity_number TEXT,
    ADD COLUMN IF NOT EXISTS verbalized_phone TEXT,
    ADD COLUMN IF NOT EXISTS verbalized_address TEXT,
    ADD COLUMN IF NOT EXISTS vehicle_registration_card_number TEXT,
    ADD COLUMN IF NOT EXISTS vehicle_make TEXT,
    ADD COLUMN IF NOT EXISTS vehicle_model TEXT,
    ADD COLUMN IF NOT EXISTS vehicle_color TEXT,
    ADD COLUMN IF NOT EXISTS vehicle_owner_name TEXT;

UPDATE pvs
SET verbalized_identity_number = verbalized_identifier
WHERE verbalized_identity_number IS NULL
  AND COALESCE(verbalized_identifier, '') <> '';

UPDATE pvs
SET verbalized_identity_type = 'AUTRE'
WHERE verbalized_identity_type IS NULL
  AND COALESCE(verbalized_identity_number, '') <> '';

UPDATE pvs
SET verbalized_last_name = verbalized_name
WHERE verbalized_last_name IS NULL
  AND verbalized_first_name IS NULL
  AND COALESCE(verbalized_name, '') <> '';

ALTER TABLE pvs
    DROP CONSTRAINT IF EXISTS pvs_verbalized_identity_type_check,
    ADD CONSTRAINT pvs_verbalized_identity_type_check CHECK (
        verbalized_identity_type IS NULL
        OR verbalized_identity_type IN (
            'CNI',
            'PASSEPORT',
            'PERMIS_CONDUIRE',
            'CARTE_SEJOUR',
            'NIU',
            'AUTRE'
        )
    );

CREATE INDEX IF NOT EXISTS idx_pvs_double_verb_identity_number
    ON pvs (commune_id, intervention_id, verbalized_identity_number)
    WHERE deleted_at IS NULL AND verbalized_identity_number IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_pvs_double_verb_registration_card
    ON pvs (commune_id, intervention_id, vehicle_registration_card_number)
    WHERE deleted_at IS NULL AND vehicle_registration_card_number IS NOT NULL;
