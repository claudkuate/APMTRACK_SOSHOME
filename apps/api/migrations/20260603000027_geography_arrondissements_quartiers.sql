-- Découpage administratif national — niveaux 3 (Arrondissements) et 4 (Quartiers).
--
-- La migration 15 avait normalisé Régions (10) et Départements (58), mais en lecture
-- seule et sans les deux niveaux inférieurs. Le porteur du produit demande le répertoire
-- national complet « avec la possibilité de rajouter ou enlever au cas où la carte
-- administrative du Cameroun changerait ».
--
-- ⚠️ Cette migration ne crée AUCUNE donnée d'arrondissement ni de quartier : le fichier
-- source du client n'est pas disponible, et inventer un découpage national produirait
-- une donnée de référence fausse à laquelle tous les tenants feraient confiance. Le
-- contenu réel se charge par `POST /api/v1/geography/import-csv` ou la commande
-- `seed-geography`. Une donnée de référence absente vaut mieux qu'une donnée fausse.
--
-- Choix de modélisation : `quartiers` est un référentiel NATIONAL, distinct des `zones`.
-- `zones.commune_id` est NOT NULL et porte le cloisonnement multi-tenant (helpers
-- `resolve_commune_filter`, `pvs.zone_id`, couches carto) : rendre cette colonne
-- nullable pour y loger des quartiers nationaux serait un refactor à haut risque.
-- Les `zones` restent donc les aires opérationnelles d'une commune (quartier, marché,
-- axe routier, zone sensible) et `zones.quartier_id` fait le pont, à la demande.

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. Codes et unicité compatibles avec la suppression logique
-- ─────────────────────────────────────────────────────────────────────────────

-- Le fichier client porte des codes de département ; la table n'en avait pas.
-- Nullable : les 58 départements semés par la migration 15 restent à NULL jusqu'à
-- l'import du fichier national.
ALTER TABLE departements ADD COLUMN IF NOT EXISTS code TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS departements_code_unique_ci
    ON departements (lower(code))
    WHERE code IS NOT NULL AND deleted_at IS NULL;

-- L'ancienne contrainte UNIQUE (region_id, nom) était sensible à la casse et ignorait
-- `deleted_at` : « MFOUNDI » aurait coexisté avec « Mfoundi », et surtout un département
-- supprimé bloquait à jamais la recréation du même nom — ce qui contredit directement le
-- « enlever puis rajouter » demandé.
ALTER TABLE departements DROP CONSTRAINT IF EXISTS departements_region_id_nom_key;
CREATE UNIQUE INDEX IF NOT EXISTS departements_nom_region_unique_ci
    ON departements (region_id, lower(nom))
    WHERE deleted_at IS NULL;

-- Même correction sur regions.code (contrainte de table -> index partiel CI).
ALTER TABLE regions DROP CONSTRAINT IF EXISTS regions_code_key;
CREATE UNIQUE INDEX IF NOT EXISTS regions_code_unique_ci
    ON regions (lower(code))
    WHERE deleted_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. Arrondissements (niveau 3)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS arrondissements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    departement_id UUID NOT NULL REFERENCES departements(id),
    code TEXT,
    nom TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS arrondissements_departement_idx
    ON arrondissements (departement_id)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS arrondissements_nom_departement_unique_ci
    ON arrondissements (departement_id, lower(nom))
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS arrondissements_code_unique_ci
    ON arrondissements (lower(code))
    WHERE code IS NOT NULL AND deleted_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. Quartiers (niveau 4) — référentiel national, distinct des zones communales
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS quartiers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    arrondissement_id UUID NOT NULL REFERENCES arrondissements(id),
    code TEXT,
    nom TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS quartiers_arrondissement_idx
    ON quartiers (arrondissement_id)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS quartiers_nom_arrondissement_unique_ci
    ON quartiers (arrondissement_id, lower(nom))
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS quartiers_code_unique_ci
    ON quartiers (lower(code))
    WHERE code IS NOT NULL AND deleted_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. Liaisons — additives, nullables, sans backfill
