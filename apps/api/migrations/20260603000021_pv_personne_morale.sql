-- Personne morale sur un PV.
-- Permet de verbaliser un contribuable personne morale (ex. raison sociale « Ets CAMERAMAN »)
-- en plus des personnes physiques. `subject_kind` distingue le type ; `raison_sociale` porte
-- la dénomination sociale. Pour une personne morale, la raison sociale tient lieu de nom du
-- contrevenant (colonne `verbalized_name` alimentée côté application).

ALTER TABLE pvs
    ADD COLUMN IF NOT EXISTS subject_kind TEXT NOT NULL DEFAULT 'PHYSIQUE'
        CHECK (subject_kind IN ('PHYSIQUE', 'MORALE')),
    ADD COLUMN IF NOT EXISTS raison_sociale TEXT;
