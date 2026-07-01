-- Intervention système « Mise en fourrière » : chaque mise en fourrière sans PV
-- existant génère automatiquement un PV adossé à cette intervention. `system_code`
-- identifie l'intervention de manière stable (robuste au renommage par l'admin
-- commune). Montant/délai/pénalité par défaut, ajustables ensuite via le référentiel.

ALTER TABLE interventions
    ADD COLUMN IF NOT EXISTS system_code TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS interventions_system_code_unique
    ON interventions (commune_id, system_code)
    WHERE deleted_at IS NULL AND system_code IS NOT NULL;

-- 1) Adopter une éventuelle intervention « Mise en fourrière » créée manuellement
--    (évite un doublon et un conflit avec interventions_nom_type_unique_ci).
UPDATE interventions i
SET system_code = 'FOURRIERE'
WHERE i.id IN (
    SELECT DISTINCT ON (commune_id) id
    FROM interventions
    WHERE lower(nom) = 'mise en fourrière'
      AND deleted_at IS NULL
      AND system_code IS NULL
    ORDER BY commune_id, created_at
)
AND NOT EXISTS (
    SELECT 1 FROM interventions x
    WHERE x.commune_id = i.commune_id
      AND x.system_code = 'FOURRIERE'
      AND x.deleted_at IS NULL
);

-- 2) Catégorie « Fourrière » (réutilisée si elle existe déjà — index unique CI sur le nom).
INSERT INTO intervention_categories (id, commune_id, nom, description, active)
SELECT gen_random_uuid(), c.id, 'Fourrière',
       'Mises en fourrière (référentiel système)', TRUE
FROM communes c
WHERE c.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM intervention_categories ic
      WHERE ic.commune_id = c.id
        AND lower(ic.nom) = 'fourrière'
        AND ic.deleted_at IS NULL
  );

-- 3) Type « Mise en fourrière » sous la catégorie « Fourrière ».
INSERT INTO intervention_types (id, commune_id, category_id, nom, description, active)
SELECT gen_random_uuid(), ic.commune_id, ic.id, 'Mise en fourrière',
       'Enlèvement et mise en fourrière (véhicules et autres objets)', TRUE
FROM intervention_categories ic
WHERE lower(ic.nom) = 'fourrière'
  AND ic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM intervention_types it
      WHERE it.category_id = ic.id
        AND lower(it.nom) = 'mise en fourrière'
        AND it.deleted_at IS NULL
  );

-- 4) Intervention par commune — 25 000 FCFA, délai 7 j, pénalité 10 %, modifiable
--    ensuite par la commune. reference_deliberation exigée par le CHECK
--    interventions_deliberation_check (sujet_paiement = TRUE). Source restreinte
--    au type de la catégorie « Fourrière » (une seule ligne par commune, même si
--    un type homonyme existe sous une autre catégorie).
INSERT INTO interventions (
    id, commune_id, type_id, nom, description, sujet_paiement,
    montant, montant_fcfa, delai_paiement_jours,
    taux_penalite, taux_penalite_basis_points,
    reference_deliberation, requires_vehicle, active, system_code
)
SELECT gen_random_uuid(), t.commune_id, t.type_id,
       'Mise en fourrière',
       'Frais forfaitaires de mise en fourrière (montant à ajuster par la commune)',
       TRUE, 25000, 25000, 7, 10, 1000,
       'A_COMPLETER', FALSE, TRUE, 'FOURRIERE'
FROM (
    SELECT DISTINCT ON (it.commune_id) it.commune_id, it.id AS type_id
    FROM intervention_types it
    JOIN intervention_categories ic ON ic.id = it.category_id
    WHERE lower(it.nom) = 'mise en fourrière'
      AND lower(ic.nom) = 'fourrière'
      AND it.deleted_at IS NULL
      AND ic.deleted_at IS NULL
    ORDER BY it.commune_id, it.created_at
) t
WHERE NOT EXISTS (
    SELECT 1 FROM interventions i
    WHERE i.commune_id = t.commune_id
      AND i.system_code = 'FOURRIERE'
      AND i.deleted_at IS NULL
);