-- ─────────────────────────────────────────────────────────────────────────────

-- Une commune d'arrondissement EST un arrondissement : le lien est 1:1 en droit, mais
-- volontairement NON contraint UNIQUE. Un doublon dans le fichier que le client
-- refournira produirait sinon un 23505 qui avorterait tout l'import en pleine semaine
-- de pilote ; l'unicité est signalée comme erreur de ligne, pas comme échec global.
ALTER TABLE communes
    ADD COLUMN IF NOT EXISTS arrondissement_id UUID REFERENCES arrondissements(id);

CREATE INDEX IF NOT EXISTS communes_arrondissement_id_idx
    ON communes (arrondissement_id)
    WHERE deleted_at IS NULL;

-- Pont optionnel zone communale -> quartier national. Aucune donnée existante n'est
-- modifiée : la colonne reste NULL tant qu'un administrateur ne rattache pas sa zone.
ALTER TABLE zones
    ADD COLUMN IF NOT EXISTS quartier_id UUID REFERENCES quartiers(id);

CREATE INDEX IF NOT EXISTS zones_quartier_id_idx
    ON zones (quartier_id)
    WHERE deleted_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- 5. Trigger de réconciliation — RESTITUÉ intégralement (CREATE OR REPLACE)
-- ─────────────────────────────────────────────────────────────────────────────
-- Ajout de la descente arrondissement -> département -> région. Le principe historique
-- est conservé : le trigger ne remplit que les valeurs NULL, il n'écrase jamais une
-- valeur explicitement fournie.

CREATE OR REPLACE FUNCTION communes_link_geography() RETURNS trigger AS $$
BEGIN
    -- (NOUVEAU) Descente depuis l'arrondissement.
    IF NEW.arrondissement_id IS NOT NULL AND NEW.departement_id IS NULL THEN
        SELECT a.departement_id INTO NEW.departement_id
        FROM arrondissements a
        WHERE a.id = NEW.arrondissement_id AND a.deleted_at IS NULL;
    END IF;

    IF NEW.region_id IS NULL AND NEW.departement_id IS NOT NULL THEN
        SELECT d.region_id INTO NEW.region_id
        FROM departements d
        WHERE d.id = NEW.departement_id AND d.deleted_at IS NULL;
    END IF;

    -- (INCHANGÉ) Texte -> identifiants.
    IF NEW.region_id IS NULL AND NEW.region IS NOT NULL THEN
        SELECT id INTO NEW.region_id
        FROM regions
        WHERE lower(nom) = lower(trim(NEW.region)) AND deleted_at IS NULL
        LIMIT 1;
    END IF;

    IF NEW.departement_id IS NULL AND NEW.departement IS NOT NULL THEN
        SELECT d.id INTO NEW.departement_id
        FROM departements d
        WHERE lower(d.nom) = lower(trim(NEW.departement))
          AND (NEW.region_id IS NULL OR d.region_id = NEW.region_id)
          AND d.deleted_at IS NULL
        LIMIT 1;
    END IF;

    -- (INCHANGÉ) Identifiants -> texte.
    IF (NEW.region IS NULL OR btrim(NEW.region) = '') AND NEW.region_id IS NOT NULL THEN
        SELECT nom INTO NEW.region FROM regions WHERE id = NEW.region_id;
    END IF;

    IF (NEW.departement IS NULL OR btrim(NEW.departement) = '') AND NEW.departement_id IS NOT NULL THEN
        SELECT nom INTO NEW.departement FROM departements WHERE id = NEW.departement_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- CRITIQUE : `arrondissement_id` doit figurer dans la liste « UPDATE OF », sinon un
-- PATCH ne renseignant que ce champ ne déclencherait pas le trigger et laisserait la
-- commune sans département ni région.
DROP TRIGGER IF EXISTS communes_link_geography_trg ON communes;
CREATE TRIGGER communes_link_geography_trg
    BEFORE INSERT OR UPDATE OF
        region, departement, region_id, departement_id, arrondissement_id
    ON communes
    FOR EACH ROW EXECUTE FUNCTION communes_link_geography();
